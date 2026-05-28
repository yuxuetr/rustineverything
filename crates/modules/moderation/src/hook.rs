//! 给业务模块（comments / forum / annotation）一站式接入审核流水线的便利层。
//!
//! ## 入口
//! [`evaluate_submission`] / [`evaluate_with_images`]：开箱即用，背后自动
//! 走 [`shared_pipeline`]（基于 `site.json` + env 一次性初始化的全局实例）。
//!
//! ## 默认零开销
//! `site.json::moderation.enabled = false` 时全局 pipeline 内部 stages 为
//! 空，evaluate 直接返回 Allow，不进 wasm / 不调 LLM。
//!
//! ## 调用模板
//! ```ignore
//! use rustineverything_module_moderation::hook::evaluate_submission;
//! use rustineverything_module_moderation::ModerationLabel;
//! use rustineverything_sdk::ModerationSubmission;
//!
//! let verdict = evaluate_submission(
//!   ModerationSubmission::new(comment_body)
//!     .with_kind("comment")
//!     .with_ref_path(format!("blog/{}", blog_id))
//!     .with_images(extract_uploaded_images(&comment_body)),
//! ).await;
//! match verdict.label {
//!   ModerationLabel::Block => return Err(ServerFnError::new(format!(
//!     "评论被审核拒绝：{}", verdict.reason
//!   ))),
//!   ModerationLabel::Flag  => tracing::warn!(reason = %verdict.reason, "moderation: flagged"),
//!   ModerationLabel::Allow => {}
//! }
//! ```

use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use rustineverything_core::engines::moderation::{ModerationLabel, Verdict};
use rustineverything_core::settings::SiteConfig;
use rustineverything_llm::{default_client_from_env, LlmClient};
use rustineverything_sdk::{ImageRef, ModerationSubmission};

use crate::pipeline::ModerationPipeline;

/// 进程级共享 pipeline，包在 `RwLock` 里以支持 hot reload（Phase 5.1）。
/// 读多写极少（仅 admin reload 时写一次），`RwLock` 读锁开销可忽略。
static SHARED: OnceLock<RwLock<Arc<ModerationPipeline>>> = OnceLock::new();

/// 从 `site.json` + env 重新构造一条 pipeline。
fn build_pipeline() -> ModerationPipeline {
  let site = SiteConfig::from_file(
    rustineverything_core::utils::get_asset_root()
      .join("site.json")
      .to_str()
      .unwrap_or_default(),
  )
  .unwrap_or_default();

  let plugin_dir: PathBuf = rustineverything_core::utils::get_asset_root().join("plugins");
  let llm: Option<Arc<dyn LlmClient>> = default_client_from_env().map(Arc::from);

  let pipeline = ModerationPipeline::from_site_config(&site, &plugin_dir, llm);
  if pipeline.is_empty() {
    tracing::info!("moderation: shared pipeline empty (disabled or unconfigured)");
  } else {
    tracing::info!(
      stages = ?pipeline.stage_names(),
      "moderation: shared pipeline (re)initialized"
    );
  }
  pipeline
}

fn cell() -> &'static RwLock<Arc<ModerationPipeline>> {
  SHARED.get_or_init(|| RwLock::new(Arc::new(build_pipeline())))
}

/// 进程级共享 pipeline。第一次访问时从 `site.json` + env 装载。
///
/// 调用 [`reload_pipeline`] 后会换成新装载的实例（无需重启进程）。
pub fn shared_pipeline() -> Arc<ModerationPipeline> {
  match cell().read() {
    Ok(guard) => guard.clone(),
    // 锁中毒（某次写时 panic）→ 退化到一条新装载的 pipeline，绝不阻塞业务。
    Err(poisoned) => poisoned.into_inner().clone(),
  }
}

/// Phase 5.1 hot reload：重读 `site.json` + 插件目录，原子替换全局 pipeline。
/// admin 上传 / 切换审核插件或改阈值后调用即可生效，无需重启进程。
///
/// 旧 pipeline（连同其 `PluginManager` 缓存的 wasmi `Module`）在替换后引用计
/// 数归零即被 Drop，不会泄漏。
pub fn reload_pipeline() {
  let fresh = Arc::new(build_pipeline());
  match cell().write() {
    Ok(mut guard) => *guard = fresh,
    Err(poisoned) => *poisoned.into_inner() = fresh,
  }
}

/// 仅供测试使用：等价于 [`reload_pipeline`]，强制下一次访问重读 site.json。
#[doc(hidden)]
pub fn _internal_reset_for_tests() {
  reload_pipeline();
}

