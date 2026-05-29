//! LLM 集成：OpenAI 兼容 + Anthropic 兼容双模式。
//!
//! ## 设计
//! - **配置驱动**：四个独立 env 变量，两两成对——OpenAI 一对
//!   (`OPENAI_LLM_BASE_URL` + `OPENAI_LLM_API_KEY`) / Anthropic 一对
//!   (`ANTHROPIC_LLM_BASE_URL` + `ANTHROPIC_LLM_API_KEY`)。独立 key 让用户
//!   可以混合不同厂商（例如 OpenAI 兼容指向 DeepSeek，Anthropic 兼容指向
//!   真正的 Claude）。
//! - **优先级**：`OPENAI_LLM_BASE_URL` 非空（且 key 也非空）→ OpenAI；
//!   否则 `ANTHROPIC_LLM_BASE_URL` 非空（且 key 也非空）→ Anthropic；
//!   都未配置 → `None`。
//! - **无运行时 failover**：选定后请求只走该协议。失败原样返回错误。
//!   两个协议的请求 / 响应 shape 不同，自动切换会产生不可预期的行为。
//! - **测试兼容两端**：单测对两个 client 都用 mockito 走完整 round-trip
//!   ；集成调用方按需直接构造 [`OpenAiChat`] / [`AnthropicChat`]。
//!
//! ## DeepSeek 验证
//! DeepSeek 同时支持 OpenAI + Anthropic 协议，是双模式的天然回归靶子：
//! ```dotenv
//! OPENAI_LLM_BASE_URL=https://api.deepseek.com
//! OPENAI_LLM_API_KEY=sk-...
//! ANTHROPIC_LLM_BASE_URL=https://api.deepseek.com/anthropic
//! ANTHROPIC_LLM_API_KEY=sk-...
//! ```
//! 两侧 key 在 DeepSeek 是同一个；其它场景（OpenAI 兼容指 DeepSeek，
//! Anthropic 兼容指 Claude）可以独立。
//!
//! ## 默认模型
//! 不在 env 中显式指定时，OpenAI 走 `deepseek-chat`、Anthropic 走
//! `deepseek-chat`（DeepSeek 同名）。可分别覆盖：`OPENAI_LLM_MODEL=...`、
//! `ANTHROPIC_LLM_MODEL=...`。

pub mod anthropic;
pub mod config;
pub mod openai;

pub use anthropic::AnthropicChat;
pub use config::{LlmConfig, LlmProvider};
pub use openai::OpenAiChat;

use async_trait::async_trait;
use app_core::error::AppResult;
use serde::{Deserialize, Serialize};

/// 聊天消息中的角色。两个协议共用此抽象，序列化时分别按各自约定写入。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmRole {
  System,
  User,
  Assistant,
}

impl LlmRole {
  /// 字符串名（小写）。两个协议都用同样的字面值。
  pub fn as_str(self) -> &'static str {
    match self {
      LlmRole::System => "system",
      LlmRole::User => "user",
      LlmRole::Assistant => "assistant",
    }
  }
}

/// 多模态消息内容块。一条 [`LlmMessage`] 由若干 block 组成（最少 1 个 Text）。
///
/// 两个协议各自的 wire 序列化由 [`crate::openai`] / [`crate::anthropic`] 处理；
/// 该枚举是协议中性表示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LlmContentBlock {
  /// 纯文本
  Text { text: String },
  /// URL 图片（公网可达的 https / data URL 都可以）。
  /// - OpenAI：直接作为 `image_url` 传给上游，由上游 fetch
  /// - Anthropic：当 `url` 以 `data:` 起首时拆为 base64 source，
  ///   否则作为 url source 传给 Claude API
  ImageUrl { url: String },
  /// Base64 内联图片。两端协议都原生支持。
  /// `media_type` 形如 `"image/jpeg"` / `"image/png"`。
  ImageBase64 { media_type: String, data: String },
}

impl LlmContentBlock {
  pub fn text(s: impl Into<String>) -> Self {
    LlmContentBlock::Text { text: s.into() }
  }
  pub fn image_url(url: impl Into<String>) -> Self {
    LlmContentBlock::ImageUrl { url: url.into() }
  }
  pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
    LlmContentBlock::ImageBase64 { media_type: media_type.into(), data: data.into() }
  }

  /// 是否是文本 block。
  pub fn is_text(&self) -> bool {
    matches!(self, LlmContentBlock::Text { .. })
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LlmMessage {
  pub role: LlmRole,
  pub content: Vec<LlmContentBlock>,
}

/// 反序列化时容忍两种 content 形态：
/// - **字符串**：`{"role":"user","content":"hi"}`（老格式 / 老插件 / 大多数文档示例）
///   → 自动包装为单个 [`LlmContentBlock::Text`]
/// - **数组**：`{"role":"user","content":[{"type":"text","text":"..."}]}`（多模态）
///
/// 这保证旧插件（编译时只懂 String content）emit 的 JSON 仍能被新宿主消费。
impl<'de> serde::Deserialize<'de> for LlmMessage {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ContentRepr {
      Str(String),
      Blocks(Vec<LlmContentBlock>),
    }
    #[derive(Deserialize)]
    struct Helper {
      role: LlmRole,
      content: ContentRepr,
    }
    let h = Helper::deserialize(deserializer)?;
    let content = match h.content {
      ContentRepr::Str(s) => vec![LlmContentBlock::Text { text: s }],
      ContentRepr::Blocks(b) => b,
    };
    Ok(LlmMessage { role: h.role, content })
  }
}

