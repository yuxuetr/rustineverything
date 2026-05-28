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
//! cargo test -p rustineverything-module-moderation --test hook_e2e \
//!   -- --ignored --nocapture --test-threads=1
//! ```

use std::sync::Arc;

use rustineverything_core::settings::{ModerationSettings, SiteConfig};
use rustineverything_llm::{default_client_from_env, LlmClient};
use rustineverything_module_moderation::{
  absolutize_image_url, extract_image_urls, ModerationLabel, ModerationPipeline,
};
use rustineverything_sdk::{ImageRef, ModerationSubmission};

fn workspace_root() -> std::path::PathBuf {
  std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn plugin_dir() -> std::path::PathBuf {
  workspace_root().join("assets/plugins")
}

fn load_env() {
  let _ = dotenvy::from_path(workspace_root().join(".env"));
}

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
  let pipeline = ModerationPipeline::from_site_config(&site, &plugin_dir(), None);
  assert!(pipeline.is_empty());

  // 不调 LLM、不读插件 wasm
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
    plugins: vec![],
    url_blocklist: vec!["scam.example".into(), "*.phishing.example".into()],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, &plugin_dir(), None);

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
#[ignore = "Live LLM + wasm."]
async fn full_pipeline_blocks_abusive_comment() {
  let Some(llm) = live_llm() else {
    eprintln!("跳过：未配置 LLM env");
    return;
  };
  let mut site = SiteConfig::default();
  site.moderation = ModerationSettings {
    enabled: true,
    plugins: vec!["plugin_moderation_deepseek.wasm".into()],
    url_blocklist: vec![],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, &plugin_dir(), Some(llm));

  let label = evaluate_comment(&pipeline, "welcome", "你这个 sb，写的什么垃圾").await;
  println!("[abusive-comment] label={:?}", label);
  assert_eq!(label, ModerationLabel::Block);
}

#[tokio::test]
#[ignore = "Live LLM + wasm."]
async fn full_pipeline_with_image_evaluates_vision() {
  let Some(llm) = live_llm() else {
    return;
  };
  let mut site = SiteConfig::default();
  site.moderation = ModerationSettings {
    enabled: true,
    plugins: vec!["plugin_moderation_deepseek.wasm".into()],
    url_blocklist: vec![],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, &plugin_dir(), Some(llm));

  // 评论里夹带 Rust 公开 logo —— 中立内容
  let body = "看这张 logo：![rust](https://upload.wikimedia.org/wikipedia/commons/thumb/d/d5/Rust_programming_language_black_logo.svg/240px-Rust_programming_language_black_logo.svg.png) 很简洁";

  let label = evaluate_comment(&pipeline, "welcome", body).await;
  println!("[comment-with-image] label={:?}", label);
  assert_eq!(label, ModerationLabel::Allow);
}
