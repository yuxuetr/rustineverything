#![allow(clippy::field_reassign_with_default)] // 测试里 Default + 逐字段赋值更易读
//! 端到端 hook 测试：模拟业务模块（comments / forum）调用 hook 层的完整
//! 路径，含 markdown 图片抽取 + ModerationSubmission 构造 + pipeline 评估。
//!
//! 与 `live_pipeline.rs` 的区别：
//! - 不用全局 `shared_pipeline()`（避免污染 OnceLock 单例）
//! - 直接构造 SiteConfig + 自己拼 pipeline，可在单进程多场景之间切换
//! - 默认 disabled 路径 / URL 黑名单路径 / 文本 LLM 路径 / 多模态路径都覆盖
//!
//! 需要 LLM 的用例标 `#[ignore]`，可单独跑：
//! ```sh
//! cargo test -p module-moderation --test hook_e2e \
//!   -- --ignored --nocapture --test-threads=1
//! ```

use std::sync::Arc;

use app_core::settings::{ModerationSettings, SiteConfig};
use llm::{default_client_from_env, LlmClient};
use module_moderation::{
  absolutize_image_url, extract_image_urls, ModerationLabel, ModerationPipeline,
};
use sdk::{ImageRef, ModerationSubmission};

fn workspace_root() -> std::path::PathBuf {
  std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn load_env() {
  let _ = dotenvy::from_path(workspace_root().join(".env"));
}

/// 64×64 纯色 PNG（中立内容）。内联成 data URL：模型服务下载不了 Wikimedia 的图
/// （`invalid_image_url`）。
const NEUTRAL_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAT0lEQVR42u3PQQkAAAgEsAtvFWuZxQi+hcEKLNP1WgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELgutj3HhYLf1FgAAAABJRU5ErkJggg==";

/// 模拟业务 hook：把评论内容跑过流水线，返回 verdict label。
async fn evaluate_comment(
  pipeline: &ModerationPipeline,
  blog_id: &str,
  body: &str,
) -> ModerationLabel {
  let base_url = std::env::var("BASE_URL").unwrap_or_default();
  let images: Vec<ImageRef> = extract_image_urls(body)
    .into_iter()
    .map(|u| ImageRef::url(absolutize_image_url(&u, &base_url)))
    .collect();
  let submission = ModerationSubmission::new(body)
    .with_kind("comment")
    .with_ref_path(format!("blog:{}", blog_id))
    .with_images(images);
  pipeline.evaluate(submission).await.label
}

// ── 默认 disabled：所有评论一律 Allow，无开销 ──

#[tokio::test]
async fn disabled_pipeline_allows_anything_including_abusive() {
  let site = SiteConfig::default(); // moderation.enabled = false
  let pipeline = ModerationPipeline::from_site_config(&site, None);
  assert!(pipeline.is_empty());

  // 不调 LLM
  assert_eq!(evaluate_comment(&pipeline, "welcome", "你这个 sb").await, ModerationLabel::Allow);
  assert_eq!(
    evaluate_comment(&pipeline, "welcome", "https://scam.example/x").await,
    ModerationLabel::Allow
  );
}

// ── URL 黑名单（无 LLM 依赖） ──

#[tokio::test]
async fn url_blocklist_only_blocks_known_bad_domains() {
  let mut site = SiteConfig::default();
  site.moderation = ModerationSettings {
    enabled: true,
    url_blocklist: vec!["scam.example".into(), "*.phishing.example".into()],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, None);

  assert_eq!(
    evaluate_comment(&pipeline, "welcome", "点 https://scam.example/x 领奖").await,
    ModerationLabel::Block
  );
  assert_eq!(
    evaluate_comment(&pipeline, "welcome", "请去 https://login.phishing.example/verify").await,
    ModerationLabel::Block
  );
  // 普通链接通过
  assert_eq!(
    evaluate_comment(&pipeline, "welcome", "see https://github.com/rust-lang/rust").await,
    ModerationLabel::Allow
  );
  // 无链接也通过（黑名单 stage 不会无中生有判 Block）
  assert_eq!(evaluate_comment(&pipeline, "welcome", "感谢分享").await, ModerationLabel::Allow);
}

// ── 完整 LLM 路径 ──

fn live_llm() -> Option<Arc<dyn LlmClient>> {
  load_env();
  default_client_from_env().map(Arc::from)
}

#[tokio::test]
#[ignore = "Live LLM."]
async fn full_pipeline_blocks_abusive_comment() {
  let Some(llm) = live_llm() else {
    eprintln!("跳过：未配置 LLM env");
    return;
  };
  let mut site = SiteConfig::default();
  site.moderation = ModerationSettings {
    enabled: true,
    llm_review: true,
    url_blocklist: vec![],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, Some(llm));

  let label = evaluate_comment(&pipeline, "welcome", "你这个 sb，写的什么垃圾").await;
  println!("[abusive-comment] label={:?}", label);
  assert_eq!(label, ModerationLabel::Block);
}

#[tokio::test]
#[ignore = "Live LLM."]
async fn full_pipeline_with_image_evaluates_vision() {
  let Some(llm) = live_llm() else {
    return;
  };
  let mut site = SiteConfig::default();
  site.moderation = ModerationSettings {
    enabled: true,
    llm_review: true,
    url_blocklist: vec![],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, Some(llm));

  // 中立图片内联为 data URL（模型服务下载不了 Wikimedia 的图，以前 fail-open 把这个
  // 错误当成了 Allow）。data URL 不经 `extract_image_urls`，直接放进 submission。
  let submission = ModerationSubmission::new("分享一张配色参考图，很简洁")
    .with_kind("comment")
    .with_ref_path("blog:welcome")
    .with_images([ImageRef::url(NEUTRAL_PNG)]);
  let v = pipeline.evaluate(submission).await;
  println!("[comment-with-image] label={:?} reason={}", v.label, v.reason);
  assert_eq!(v.label, ModerationLabel::Allow);
}