impl LlmMessage {
  pub fn system(content: impl Into<String>) -> Self {
    Self { role: LlmRole::System, content: vec![LlmContentBlock::text(content)] }
  }
  pub fn user(content: impl Into<String>) -> Self {
    Self { role: LlmRole::User, content: vec![LlmContentBlock::text(content)] }
  }
  pub fn assistant(content: impl Into<String>) -> Self {
    Self { role: LlmRole::Assistant, content: vec![LlmContentBlock::text(content)] }
  }

  /// 构造一条 user 消息，附带若干图片 URL。
  /// 文本块放在最前，图像块依次追加。
  pub fn user_with_image_urls<S, I>(text: S, image_urls: I) -> Self
  where
    S: Into<String>,
    I: IntoIterator<Item = String>,
  {
    let mut content = vec![LlmContentBlock::text(text)];
    for url in image_urls {
      content.push(LlmContentBlock::image_url(url));
    }
    Self { role: LlmRole::User, content }
  }

  /// 提取所有文本块的拼接（便于 logging / 调试）。
  pub fn text_only(&self) -> String {
    self
      .content
      .iter()
      .filter_map(|b| match b {
        LlmContentBlock::Text { text } => Some(text.as_str()),
        _ => None,
      })
      .collect::<Vec<_>>()
      .join("")
  }

  /// 是否包含任意图像 block。
  pub fn has_images(&self) -> bool {
    self.content.iter().any(|b| !matches!(b, LlmContentBlock::Text { .. }))
  }
}

/// 双模式统一接口。两个实现 ([`OpenAiChat`] / [`AnthropicChat`]) 各自
/// 处理协议差异；上层只依赖该 trait。
#[async_trait]
pub trait LlmClient: Send + Sync {
  fn provider(&self) -> LlmProvider;

  /// 发起一次聊天补全。返回助手回复文本。
  async fn chat(&self, messages: Vec<LlmMessage>) -> AppResult<String>;
}

/// 从 env 构造默认客户端。`None` 表示没有任一协议被配置 — 调用方应做
/// 优雅降级（例如 LLM 审核流水线 fail-open）。
pub fn default_client_from_env() -> Option<Box<dyn LlmClient>> {
  let cfg = LlmConfig::from_env();
  cfg.build()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn role_serializes_lowercase() {
    let m = LlmMessage::user("hi");
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("\"role\":\"user\""));
  }

  #[test]
  fn role_as_str_matches_serde() {
    assert_eq!(LlmRole::System.as_str(), "system");
    assert_eq!(LlmRole::User.as_str(), "user");
    assert_eq!(LlmRole::Assistant.as_str(), "assistant");
  }

  #[test]
  fn message_constructors_set_role() {
    assert_eq!(LlmMessage::system("a").role, LlmRole::System);
    assert_eq!(LlmMessage::user("b").role, LlmRole::User);
    assert_eq!(LlmMessage::assistant("c").role, LlmRole::Assistant);
  }

  #[test]
  fn user_constructor_produces_single_text_block() {
    let m = LlmMessage::user("hi");
    assert_eq!(m.content.len(), 1);
    assert!(m.content[0].is_text());
    assert_eq!(m.text_only(), "hi");
    assert!(!m.has_images());
  }

  #[test]
  fn user_with_image_urls_appends_image_blocks() {
    let m = LlmMessage::user_with_image_urls(
      "what's in these?",
      vec!["https://x.example/a.jpg".into(), "https://x.example/b.png".into()],
    );
    assert_eq!(m.content.len(), 3);
    assert!(m.has_images());
    assert_eq!(m.text_only(), "what's in these?");
  }

  // ── 兼容老 wire format：content 是字符串 → 自动转成单 Text block ──

  #[test]
  fn deserialize_legacy_string_content_works() {
    let json = r#"{"role":"user","content":"hello"}"#;
    let m: LlmMessage = serde_json::from_str(json).unwrap();
    assert_eq!(m.role, LlmRole::User);
    assert_eq!(m.content.len(), 1);
    assert_eq!(m.text_only(), "hello");
  }

  #[test]
  fn deserialize_array_blocks_works() {
    let json = r#"{
      "role": "user",
      "content": [
        {"type": "text", "text": "describe"},
        {"type": "image_url", "url": "https://x.example/a.jpg"}
      ]
    }"#;
    let m: LlmMessage = serde_json::from_str(json).unwrap();
    assert_eq!(m.content.len(), 2);
    assert_eq!(m.text_only(), "describe");
    assert!(m.has_images());
  }

  #[test]
  fn content_block_serializes_with_tag() {
    let b = LlmContentBlock::text("hello");
    let json = serde_json::to_string(&b).unwrap();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("\"text\":\"hello\""));

    let i = LlmContentBlock::image_url("https://x.example/a.jpg");
    let json = serde_json::to_string(&i).unwrap();
    assert!(json.contains("\"type\":\"image_url\""));
    assert!(json.contains("\"url\":\"https://x.example/a.jpg\""));

    let b64 = LlmContentBlock::image_base64("image/png", "iVBOR...");
    let json = serde_json::to_string(&b64).unwrap();
    assert!(json.contains("\"type\":\"image_base64\""));
    assert!(json.contains("\"media_type\":\"image/png\""));
  }
}
