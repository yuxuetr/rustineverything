#![allow(clippy::field_reassign_with_default)] // 测试里 Default + 逐字段赋值更易读
//! 端到端集成测试：内置 LLM 审核 stage + 真实 LLM 端点（如 DeepSeek）。
//!
//! 默认 `#[ignore]`，仅当显式指定 `--ignored` 时运行：
//! ```sh
//! cargo test --features server -p module-moderation \
//!   --test live_pipeline -- --ignored --nocapture --test-threads=1
//! ```
//!
//! 要求：
//! - `.env` 已配 `OPENAI_LLM_BASE_URL` / `OPENAI_LLM_API_KEY`（或 Anthropic 那一对）
//!
//! 未配置 → 测试 early return 跳过。

use std::path::PathBuf;
use std::sync::Arc;

use app_core::settings::{LlmFailureAction, ModerationSettings, SiteConfig};
use llm::{default_client_from_env, AnthropicChat, LlmConfig, OpenAiChat};
use module_moderation::{
  AsyncModerationStage, LlmModerationStage, ModerationLabel, ModerationPipeline,
};
use sdk::{ImageRef, ModerationSubmission};

/// 64×64 纯色 PNG（中立内容）。内联成 data URL：模型服务下载不了 Wikimedia 的图
/// （`invalid_image_url`），以前 fail-open 把这个错误当成了 Allow。
const NEUTRAL_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAT0lEQVR42u3PQQkAAAgEsAtvFWuZxQi+hcEKLNP1WgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELgutj3HhYLf1FgAAAABJRU5ErkJggg==";

fn workspace_root() -> PathBuf {
  // crates/modules/moderation/ → 上溯 3 级
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| PathBuf::from("."))
}

fn load_env() {
  let _ = dotenvy::from_path(workspace_root().join(".env"));
}

fn check_prereqs() -> Option<Arc<dyn llm::LlmClient>> {
  load_env();
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
  let stage = LlmModerationStage::new(llm, LlmFailureAction::Review);
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
  let stage = LlmModerationStage::new(llm, LlmFailureAction::Review);

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
    url_blocklist: vec!["scam.example".to_string(), "*.phishing.example".to_string()],
    ..Default::default()
  };
  let pipeline = ModerationPipeline::from_site_config(&site, None);

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
  let stage = LlmModerationStage::new(llm, LlmFailureAction::Review);

  // 端点不支持图片时会返回错误 → stage 记为 Flag（送人工复核），本测试会失败。
  // 2026-10-29：deepseek-flash（V4.1）支持图片，base64 与公网 URL 都能读。

  let v = stage
    .evaluate(
      &ModerationSubmission::new("分享一张配色参考图")
        .with_kind("comment")
        .push_image(ImageRef::url(NEUTRAL_PNG).with_media_type("image/png")),
    )
    .await;

  println!("[benign+image] label={:?} score={} reason={}", v.label, v.score, v.reason);
  assert_eq!(v.label, ModerationLabel::Allow, "中立图片不应被拒: {:?}", v);
}

/// tests/fixtures 下的 PNG 内联成 data URL（上传审核走的就是这条路）。
fn fixture_png(name: &str) -> ImageRef {
  use base64::Engine as _;
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
  let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
  let data = base64::engine::general_purpose::STANDARD.encode(bytes);
  ImageRef::url(format!("data:image/png;base64,{data}")).with_media_type("image/png")
}

/// 两种协议各建一个客户端（都配置了才各跑一遍）：图片在两个协议里的序列化方式不同。
fn clients_per_protocol() -> Vec<(&'static str, Arc<dyn llm::LlmClient>)> {
  load_env();
  let cfg = LlmConfig::from_env();
  let mut out: Vec<(&'static str, Arc<dyn llm::LlmClient>)> = Vec::new();
  if let (Some(url), Some(key)) = (cfg.openai_base_url.as_deref(), cfg.openai_api_key.as_deref()) {
    let model = cfg.openai_model.clone().unwrap_or_else(|| "deepseek-chat".to_string());
    out.push(("openai", Arc::new(OpenAiChat::new(url, key, &model))));
  }
  if let (Some(url), Some(key)) =
    (cfg.anthropic_base_url.as_deref(), cfg.anthropic_api_key.as_deref())
  {
    let model = cfg.anthropic_model.clone().unwrap_or_else(|| "deepseek-chat".to_string());
    out.push(("anthropic", Arc::new(AnthropicChat::new(url, key, &model))));
  }
  out
}

/// 违规只在图片里：正文正常，图片是刷单广告 / 钓鱼 / 辱骂文字；另有只传图片的上传场景。
/// 图片是渲染出来的文字图，不含真实的色情或血腥内容。
#[tokio::test]
#[ignore = "Live vision LLM. Run with --ignored."]
async fn violations_carried_only_by_images_are_caught() {
  let clients = clients_per_protocol();
  if clients.is_empty() {
    eprintln!("跳过：未配置任一 LLM provider（OPENAI_LLM_* 或 ANTHROPIC_LLM_*）");
    return;
  }

  // (正文, 场景, 图片, 应当放行)
  let cases = [
    ("分享一下我的第一个 Rust 程序", "comment", "code.png", true),
    ("用户上传的图片", "upload", "code.png", true),
    ("大家看看这个", "comment", "spam.png", false),
    ("大家看看这个", "comment", "phish.png", false),
    ("大家看看这个", "topic", "insult.png", false),
    ("用户上传的图片", "upload", "spam.png", false),
    ("用户上传的图片", "upload", "insult.png", false),
  ];
  let mut wrong = Vec::new();
  for (protocol, llm) in clients {
    let stage = LlmModerationStage::new(llm, LlmFailureAction::Review);
    for (content, kind, image, allow) in cases {
      let v = stage
        .evaluate(
          &ModerationSubmission::new(content).with_kind(kind).push_image(fixture_png(image)),
        )
        .await;
      println!(
        "[{protocol} {kind} {image}] label={:?} score={} reason={}",
        v.label, v.score, v.reason
      );
      if (v.label == ModerationLabel::Allow) != allow {
        wrong.push(format!("{protocol} {kind} {image}: {:?} {}", v.label, v.reason));
      }
    }
  }
  assert!(wrong.is_empty(), "判错: {wrong:?}");
}

/// `extract_image_urls` 在真实 markdown 评论上正确抽取站内 + 外站图片。
#[test]
fn extract_image_urls_from_realistic_comment() {
  use module_moderation::extract_image_urls;
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
  let stage = LlmModerationStage::new(llm, LlmFailureAction::Review);

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
