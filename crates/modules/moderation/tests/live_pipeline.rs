#![allow(clippy::field_reassign_with_default)] // 测试里 Default + 逐字段赋值更易读
//! 端到端集成测试：真实 wasm 插件 + 真实 LLM 端点（DeepSeek）。
//!
//! 默认 `#[ignore]`，仅当显式指定 `--ignored` 时运行：
//! ```sh
//! cargo test --features server -p rustineverything-module-moderation \
//!   --test live_pipeline -- --ignored --nocapture --test-threads=1
//! ```
//!
//! 要求：
//! - `.env` 已配 `OPENAI_LLM_BASE_URL` / `OPENAI_LLM_API_KEY`（或 Anthropic 那一对）
//! - `assets/plugins/plugin_moderation_deepseek.wasm` 已构建（见
//!   `examples/plugin-moderation-deepseek/src/lib.rs` 顶部构建命令）
//!
//! 失败任一前置条件 → 测试 early return 跳过。

use std::path::PathBuf;
use std::sync::Arc;

use rustineverything_core::settings::{ModerationSettings, SiteConfig};
use rustineverything_llm::{default_client_from_env, LlmConfig};
use rustineverything_module_moderation::{
  AsyncModerationStage, ModerationLabel, ModerationPipeline, PluginModerationStage,
};
use rustineverything_sdk::{ImageRef, ModerationSubmission};

fn workspace_root() -> PathBuf {
  // crates/modules/moderation/ → 上溯 3 级
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| PathBuf::from("."))
}

fn plugin_path() -> PathBuf {
  workspace_root().join("assets/plugins/plugin_moderation_deepseek.wasm")
}

fn load_env() {
  let _ = dotenvy::from_path(workspace_root().join(".env"));
}

fn check_prereqs() -> Option<Arc<dyn rustineverything_llm::LlmClient>> {
  load_env();
  if !plugin_path().exists() {
    eprintln!(
      "跳过：插件 wasm 未构建。请先 `cargo build -p plugin-moderation-deepseek \
       --target wasm32-unknown-unknown --release && cp /Users/hal/.target/\
       wasm32-unknown-unknown/release/plugin_moderation_deepseek.wasm \
       assets/plugins/`"
    );
    return None;
  }
  let cfg = LlmConfig::from_env();
  if cfg.resolved_provider().is_none() {
    eprintln!("跳过：未配置任一 LLM provider（OPENAI_LLM_* 或 ANTHROPIC_LLM_*）");
    return None;
  }
  // Box<dyn> → Arc<dyn>
  default_client_from_env().map(Arc::from)
}

#[tokio::test]
#[ignore = "Live network + wasm. Run with --ignored."]
async fn benign_comment_returns_allow() {
  let Some(llm) = check_prereqs() else {
    return;
  };
  let stage = PluginModerationStage::new("moderation-deepseek", plugin_path(), llm);
  let v = stage
    .evaluate(
      &ModerationSubmission::new("感谢分享，这篇博客写得很清晰，期待下一篇。")
        .with_kind("comment")
        .with_ref_path("blog/welcome"),
    )
    .await;

  println!("[benign] label={:?} score={} reason={}", v.label, v.score, v.reason);
  // 友善评论应当 allow；阈值升级后也不应是 Block
  assert_eq!(v.label, ModerationLabel::Allow, "正常内容不应被拒绝: {:?}", v);
}

#[tokio::test]
#[ignore = "Live network + wasm. Run with --ignored."]
async fn abusive_comment_is_flagged_or_blocked() {
  let Some(llm) = check_prereqs() else {
    return;
  };
  let stage = PluginModerationStage::new("moderation-deepseek", plugin_path(), llm);

  // 明显的辱骂内容
  let v = stage
    .evaluate(
      &ModerationSubmission::new("你这个 sb，写的什么垃圾文章，你妈死了吗").with_kind("comment"),
    )
    .await;

  println!("[abusive] label={:?} score={} reason={}", v.label, v.score, v.reason);
  // 不强行断言 Block，因为 LLM 输出有随机性；但至少要 Flag 以上
  assert_ne!(v.label, ModerationLabel::Allow, "明显辱骂应至少 Flag: {:?}", v);
}

