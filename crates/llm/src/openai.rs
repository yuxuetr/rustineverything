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

use async_trait::async_trait;
use reqwest::Client;
use rustineverything_core::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{
  config::DEFAULT_TIMEOUT_SECS, LlmClient, LlmContentBlock, LlmMessage, LlmProvider, LlmRole,
};

#[derive(Debug, Clone)]
pub struct OpenAiChat {
  base_url: String,
  api_key: String,
  model: String,
  client: Client,
}

impl OpenAiChat {
  /// 构造一个新客户端。`base_url` 不应含尾斜杠（构造时自动 trim）。
  pub fn new(
    base_url: impl Into<String>,
    api_key: impl Into<String>,
    model: impl Into<String>,
  ) -> Self {
    let base_url = base_url.into().trim_end_matches('/').to_string();
    // 项目约定（CLAUDE.md::Rust HTTP Testing）：测试场景需用 `.no_proxy()`
    // 避免本地代理拦截 loopback；生产环境保持默认 client 行为。
    // 这里在测试时由 [`Self::with_client`] 注入 `.no_proxy()` 的 client。
    let client = Client::builder()
      .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
      .build()
      .unwrap_or_else(|_| Client::new());
    Self { base_url, api_key: api_key.into(), model: model.into(), client }
  }

  /// 注入自定义 [`reqwest::Client`]。供测试用 `.no_proxy()` 构造，避免
  /// macOS 系统代理把 127.0.0.1 转给 Clash 等代理黑洞。
  pub fn with_client(mut self, client: Client) -> Self {
    self.client = client;
    self
  }

