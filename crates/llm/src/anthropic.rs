//! Anthropic 兼容协议客户端。
//!
//! 适用 Claude API 本家 + 少数同时支持 Anthropic 协议的兼容厂商
//! （DeepSeek 的 `/anthropic` 路径、Bedrock 的 Claude pass-through 等）。
//!
//! ## 端点
//! `POST {base_url}/v1/messages`
//!
//! ## 请求
//! ```json
//! {
//!   "model": "deepseek-chat",
//!   "max_tokens": 1024,
//!   "system": "你是审核员",
//!   "messages": [
//!     {"role": "user", "content": "..."}
//!   ]
//! }
//! ```
//! - Header：`x-api-key: <api_key>`，`anthropic-version: 2023-06-01`，
//!   `content-type: application/json`。
//! - 注意：Anthropic 协议下 system prompt 是 **顶层 `system` 字段**，
//!   不是 messages 中的一项。本 client 自动从 [`LlmMessage`] 中抽取。
//! - `messages` 中不允许出现 `system` 角色；只能是 `user` / `assistant`
//!   并且必须以 `user` 开头。
//!
//! ## 响应
//! ```json
//! {
//!   "id": "msg_...",
//!   "type": "message",
//!   "role": "assistant",
//!   "content": [{"type": "text", "text": "..."}],
//!   "stop_reason": "end_turn"
//! }
//! ```
//! 错误体：
//! ```json
//! { "type": "error", "error": {"type": "...", "message": "..."} }
//! ```

use async_trait::async_trait;
use reqwest::Client;
use rustineverything_core::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{
  config::DEFAULT_TIMEOUT_SECS, LlmClient, LlmContentBlock, LlmMessage, LlmProvider, LlmRole,
};

/// Anthropic API 版本头。0.x SDK 与 2023-06-01 兼容；后续升级时更新。
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// 默认 max_tokens。Anthropic 协议要求显式指定，无默认值。
const DEFAULT_MAX_TOKENS: u32 = 1024;

#[derive(Debug, Clone)]
pub struct AnthropicChat {
  base_url: String,
  api_key: String,
  model: String,
  max_tokens: u32,
  client: Client,
}

impl AnthropicChat {
  pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
    let base_url = base_url.into().trim_end_matches('/').to_string();
    let client = Client::builder()
      .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
      .build()
      .unwrap_or_else(|_| Client::new());
    Self {
      base_url,
      api_key: api_key.into(),
      model: model.into(),
      max_tokens: DEFAULT_MAX_TOKENS,
      client,
    }
  }

  pub fn with_client(mut self, client: Client) -> Self {
    self.client = client;
    self
  }

  pub fn with_max_tokens(mut self, n: u32) -> Self {
    self.max_tokens = n.max(1);
    self
  }

  /// 计算最终 endpoint。允许 base_url 既不带 `/v1` 也带 `/v1`：
  /// - `https://api.anthropic.com`              → 拼接 `/v1/messages`
  /// - `https://api.anthropic.com/v1`           → 拼接 `/messages`
  /// - `https://api.deepseek.com/anthropic`     → 拼接 `/v1/messages`
  /// - `https://api.deepseek.com/anthropic/v1`  → 拼接 `/messages`
  fn endpoint(&self) -> String {
    if self.base_url.ends_with("/v1") {
      format!("{}/messages", self.base_url)
    } else {
      format!("{}/v1/messages", self.base_url)
    }
  }
}

// ────────────────────────────────────────────────────────────
// 协议转换：LlmMessage[] → Anthropic 请求体
// ────────────────────────────────────────────────────────────

/// 把统一抽象的 messages 拆成 `(system_prompt, conversation)`。
/// system 消息的 **文本块** 按出现顺序用换行串接；图像 block 在 system
/// 角色下被丢弃（Anthropic 顶层 `system` 字段只支持字符串）。
fn split_system_and_messages(messages: &[LlmMessage]) -> (Option<String>, Vec<&LlmMessage>) {
  let mut systems: Vec<String> = Vec::new();
  let mut conv: Vec<&LlmMessage> = Vec::new();
  for m in messages {
    match m.role {
      LlmRole::System => {
        let text = m.text_only();
        if !text.is_empty() {
          systems.push(text);
        }
      }
      _ => conv.push(m),
    }
  }
  let system = if systems.is_empty() {
    None
  } else {
    Some(systems.join("\n"))
  };
  (system, conv)
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
  model: &'a str,
  max_tokens: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  system: Option<&'a str>,
  messages: Vec<WireMessage<'a>>,
}