/// URL 黑名单：完全本地，无 LLM 依赖。即使 `.env` 没配 LLM 也应当通过。
#[tokio::test]
#[ignore = "Pipeline integration test (no network). Run with --ignored."]
async fn url_blocklist_pipeline_blocks_scam_link() {
  load_env(); // 不需要 LLM，但保持一致

  let mut site = SiteConfig::default();
  site.moderation = ModerationSettings {
    enabled: true,
    plugins: vec![],
    url_blocklist: vec!["scam.example".to_string(), "*.phishing.example".to_string()],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(
    &site,
    workspace_root().join("assets/plugins").as_path(),
    None,
  );

  // 命中精确域名
  let v = pipeline.evaluate(ModerationSubmission::new("点 https://scam.example/x 领奖")).await;
  println!("[url-block exact] label={:?} reason={}", v.label, v.reason);
  assert_eq!(v.label, ModerationLabel::Block);
  assert!(v.reason.contains("scam.example"));

  // 命中通配子域
  let v = pipeline
    .evaluate(ModerationSubmission::new("https://login.phishing.example/verify 紧急确认"))
    .await;
  println!("[url-block wildcard] label={:?} reason={}", v.label, v.reason);
  assert_eq!(v.label, ModerationLabel::Block);

  // 干净链接通过
  let v =
    pipeline.evaluate(ModerationSubmission::new("see https://github.com/rust-lang/rust")).await;
  assert_eq!(v.label, ModerationLabel::Allow);
}

#[tokio::test]
#[ignore = "Live vision LLM + wasm. Requires gpt-4o-mini / claude-3.5 etc."]
async fn comment_with_benign_image_returns_allow() {
  let Some(llm) = check_prereqs() else {
    return;
  };
  let stage = PluginModerationStage::new("moderation-deepseek", plugin_path(), llm);

  // 用 Wikipedia 公开 logo（中立内容）。LLM provider 服务器侧 fetch。
  // 若用 DeepSeek 之类不带视觉的端点，会返回错误 → stage fail-open 为 Allow。
  let url = "https://upload.wikimedia.org/wikipedia/commons/thumb/d/d5/Rust_programming_language_black_logo.svg/240px-Rust_programming_language_black_logo.svg.png";

  let v = stage
    .evaluate(
      &ModerationSubmission::new("分享一张 Rust 的 logo")
        .with_kind("comment")
        .push_image(ImageRef::url(url).with_media_type("image/png")),
    )
    .await;

  println!("[benign+image] label={:?} score={} reason={}", v.label, v.score, v.reason);
  assert_eq!(v.label, ModerationLabel::Allow, "中立图片不应被拒: {:?}", v);
}

/// `extract_image_urls` 在真实 markdown 评论上正确抽取站内 + 外站图片。
#[test]
fn extract_image_urls_from_realistic_comment() {
  use rustineverything_module_moderation::extract_image_urls;
  let body = r#"我在博客里看到了这张图：

![截图](/uploads/abc.png)

还有这张外站的：![meme](https://example.com/meme.jpg "好笑")

最后是个普通链接 [跳转](https://other.example/page) — 不是图。
"#;
  let urls = extract_image_urls(body);
  assert_eq!(urls, vec!["/uploads/abc.png", "https://example.com/meme.jpg"]);
}

/// 链接上下文进入 LLM prompt：评论里带可疑域名（仿冒 paypal），让模型
/// 通过 prompt 中的 `[包含链接: ...]` 标签做风险判定。
#[tokio::test]
#[ignore = "Live LLM + wasm. Run with --ignored."]
async fn phishing_link_context_flags_or_blocks_via_llm() {
  let Some(llm) = check_prereqs() else {
    return;
  };
  let stage = PluginModerationStage::new("moderation-deepseek", plugin_path(), llm);

  // 仿冒 PayPal：domain 拼写仿冒 + 诱导话术
  let v = stage
    .evaluate(
      &ModerationSubmission::new(
        "您的 PayPal 账户已被冻结，请立即登录 https://paypa1-security.com/verify 解冻",
      )
      .with_kind("comment"),
    )
    .await;

  println!("[phishing] label={:?} score={} reason={}", v.label, v.score, v.reason);
  assert_ne!(v.label, ModerationLabel::Allow, "钓鱼仿冒域名应至少 Flag: {:?}", v);
}
