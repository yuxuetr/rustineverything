//! OpenAI 兼容协议客户端。
//!
//! 适用大部分主流 vendor（OpenAI 本家 / DeepSeek / Moonshot / Qwen /
//! Zhipu / Together / Groq / Ollama 等）。
//!
//! ## 端点
//! `POST {base_url}/v1/chat/completions`
//!
//! ## 请求
//! ```json
//! {
//!   "model": "deepseek-chat",
//!   "messages": [
//!     {"role": "system", "content": "..."},
//!     {"role": "user", "content": "..."}
//!   ]
//! }
//! ```
//! Header：`Authorization: Bearer <api_key>`, `Content-Type: application/json`.
//!
//! ## 响应
//! ```json
//! { "choices": [{"message": {"role": "assistant", "content": "..."}}] }
//! ```

#![cfg(feature = "server")]

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{config::DEFAULT_TIMEOUT_SECS, LlmClient, LlmMessage, LlmProvider, LlmRole};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct OpenAiChat {
  base_url: String,
  api_key: String,
  model: String,
  client: Client,
}

impl OpenAiChat {
  /// 构造一个新客户端。`base_url` 不应含尾斜杠（构造时自动 trim）。
  pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
    let base_url = base_url.into().trim_end_matches('/').to_string();
    // 项目约定（CLAUDE.md::Rust HTTP Testing）：测试场景需用 `.no_proxy()`
    // 避免本地代理拦截 loopback；生产环境保持默认 client 行为。
    // 这里在测试时由 [`Self::with_client`] 注入 `.no_proxy()` 的 client。
    let client = Client::builder()
      .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
      .build()
      .unwrap_or_else(|_| Client::new());
    Self {
      base_url,
      api_key: api_key.into(),
      model: model.into(),
      client,
    }
  }

  /// 注入自定义 [`reqwest::Client`]。供测试用 `.no_proxy()` 构造，避免
  /// macOS 系统代理把 127.0.0.1 转给 Clash 等代理黑洞。
  pub fn with_client(mut self, client: Client) -> Self {
    self.client = client;
    self
  }

  fn endpoint(&self) -> String {
    format!("{}/v1/chat/completions", self.base_url)
  }
}

// ────────────────────────────────────────────────────────────
// JSON wire 类型。仅本文件内使用。
// ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest<'a> {
  model: &'a str,
  messages: &'a [WireMessage<'a>],
}

#[derive(Serialize)]
struct WireMessage<'a> {
  role: &'a str,
  content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
  #[serde(default)]
  choices: Vec<Choice>,
  #[serde(default)]
  error: Option<ApiError>,
}

#[derive(Deserialize)]
struct Choice {
  message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
  #[serde(default)]
  content: String,
}

#[derive(Deserialize)]
struct ApiError {
  #[serde(default)]
  message: String,
  #[serde(default, rename = "type")]
  kind: String,
}

#[async_trait]
impl LlmClient for OpenAiChat {
  fn provider(&self) -> LlmProvider {
    LlmProvider::OpenAi
  }

  async fn chat(&self, messages: Vec<LlmMessage>) -> AppResult<String> {
    let wire: Vec<WireMessage> = messages
      .iter()
      .map(|m| WireMessage {
        role: m.role.as_str(),
        content: &m.content,
      })
      .collect();

    // role 校验：OpenAI 协议拒绝空 messages，提前给可读错误
    if wire.is_empty() {
      return Err(AppError::validation("LLM 请求 messages 不能为空"));
    }
    // 角色覆盖检查；assistant-only 在 chat completions 中没意义
    let has_meaningful_input = messages
      .iter()
      .any(|m| matches!(m.role, LlmRole::System | LlmRole::User));
    if !has_meaningful_input {
      return Err(AppError::validation(
        "LLM 请求 messages 至少需要包含 system 或 user 角色",
      ));
    }

    let body = ChatRequest {
      model: &self.model,
      messages: &wire,
    };
    let url = self.endpoint();

    tracing::debug!(provider = "openai", url = %url, model = %self.model, "llm: chat request");

    let resp = self
      .client
      .post(&url)
      .bearer_auth(&self.api_key)
      .header("Content-Type", "application/json")
      .json(&body)
      .send()
      .await
      .map_err(|e| AppError::other(format!("LLM HTTP 请求失败: {}", e)))?;

    let status = resp.status();
    let text = resp
      .text()
      .await
      .map_err(|e| AppError::other(format!("LLM 响应读取失败: {}", e)))?;

    if !status.is_success() {
      tracing::warn!(provider = "openai", status = %status, body = %text, "llm: non-2xx response");
      return Err(AppError::other(format!(
        "LLM 服务返回 {}: {}",
        status,
        truncate(&text, 500)
      )));
    }

    let parsed: ChatResponse = serde_json::from_str(&text)
      .map_err(|e| AppError::other(format!("LLM 响应不是合法 JSON: {} (body={})", e, truncate(&text, 200))))?;

    if let Some(err) = parsed.error {
      return Err(AppError::other(format!(
        "LLM 服务返回错误（{}）: {}",
        err.kind, err.message
      )));
    }

    let answer = parsed
      .choices
      .into_iter()
      .next()
      .map(|c| c.message.content)
      .unwrap_or_default();

    if answer.is_empty() {
      return Err(AppError::other(
        "LLM 服务返回空 content（choices 为空或 message.content 为空）".to_string(),
      ));
    }

    tracing::debug!(provider = "openai", len = answer.len(), "llm: chat ok");
    Ok(answer)
  }
}

