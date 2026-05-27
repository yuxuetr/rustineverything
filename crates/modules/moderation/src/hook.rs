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
use std::sync::{Arc, OnceLock};

use rustineverything_core::engines::moderation::Verdict;
use rustineverything_core::settings::SiteConfig;
use rustineverything_llm::{default_client_from_env, LlmClient};
use rustineverything_sdk::{ImageRef, ModerationSubmission};

use crate::pipeline::ModerationPipeline;

static SHARED: OnceLock<Arc<ModerationPipeline>> = OnceLock::new();

/// 进程级共享 pipeline。第一次访问时从 `site.json` + env 装载。
///
/// 后续 `site.json` 改动需要重启进程才会生效（Phase 5.1 hot reload 之前）。
pub fn shared_pipeline() -> Arc<ModerationPipeline> {
  SHARED
    .get_or_init(|| {
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
          "moderation: shared pipeline initialized"
        );
      }
      Arc::new(pipeline)
    })
    .clone()
}

/// 仅供测试 / hot reload 使用：清空全局 pipeline 以便下一次访问重读 site.json。
/// 生产代码不要调；当前 OnceLock 不支持 reset，所以该函数留为占位接口。
#[doc(hidden)]
pub fn _internal_reset_for_tests() {
  // OnceLock 在 stable rust 不支持 take（要 nightly），所以这里只是占位。
  // 真正的 hot reload 见 Phase 5.1。
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