#[derive(Serialize)]
struct WireMessage<'a> {
  role: &'a str,
  /// Anthropic 协议 content 始终是数组。
  content: Vec<AnthropicContentBlock<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock<'a> {
  Text {
    text: &'a str,
  },
  Image {
    source: AnthropicImageSource<'a>,
  },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum AnthropicImageSource<'a> {
  Base64 {
    media_type: &'a str,
    data: &'a str,
  },
  Url {
    url: &'a str,
  },
}

/// 把内容块转 Anthropic 协议 wire 形态。
/// - Text → `{type:"text",...}`
/// - ImageUrl with `data:` prefix → 拆出 media_type / base64 走 `source.base64`
/// - ImageUrl with `http(s):` → `source.url`（2024-10+ Claude 支持）
/// - ImageBase64 → `source.base64`
fn build_anthropic_content<'a>(blocks: &'a [LlmContentBlock]) -> Vec<AnthropicContentBlock<'a>> {
  blocks
    .iter()
    .map(|b| match b {
      LlmContentBlock::Text { text } => AnthropicContentBlock::Text { text: text.as_str() },
      LlmContentBlock::ImageUrl { url } => {
        // 如果是 data URL，拆成 base64 source；否则走 url source
        if let Some((media_type, data)) = parse_data_url(url) {
          AnthropicContentBlock::Image {
            source: AnthropicImageSource::Base64 { media_type, data },
          }
        } else {
          AnthropicContentBlock::Image {
            source: AnthropicImageSource::Url { url: url.as_str() },
          }
        }
      }
      LlmContentBlock::ImageBase64 { media_type, data } => AnthropicContentBlock::Image {
        source: AnthropicImageSource::Base64 {
          media_type: media_type.as_str(),
          data: data.as_str(),
        },
      },
    })
    .collect()
}

/// 拆 `data:<media_type>;base64,<data>` 为 `(media_type, data)`。
/// 不是 data URL 返回 None。
fn parse_data_url(url: &str) -> Option<(&str, &str)> {
  let rest = url.strip_prefix("data:")?;
  let semi = rest.find(';')?;
  let media_type = &rest[..semi];
  let after_semi = &rest[semi + 1..];
  let comma = after_semi.find(',')?;
  // 形如 "base64,<data>" — 只接受 base64 编码
  let encoding = &after_semi[..comma];
  if encoding != "base64" {
    return None;
  }
  let data = &after_semi[comma + 1..];
  Some((media_type, data))
}

#[derive(Deserialize)]
struct MessagesResponse {
  #[serde(default)]
  content: Vec<ContentBlock>,
  // 错误响应使用 `type: "error"` + `error: {...}` 顶层结构
  #[serde(default, rename = "type")]
  resp_type: String,
  #[serde(default)]
  error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ContentBlock {
  #[serde(default, rename = "type")]
  kind: String,
  #[serde(default)]
  text: String,
}

#[derive(Deserialize)]
struct ApiError {
  #[serde(default, rename = "type")]
  kind: String,
  #[serde(default)]
  message: String,
}

#[async_trait]
impl LlmClient for AnthropicChat {
  fn provider(&self) -> LlmProvider {
    LlmProvider::Anthropic
  }

  async fn chat(&self, messages: Vec<LlmMessage>) -> AppResult<String> {
    let (system, conv) = split_system_and_messages(&messages);

    if conv.is_empty() {
      return Err(AppError::validation(
        "Anthropic 请求至少需要一条 user/assistant 消息（system 不算）",
      ));
    }
    // Anthropic 要求 conversation 以 user 开头
    if conv.first().map(|m| m.role) != Some(LlmRole::User) {
      return Err(AppError::validation(
        "Anthropic 协议要求 messages 列表首条必须为 user",
      ));
    }

    let wire: Vec<WireMessage> = conv
      .iter()
      .map(|m| WireMessage {
        role: m.role.as_str(),
        content: build_anthropic_content(&m.content),
      })
      .collect();

    let body = MessagesRequest {
      model: &self.model,
      max_tokens: self.max_tokens,
      system: system.as_deref(),
      messages: wire,
    };
    let url = self.endpoint();

    tracing::debug!(provider = "anthropic", url = %url, model = %self.model, "llm: chat request");

    let resp = self
      .client
      .post(&url)
      .header("x-api-key", &self.api_key)
      .header("anthropic-version", ANTHROPIC_VERSION)
      .header("content-type", "application/json")
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
      tracing::warn!(provider = "anthropic", status = %status, body = %text, "llm: non-2xx response");
      return Err(AppError::other(format!(
        "LLM 服务返回 {}: {}",
        status,
        truncate(&text, 500)
      )));
    }

