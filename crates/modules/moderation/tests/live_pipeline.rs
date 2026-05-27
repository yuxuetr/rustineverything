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

use rustineverything_llm::{default_client_from_env, LlmConfig};
use rustineverything_module_moderation::{
  AsyncModerationStage, ModerationLabel, PluginModerationStage,
};
use rustineverything_sdk::ModerationSubmission;

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
  match default_client_from_env() {
    Some(b) => Some(Arc::from(b)),
    None => None,
  }
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

  println!(
    "[benign] label={:?} score={} reason={}",
    v.label, v.score, v.reason
  );
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
      &ModerationSubmission::new("你这个 sb，写的什么垃圾文章，你妈死了吗")
        .with_kind("comment"),
    )
    .await;

  println!(
    "[abusive] label={:?} score={} reason={}",
    v.label, v.score, v.reason
  );
  // 不强行断言 Block，因为 LLM 输出有随机性；但至少要 Flag 以上
  assert_ne!(
    v.label,
    ModerationLabel::Allow,
    "明显辱骂应至少 Flag: {:?}",
    v
  );
}