fn truncate(s: &str, max: usize) -> String {
  if s.len() <= max {
    s.to_string()
  } else {
    let mut end = max;
    while !s.is_char_boundary(end) {
      end -= 1;
    }
    format!("{}…", &s[..end])
  }
}

// ────────────────────────────────────────────────────────────
// 单测：mockito round-trip
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  fn test_client() -> Client {
    // 与项目约定一致：本地 mock 必须显式禁用系统代理。
    Client::builder().no_proxy().build().expect("test client")
  }

  fn build_chat(base_url: &str) -> OpenAiChat {
    OpenAiChat::new(base_url, "sk-test", "deepseek-chat").with_client(test_client())
  }

  #[tokio::test]
  async fn chat_round_trip_returns_message_content() {
    let mut server = mockito::Server::new_async().await;
    let body = r#"{
      "choices": [
        {"message": {"role": "assistant", "content": "你好，我是 DeepSeek。"}}
      ]
    }"#;
    let mock = server
      .mock("POST", "/v1/chat/completions")
      .match_header("authorization", "Bearer sk-test")
      .match_header("content-type", "application/json")
      .with_status(200)
      .with_header("content-type", "application/json")
      .with_body(body)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let answer = client
      .chat(vec![LlmMessage::user("你好")])
      .await
      .expect("chat ok");
    assert_eq!(answer, "你好，我是 DeepSeek。");
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn endpoint_trims_trailing_slash() {
    let client = OpenAiChat::new("https://api.deepseek.com/", "k", "m");
    assert_eq!(client.endpoint(), "https://api.deepseek.com/v1/chat/completions");
  }

  #[tokio::test]
  async fn request_body_includes_model_and_messages() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/chat/completions")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"model\":\"deepseek-chat\"".to_string()),
        mockito::Matcher::Regex("\"role\":\"system\"".to_string()),
        mockito::Matcher::Regex("\"role\":\"user\"".to_string()),
        mockito::Matcher::Regex("\"content\":\"你是审核员\"".to_string()),
      ]))
      .with_status(200)
      .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client
      .chat(vec![
        LlmMessage::system("你是审核员"),
        LlmMessage::user("hello"),
      ])
      .await
      .unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn non_2xx_returns_error_with_body_excerpt() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
      .mock("POST", "/v1/chat/completions")
      .with_status(401)
      .with_body(r#"{"error":{"message":"Invalid auth","type":"authentication_error"}}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let err = client.chat(vec![LlmMessage::user("x")]).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("401"), "expected status in error: {}", msg);
  }

  #[tokio::test]
  async fn api_error_in_2xx_envelope_surfaces() {
    // 某些 OpenAI 兼容厂商对鉴权失败会用 200 + error 字段
    let mut server = mockito::Server::new_async().await;
    let _m = server
      .mock("POST", "/v1/chat/completions")
      .with_status(200)
      .with_body(r#"{"error":{"message":"quota exceeded","type":"insufficient_quota"}}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let err = client.chat(vec![LlmMessage::user("x")]).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("quota"));
    assert!(msg.contains("insufficient_quota"));
  }

  #[tokio::test]
  async fn empty_messages_returns_validation_error() {
    let client = build_chat("http://unreachable.invalid");
    let err = client.chat(vec![]).await.unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
  }

  #[tokio::test]
  async fn assistant_only_messages_rejected() {
    let client = build_chat("http://unreachable.invalid");
    let err = client
      .chat(vec![LlmMessage::assistant("我已经回答过")])
      .await
      .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
  }

  #[test]
  fn truncate_keeps_short_strings() {
    assert_eq!(truncate("hi", 10), "hi");
  }

  #[test]
  fn truncate_respects_char_boundary() {
    // 中文 3 字节，截断点应该向左滚到字符边界
    let s = "你好世界";
    let out = truncate(s, 4); // 落在 "你" 中间字节
    assert!(out.ends_with('…'));
    assert!(out.starts_with('你'));
  }
}