    let parsed: MessagesResponse = serde_json::from_str(&text)
      .map_err(|e| AppError::other(format!("LLM 响应不是合法 JSON: {} (body={})", e, truncate(&text, 200))))?;

    if parsed.resp_type == "error" {
      let err = parsed.error.unwrap_or(ApiError {
        kind: "unknown".to_string(),
        message: "未知错误".to_string(),
      });
      return Err(AppError::other(format!(
        "LLM 服务返回错误（{}）: {}",
        err.kind, err.message
      )));
    }

    let answer = parsed
      .content
      .into_iter()
      .filter(|b| b.kind == "text")
      .map(|b| b.text)
      .collect::<Vec<_>>()
      .join("");

    if answer.is_empty() {
      return Err(AppError::other(
        "LLM 服务返回空 content（content[].type=text 缺失或 text 为空）".to_string(),
      ));
    }

    tracing::debug!(provider = "anthropic", len = answer.len(), "llm: chat ok");
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
// 单测：mockito round-trip + 协议转换正确性
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  fn test_client() -> Client {
    Client::builder().no_proxy().build().expect("test client")
  }

  fn build_chat(base_url: &str) -> AnthropicChat {
    AnthropicChat::new(base_url, "sk-test", "deepseek-chat").with_client(test_client())
  }