/// 用全局 pipeline 评估一条提交。便捷的「一行接入」入口。
pub async fn evaluate_submission(submission: ModerationSubmission) -> Verdict {
  shared_pipeline().evaluate(submission).await
}

/// 包含图像的便捷形式。把图像作为 `ModerationSubmission.images` 字段。
pub async fn evaluate_with_images(
  content: impl Into<String>,
  kind: impl Into<String>,
  ref_path: impl Into<String>,
  image_urls: impl IntoIterator<Item = String>,
) -> Verdict {
  let images: Vec<ImageRef> = image_urls.into_iter().map(ImageRef::url).collect();
  let submission = ModerationSubmission::new(content)
    .with_kind(kind)
    .with_ref_path(ref_path)
    .with_images(images);
  evaluate_submission(submission).await
}

/// Phase 4.5：把 Flag verdict 入库到 `moderation_queue`。
///
/// Allow / Block 都是 no-op：Allow 不需要审核，Block 在上游已经被拒绝
/// （未入业务库）。**只有 Flag 进队列**等 admin 复核。
///
/// `ref_id` 是业务表的主键（comment.id 等）。允许 None — 如果调用方
/// 在业务插入前就想入队（用于事务一致性时），ref_id 可以为 None，但通常
/// 推荐先插业务行拿到 id 再入队。
///
/// 失败仅 log warning，不传播错误：审核记录丢失不应影响主业务流。
pub async fn enqueue_if_flagged(
  db: &sea_orm::DatabaseConnection,
  verdict: &Verdict,
  kind: &str,
  ref_id: Option<i64>,
  ref_path: &str,
  user_id: Option<i32>,
  content: &str,
  image_urls: &[String],
) {
  if verdict.label != ModerationLabel::Flag {
    return;
  }
  use rustineverything_core::entities::moderation_queue;
  use sea_orm::{ActiveModelTrait, ActiveValue::Set};

  let images_json = if image_urls.is_empty() {
    None
  } else {
    serde_json::to_string(image_urls).ok()
  };

  let am = moderation_queue::ActiveModel {
    kind: Set(kind.to_string()),
    ref_id: Set(ref_id),
    ref_path: Set(ref_path.to_string()),
    user_id: Set(user_id),
    content: Set(content.to_string()),
    images: Set(images_json),
    score: Set(verdict.score),
    label: Set("flag".to_string()),
    reason: Set(verdict.reason.clone()),
    status: Set("pending".to_string()),
    reviewer_user_id: Set(None),
    reviewer_note: Set(None),
    created_at: Set(chrono::Utc::now().fixed_offset()),
    reviewed_at: Set(None),
    ..Default::default()
  };
  match am.insert(db).await {
    Ok(row) => tracing::info!(
      queue_id = row.id,
      kind = %kind,
      ref_path = %ref_path,
      "moderation: flagged item enqueued for review"
    ),
    Err(e) => tracing::warn!(
      error = %e,
      kind = %kind,
      ref_path = %ref_path,
      "moderation: failed to enqueue flagged item (业务不受影响)"
    ),
  }
}

// ────────────────────────────────────────────────────────────
// Markdown 图片 URL 抽取
// ────────────────────────────────────────────────────────────

/// 从 markdown 文本里抽出 `![alt](url)` 形式的图片链接。
///
/// 抽取规则（简版）：
/// - `![alt](url)` 句法，alt 可空
/// - URL 允许 `http://...` / `https://...` / 站内 `/uploads/...`（绝对路径）
/// - 不识别 `<img src="">` HTML 形式（评论场景默认走 markdown）
/// - 重复 URL 不去重，按出现顺序
pub fn extract_image_urls(markdown: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = markdown.as_bytes();
  let len = bytes.len();
  let mut i = 0;
  while i + 4 < len {
    if bytes[i] == b'!' && bytes[i + 1] == b'[' {
      // 找 alt 结尾 `]`
      let alt_start = i + 2;
      let mut p = alt_start;
      while p < len && bytes[p] != b']' {
        p += 1;
      }
      if p >= len - 1 || bytes[p + 1] != b'(' {
        i += 1;
        continue;
      }
      let url_start = p + 2;
      let mut q = url_start;
      // URL 内允许嵌套括号？markdown spec 严格说不允许 — 用 `)` 终止即可。
      // 但有些图床 URL 会带 query 含 `)` —— 我们走严格路径，遇 `)` 即截断。
      while q < len && bytes[q] != b')' {
        q += 1;
      }
      if q >= len {
        // 未闭合，放弃整段
        break;
      }
      // URL 可能带 title `(url "title")`；按空白 + 双引号截断
      let raw = &markdown[url_start..q];
      let url = match raw.split_once(char::is_whitespace) {
        Some((u, _)) => u,
        None => raw,
      }
      .trim();
      if !url.is_empty() && is_acceptable_image_url(url) {
        out.push(url.to_string());
      }
      i = q + 1;
    } else {
      i += 1;
    }
  }
  out
}

