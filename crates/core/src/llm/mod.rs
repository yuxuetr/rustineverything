//! LLM 集成：OpenAI 兼容 + Anthropic 兼容双模式。
//!
//! ## 设计
//! - **配置驱动**：四个独立 env 变量，两两成对（OpenAI 一对 / Anthropic
//!   一对）。
//!     - `OPENAI_LLM_BASE_URL` + `OPENAI_LLM_API_KEY`
//!     - `ANTHROPIC_LLM_BASE_URL` + `ANTHROPIC_LLM_API_KEY`
//!   独立 key 让用户可以混合不同厂商（例如 OpenAI 兼容指向 DeepSeek，
//!   Anthropic 兼容指向真正的 Claude）。
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

#![cfg(feature = "server")]

pub mod anthropic;
pub mod config;
pub mod openai;

pub use anthropic::AnthropicChat;
pub use config::{LlmConfig, LlmProvider};
pub use openai::OpenAiChat;

use crate::error::AppResult;
use async_trait::async_trait;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmMessage {
  pub role: LlmRole,
  pub content: String,
}

impl LlmMessage {
  pub fn system(content: impl Into<String>) -> Self {
    Self {
      role: LlmRole::System,
      content: content.into(),
    }
  }
  pub fn user(content: impl Into<String>) -> Self {
    Self {
      role: LlmRole::User,
      content: content.into(),
    }
  }
  pub fn assistant(content: impl Into<String>) -> Self {
    Self {
      role: LlmRole::Assistant,
      content: content.into(),
    }
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
}