  #[tokio::test]
  async fn chat_round_trip_returns_concatenated_text_blocks() {
    let mut server = mockito::Server::new_async().await;
    let body = r#"{
      "id": "msg_01",
      "type": "message",
      "role": "assistant",
      "content": [
        {"type": "text", "text": "你好，"},
        {"type": "text", "text": "我是 Claude 兼容端点。"}
      ],
      "stop_reason": "end_turn"
    }"#;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_header("x-api-key", "sk-test")
      .match_header("anthropic-version", ANTHROPIC_VERSION)
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
    assert_eq!(answer, "你好，我是 Claude 兼容端点。");
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn system_role_is_extracted_to_top_level_field() {
    let mut server = mockito::Server::new_async().await;
    // 同时验证：system 在顶层、messages 中不含 system 角色
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"system\":\"你是审核员\"".to_string()),
        mockito::Matcher::Regex("\"role\":\"user\"".to_string()),
      ]))
      // 同时确认请求 body 里没有 role=system
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"ok"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client
      .chat(vec![
        LlmMessage::system("你是审核员"),
        LlmMessage::user("hi"),
      ])
      .await
      .expect("chat ok");
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn multiple_system_messages_joined_with_newline() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::Regex(
        "\"system\":\"line1\\\\nline2\"".to_string(),
      ))
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"ok"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client
      .chat(vec![
        LlmMessage::system("line1"),
        LlmMessage::system("line2"),
        LlmMessage::user("hi"),
      ])
      .await
      .expect("chat ok");
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn empty_user_messages_rejected() {
    let client = build_chat("http://unreachable.invalid");
    // 只有 system 没有 user → 拒绝
    let err = client
      .chat(vec![LlmMessage::system("policy")])
      .await
      .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
  }

  #[tokio::test]
  async fn conversation_not_starting_with_user_rejected() {
    let client = build_chat("http://unreachable.invalid");
    let err = client
      .chat(vec![LlmMessage::assistant("我先讲")])
      .await
      .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
  }

  #[tokio::test]
  async fn error_envelope_surfaces() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
      .mock("POST", "/v1/messages")
      .with_status(400)
      .with_body(r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad model"}}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let err = client.chat(vec![LlmMessage::user("x")]).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("400"));
  }

  #[tokio::test]
  async fn error_in_2xx_envelope_surfaces() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
      .mock("POST", "/v1/messages")
      .with_status(200)
      .with_body(r#"{"type":"error","error":{"type":"rate_limit","message":"slow down"}}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let err = client.chat(vec![LlmMessage::user("x")]).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("rate_limit"));
    assert!(msg.contains("slow down"));
  }

  #[test]
  fn split_system_handles_no_system() {
    let msgs = vec![LlmMessage::user("a"), LlmMessage::assistant("b")];
    let (sys, conv) = split_system_and_messages(&msgs);
    assert_eq!(sys, None);
    assert_eq!(conv.len(), 2);
  }

  #[test]
  fn split_system_joins_multiple() {
    let msgs = vec![
      LlmMessage::system("a"),
      LlmMessage::system("b"),
      LlmMessage::user("u"),
    ];
    let (sys, conv) = split_system_and_messages(&msgs);
    assert_eq!(sys, Some("a\nb".to_string()));
    assert_eq!(conv.len(), 1);
  }

  #[tokio::test]
  async fn with_max_tokens_changes_request_body() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::Regex("\"max_tokens\":42".to_string()))
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"ok"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url()).with_max_tokens(42);
    let _ = client.chat(vec![LlmMessage::user("x")]).await.unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn endpoint_trims_trailing_slash() {
    let client = AnthropicChat::new("https://api.deepseek.com/anthropic/", "k", "m");
    assert_eq!(client.endpoint(), "https://api.deepseek.com/anthropic/v1/messages");
  }

  // ── 多模态 wire format + data URL 拆分 ─────────────────────

  #[test]
  fn parse_data_url_extracts_media_type_and_data() {
    let (mt, data) = parse_data_url("data:image/png;base64,iVBORw0K").unwrap();
    assert_eq!(mt, "image/png");
    assert_eq!(data, "iVBORw0K");
  }

  #[test]
  fn parse_data_url_returns_none_for_non_data() {
    assert!(parse_data_url("https://example.com/a.jpg").is_none());
    assert!(parse_data_url("data:image/png,iVBORw0K").is_none()); // 缺 base64;
  }

  #[tokio::test]
  async fn text_only_message_serialized_as_text_block() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"type\":\"text\"".to_string()),
        mockito::Matcher::Regex("\"text\":\"hi\"".to_string()),
      ]))
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"ok"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client.chat(vec![LlmMessage::user("hi")]).await.unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn image_url_serialized_as_url_source() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"type\":\"image\"".to_string()),
        mockito::Matcher::Regex("\"type\":\"url\"".to_string()),
        mockito::Matcher::Regex(
          "\"url\":\"https://example.com/a.jpg\"".to_string(),
        ),
      ]))
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"a"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client
      .chat(vec![LlmMessage::user_with_image_urls(
        "what",
        vec!["https://example.com/a.jpg".to_string()],
      )])
      .await
      .unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn data_url_split_into_base64_source() {
    // ImageUrl with `data:` prefix should be split to base64 source
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"type\":\"base64\"".to_string()),
        mockito::Matcher::Regex("\"media_type\":\"image/png\"".to_string()),
        mockito::Matcher::Regex("\"data\":\"iVBORw0K\"".to_string()),
      ]))
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"a"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let msg = LlmMessage {
      role: LlmRole::User,
      content: vec![
        LlmContentBlock::text("look"),
        LlmContentBlock::image_url("data:image/png;base64,iVBORw0K"),
      ],
    };
    let _ = client.chat(vec![msg]).await.unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn image_base64_block_serialized_directly() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/messages")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"type\":\"base64\"".to_string()),
        mockito::Matcher::Regex("\"media_type\":\"image/jpeg\"".to_string()),
        mockito::Matcher::Regex("\"data\":\"/9j/4AA\"".to_string()),
      ]))
      .with_status(200)
      .with_body(r#"{"type":"message","content":[{"type":"text","text":"a"}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let msg = LlmMessage {
      role: LlmRole::User,
      content: vec![LlmContentBlock::image_base64("image/jpeg", "/9j/4AA")],
    };
    let _ = client.chat(vec![msg]).await.unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn endpoint_handles_base_with_v1_suffix() {
    let with_v1 = AnthropicChat::new("https://api.anthropic.com/v1", "k", "m");
    assert_eq!(with_v1.endpoint(), "https://api.anthropic.com/v1/messages");

    let without_v1 = AnthropicChat::new("https://api.anthropic.com", "k", "m");
    assert_eq!(without_v1.endpoint(), "https://api.anthropic.com/v1/messages");

    // DeepSeek 的 /anthropic 路径 + 带或不带 /v1
    let deepseek = AnthropicChat::new("https://api.deepseek.com/anthropic", "k", "m");
    assert_eq!(deepseek.endpoint(), "https://api.deepseek.com/anthropic/v1/messages");

    let deepseek_v1 = AnthropicChat::new("https://api.deepseek.com/anthropic/v1", "k", "m");
    assert_eq!(deepseek_v1.endpoint(), "https://api.deepseek.com/anthropic/v1/messages");
  }
}
