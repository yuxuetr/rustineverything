//! 真实端点 LLM 集成测试。
//!
//! 这些测试 **默认 ignored**，仅当显式指定 `--ignored` 时运行：
//! ```sh
//! cargo test --features server -p app-core -- --ignored --test-threads=1
//! ```
//!
//! ## 要求
//! 在 .env 或 shell env 中设置：
//! - `OPENAI_LLM_BASE_URL` + `OPENAI_LLM_API_KEY`（OpenAI 兼容协议）
//! - `ANTHROPIC_LLM_BASE_URL` + `ANTHROPIC_LLM_API_KEY`（Anthropic 兼容协议）
//!
//! 对 DeepSeek 来说，二者可以共用同一 key：
//! ```dotenv
//! OPENAI_LLM_BASE_URL=https://api.deepseek.com
//! OPENAI_LLM_API_KEY=sk-...
//! ANTHROPIC_LLM_BASE_URL=https://api.deepseek.com/anthropic
//! ANTHROPIC_LLM_API_KEY=sk-...
//! ```
//!
//! ## 失败语义
//! 如果对应的 env 未配置，测试 `early return` 并提示。这样可在不同 CI
//! / 本地环境下「只跑配置好的那一种」。

use llm::{AnthropicChat, LlmClient, LlmConfig, LlmMessage, LlmProvider, OpenAiChat};

fn load_env() {
  // 测试进程默认不会读 .env，显式加载一次。失败时静默（CI 也可能直接走 env）。
  let _ = dotenvy::from_path(workspace_root().join(".env"));
}

fn workspace_root() -> std::path::PathBuf {
  // CARGO_MANIFEST_DIR = crates/llm/ → 上溯 2 级
  std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[tokio::test]
#[ignore = "Live network. Run with --ignored after setting OPENAI_LLM_BASE_URL + OPENAI_LLM_API_KEY"]
async fn openai_compatible_chat_round_trip() {
  load_env();
  let cfg = LlmConfig::from_env();
  let (Some(url), Some(key)) = (cfg.openai_base_url.as_deref(), cfg.openai_api_key.as_deref())
  else {
    eprintln!("跳过 openai live test：OPENAI_LLM_BASE_URL / OPENAI_LLM_API_KEY 未配置");
    return;
  };
  let model = cfg.openai_model.clone().unwrap_or_else(|| "deepseek-chat".to_string());

  let client = OpenAiChat::new(url, key, &model);
  assert_eq!(client.provider(), LlmProvider::OpenAi);

  let reply = client
    .chat(vec![
      LlmMessage::system("你是简短回答的助手，请用一句话回答。"),
      LlmMessage::user("一加一等于几？只回答数字。"),
    ])
    .await
    .expect("OpenAI-compatible chat 应当成功");

  println!("[openai-live] reply = {}", reply);
  assert!(!reply.trim().is_empty(), "回复不应为空");
}

#[tokio::test]
#[ignore = "Live network. Run with --ignored after setting ANTHROPIC_LLM_BASE_URL + ANTHROPIC_LLM_API_KEY"]
async fn anthropic_compatible_chat_round_trip() {
  load_env();
  let cfg = LlmConfig::from_env();
  let (Some(url), Some(key)) =
    (cfg.anthropic_base_url.as_deref(), cfg.anthropic_api_key.as_deref())
  else {
    eprintln!("跳过 anthropic live test：ANTHROPIC_LLM_BASE_URL / ANTHROPIC_LLM_API_KEY 未配置");
    return;
  };
  let model = cfg.anthropic_model.clone().unwrap_or_else(|| "deepseek-chat".to_string());

  let client = AnthropicChat::new(url, key, &model);
  assert_eq!(client.provider(), LlmProvider::Anthropic);

  let reply = client
    .chat(vec![
      LlmMessage::system("你是简短回答的助手，请用一句话回答。"),
      LlmMessage::user("一加一等于几？只回答数字。"),
    ])
    .await
    .expect("Anthropic-compatible chat 应当成功");

  println!("[anthropic-live] reply = {}", reply);
  assert!(!reply.trim().is_empty(), "回复不应为空");
}