  /// 计算最终 endpoint。允许 base_url 既不带 `/v1` 也带 `/v1`：
  /// - `https://api.openai.com`       → 拼接 `/v1/chat/completions`
  /// - `https://api.openai.com/v1`    → 拼接 `/chat/completions`
  ///
  /// OpenAI 官方文档示例同时存在两种写法，用户两种都可能填。
  fn endpoint(&self) -> String {
    if self.base_url.ends_with("/v1") {
      format!("{}/chat/completions", self.base_url)
    } else {
      format!("{}/v1/chat/completions", self.base_url)
    }
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
  /// OpenAI 兼容协议下，content 可以是 **字符串** 或 **数组**。
  /// 只有 1 个 Text block 时我们走字符串路径（最广兼容性，
  /// 部分老 provider 不接受数组）；任何 image block 都走数组。
  content: OpenAiContent<'a>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OpenAiContent<'a> {
  Text(&'a str),
  Blocks(Vec<OpenAiContentBlock<'a>>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAiContentBlock<'a> {
  Text { text: &'a str },
  ImageUrl { image_url: OpenAiImageUrl<'a> },
}

#[derive(Serialize)]
struct OpenAiImageUrl<'a> {
  url: std::borrow::Cow<'a, str>,
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
    if messages.is_empty() {
      return Err(AppError::validation("LLM 请求 messages 不能为空"));
    }
    let has_meaningful_input =
      messages.iter().any(|m| matches!(m.role, LlmRole::System | LlmRole::User));
    if !has_meaningful_input {
      return Err(AppError::validation("LLM 请求 messages 至少需要包含 system 或 user 角色"));
    }

    let wire: Vec<WireMessage> = messages
      .iter()
      .map(|m| WireMessage { role: m.role.as_str(), content: build_openai_content(&m.content) })
      .collect();

    let body = ChatRequest { model: &self.model, messages: &wire };
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
    let text =
      resp.text().await.map_err(|e| AppError::other(format!("LLM 响应读取失败: {}", e)))?;

    if !status.is_success() {
      tracing::warn!(provider = "openai", status = %status, body = %text, "llm: non-2xx response");
      return Err(AppError::other(format!("LLM 服务返回 {}: {}", status, truncate(&text, 500))));
    }

    let parsed: ChatResponse = serde_json::from_str(&text).map_err(|e| {
      AppError::other(format!("LLM 响应不是合法 JSON: {} (body={})", e, truncate(&text, 200)))
    })?;

    if let Some(err) = parsed.error {
      return Err(AppError::other(format!("LLM 服务返回错误（{}）: {}", err.kind, err.message)));
    }

    let answer = parsed.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default();

    if answer.is_empty() {
      return Err(AppError::other(
        "LLM 服务返回空 content（choices 为空或 message.content 为空）".to_string(),
      ));
    }

    tracing::debug!(provider = "openai", len = answer.len(), "llm: chat ok");
    Ok(answer)
  }
}

/// 把统一抽象的内容块转成 OpenAI 协议的 wire 形态。
/// - 单 Text → `content: "string"`（最大兼容老 provider）
/// - 任何含图像 / 多 block → `content: [{type, ...}, ...]` 数组
/// - ImageBase64 → 转 data URL 嵌入 `image_url`
fn build_openai_content(blocks: &[LlmContentBlock]) -> OpenAiContent<'_> {
  match blocks {
    [LlmContentBlock::Text { text }] => OpenAiContent::Text(text.as_str()),
    _ => {
      let v: Vec<OpenAiContentBlock> = blocks
        .iter()
        .map(|b| match b {
          LlmContentBlock::Text { text } => OpenAiContentBlock::Text { text: text.as_str() },
          LlmContentBlock::ImageUrl { url } => OpenAiContentBlock::ImageUrl {
            image_url: OpenAiImageUrl { url: std::borrow::Cow::Borrowed(url.as_str()) },
          },
          LlmContentBlock::ImageBase64 { media_type, data } => OpenAiContentBlock::ImageUrl {
            image_url: OpenAiImageUrl {
              url: std::borrow::Cow::Owned(format!("data:{};base64,{}", media_type, data)),
            },
          },
        })
        .collect();
      OpenAiContent::Blocks(v)
    }
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
    let answer = client.chat(vec![LlmMessage::user("你好")]).await.expect("chat ok");
    assert_eq!(answer, "你好，我是 DeepSeek。");
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn endpoint_trims_trailing_slash() {
    let client = OpenAiChat::new("https://api.deepseek.com/", "k", "m");
    assert_eq!(client.endpoint(), "https://api.deepseek.com/v1/chat/completions");
  }

  #[tokio::test]
  async fn endpoint_handles_base_with_v1_suffix() {
    // OpenAI 官方文档示例同时存在 `https://api.openai.com` 和
    // `https://api.openai.com/v1` 两种写法；两者都必须工作。
    let with_v1 = OpenAiChat::new("https://api.openai.com/v1", "k", "m");
    assert_eq!(with_v1.endpoint(), "https://api.openai.com/v1/chat/completions");

    let with_v1_slash = OpenAiChat::new("https://api.openai.com/v1/", "k", "m");
    assert_eq!(with_v1_slash.endpoint(), "https://api.openai.com/v1/chat/completions");

    let without_v1 = OpenAiChat::new("https://api.openai.com", "k", "m");
    assert_eq!(without_v1.endpoint(), "https://api.openai.com/v1/chat/completions");
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
    let _ =
      client.chat(vec![LlmMessage::system("你是审核员"), LlmMessage::user("hello")]).await.unwrap();
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
    let err = client.chat(vec![LlmMessage::assistant("我已经回答过")]).await.unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
  }

  // ── 多模态 wire format ─────────────────────────────────────

  #[tokio::test]
  async fn text_only_message_still_serialized_as_string_content() {
    // 单 Text block 应当走字符串路径，确保最大 provider 兼容性。
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/chat/completions")
      .match_body(mockito::Matcher::AllOf(vec![mockito::Matcher::Regex(
        "\"content\":\"hi\"".to_string(),
      )]))
      .with_status(200)
      .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client.chat(vec![LlmMessage::user("hi")]).await.unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn message_with_image_url_serializes_as_blocks_array() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/chat/completions")
      .match_body(mockito::Matcher::AllOf(vec![
        mockito::Matcher::Regex("\"type\":\"text\"".to_string()),
        mockito::Matcher::Regex("\"text\":\"what is this\\?\"".to_string()),
        mockito::Matcher::Regex("\"type\":\"image_url\"".to_string()),
        mockito::Matcher::Regex("\"url\":\"https://example.com/a.jpg\"".to_string()),
      ]))
      .with_status(200)
      .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"a cat"}}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let _ = client
      .chat(vec![LlmMessage::user_with_image_urls(
        "what is this?",
        vec!["https://example.com/a.jpg".to_string()],
      )])
      .await
      .unwrap();
    mock.assert_async().await;
  }

  #[tokio::test]
  async fn image_base64_block_emitted_as_data_url() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
      .mock("POST", "/v1/chat/completions")
      .match_body(mockito::Matcher::Regex("\"url\":\"data:image/png;base64,iVBORw0K\"".to_string()))
      .with_status(200)
      .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"png"}}]}"#)
      .create_async()
      .await;

    let client = build_chat(&server.url());
    let msg = LlmMessage {
      role: LlmRole::User,
      content: vec![
        LlmContentBlock::text("look"),
        LlmContentBlock::image_base64("image/png", "iVBORw0K"),
      ],
    };
    let _ = client.chat(vec![msg]).await.unwrap();
    mock.assert_async().await;
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