fn is_acceptable_image_url(url: &str) -> bool {
  url.starts_with("http://")
    || url.starts_with("https://")
    || url.starts_with("/uploads/")
    || url.starts_with("/images/")
}

/// 站内相对路径（如 `/uploads/xxx.jpg`）→ 绝对 URL（用 `BASE_URL` 拼接）。
/// 已是 `http(s)://` 起首的原样返回；其它无 base_url 时也原样返回（LLM 大概率失败 →
/// stage fail-open）。
pub fn absolutize_image_url(url: &str, base_url: &str) -> String {
  if url.starts_with("http://") || url.starts_with("https://") {
    return url.to_string();
  }
  let base = base_url.trim_end_matches('/');
  if base.is_empty() {
    return url.to_string();
  }
  if url.starts_with('/') {
    format!("{}{}", base, url)
  } else {
    format!("{}/{}", base, url)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_no_image() {
    assert!(extract_image_urls("纯文本，没有图").is_empty());
    assert!(extract_image_urls("有链接但不是图 https://x.com").is_empty());
  }

  #[test]
  fn extract_single_image_with_alt() {
    let urls = extract_image_urls("看图：![一只猫](https://example.com/cat.jpg) 可爱");
    assert_eq!(urls, vec!["https://example.com/cat.jpg"]);
  }

  #[test]
  fn extract_image_with_uploads_path() {
    let urls = extract_image_urls("![](/uploads/abc.jpg)");
    assert_eq!(urls, vec!["/uploads/abc.jpg"]);
  }

  #[test]
  fn extract_multiple_images() {
    let urls = extract_image_urls("![a](/uploads/1.png) 和 ![b](https://x.example/2.jpg)");
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0], "/uploads/1.png");
    assert_eq!(urls[1], "https://x.example/2.jpg");
  }

  #[test]
  fn extract_skips_non_image_links() {
    // 普通 link `[a](url)` 不是图片
    let urls = extract_image_urls("[纯链接](https://x.com)");
    assert!(urls.is_empty());
  }

  #[test]
  fn extract_skips_relative_filename_only() {
    // 不带 / 前缀也不是 http → 跳过
    let urls = extract_image_urls("![bad](relative.jpg)");
    assert!(urls.is_empty());
  }

  #[test]
  fn extract_skips_unclosed_paren() {
    let urls = extract_image_urls("![bad](/uploads/x.jpg");
    assert!(urls.is_empty());
  }

  #[test]
  fn extract_strips_title_attribute() {
    // ![alt](url "title")
    let urls = extract_image_urls(r#"![cat](/uploads/cat.png "可爱的猫")"#);
    assert_eq!(urls, vec!["/uploads/cat.png"]);
  }

  // ── absolutize ──

  #[test]
  fn absolutize_absolute_url_unchanged() {
    let u = absolutize_image_url("https://example.com/x.jpg", "https://my.site");
    assert_eq!(u, "https://example.com/x.jpg");
  }

  #[test]
  fn absolutize_relative_with_base() {
    let u = absolutize_image_url("/uploads/x.jpg", "https://my.site");
    assert_eq!(u, "https://my.site/uploads/x.jpg");
  }

  #[test]
  fn absolutize_relative_strips_base_trailing_slash() {
    let u = absolutize_image_url("/uploads/x.jpg", "https://my.site/");
    assert_eq!(u, "https://my.site/uploads/x.jpg");
  }

  #[test]
  fn absolutize_no_base_returns_relative_as_is() {
    let u = absolutize_image_url("/uploads/x.jpg", "");
    assert_eq!(u, "/uploads/x.jpg");
  }

  #[test]
  fn absolutize_relative_no_leading_slash() {
    let u = absolutize_image_url("uploads/x.jpg", "https://my.site");
    assert_eq!(u, "https://my.site/uploads/x.jpg");
  }
}
