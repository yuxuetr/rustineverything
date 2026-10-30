//! LLM 配置：从 env 读取四个变量，决定使用哪一个 provider。
//!
//! 见 [`super::default_client_from_env`] 的入口逻辑。
//!
//! 测试时绕开 env：直接构造 [`LlmConfig`] 字段后调 [`LlmConfig::build`]。

use std::env;

use super::{AnthropicChat, LlmClient, OpenAiChat};

/// 当前选定的协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
  OpenAi,
  Anthropic,
}

/// 默认模型。如未在 env 中指定，使用 DeepSeek 推荐名（OpenAI / Anthropic
/// 协议下 DeepSeek 都用同一字面值）。其它厂商接入时按需覆盖。
const DEFAULT_MODEL: &str = "deepseek-chat";

/// 默认 timeout。LLM 调用普遍较慢，但单次至多 30s；超过则上游有问题。
pub(crate) const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// 连接（含 TLS 握手）的超时：上游连不上时早点失败，不占满整个 30s。
const CONNECT_TIMEOUT_SECS: u64 = 10;

/// 调 LLM API 用的 HTTP 客户端配置：带超时，不跟随重定向。
/// API 端点不该重定向；跟过去会把 API key 带到别处（`x-api-key` 跨域也不会被去掉）。
pub(crate) fn api_client_builder() -> reqwest::ClientBuilder {
  reqwest::Client::builder()
    .connect_timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS))
    .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
    .redirect(reqwest::redirect::Policy::none())
}

/// 全部 env 字段的解析结果。trim 后的空串视为未配置。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LlmConfig {
  pub openai_base_url: Option<String>,
  pub openai_api_key: Option<String>,
  pub openai_model: Option<String>,

  pub anthropic_base_url: Option<String>,
  pub anthropic_api_key: Option<String>,
  pub anthropic_model: Option<String>,
}

impl LlmConfig {
  /// 读 env，标准化为 Option（空串 → None）。
  pub fn from_env() -> Self {
    Self {
      openai_base_url: read_nonempty("OPENAI_LLM_BASE_URL"),
      openai_api_key: read_nonempty("OPENAI_LLM_API_KEY"),
      openai_model: read_nonempty("OPENAI_LLM_MODEL"),
      anthropic_base_url: read_nonempty("ANTHROPIC_LLM_BASE_URL"),
      anthropic_api_key: read_nonempty("ANTHROPIC_LLM_API_KEY"),
      anthropic_model: read_nonempty("ANTHROPIC_LLM_MODEL"),
    }
  }

  /// 选定 provider 并构造对应客户端。优先级：OpenAI > Anthropic。
  pub fn build(&self) -> Option<Box<dyn LlmClient>> {
    if let (Some(url), Some(key)) =
      (self.openai_base_url.as_deref(), self.openai_api_key.as_deref())
    {
      let model = self.openai_model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string());
      let client = OpenAiChat::new(url, key, model);
      return Some(Box::new(client));
    }
    if let (Some(url), Some(key)) =
      (self.anthropic_base_url.as_deref(), self.anthropic_api_key.as_deref())
    {
      let model = self.anthropic_model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string());
      let client = AnthropicChat::new(url, key, model);
      return Some(Box::new(client));
    }
    None
  }

  /// 判断本配置最终会落到哪个 provider（不调用网络）。
  pub fn resolved_provider(&self) -> Option<LlmProvider> {
    if self.openai_base_url.is_some() && self.openai_api_key.is_some() {
      return Some(LlmProvider::OpenAi);
    }
    if self.anthropic_base_url.is_some() && self.anthropic_api_key.is_some() {
      return Some(LlmProvider::Anthropic);
    }
    None
  }
}

fn read_nonempty(name: &str) -> Option<String> {
  env::var(name).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
  #[tokio::test]
  async fn api_client_does_not_follow_redirects() {
    let mut server = mockito::Server::new_async().await;
    let _r = server
      .mock("POST", "/v1/chat/completions")
      .with_status(302)
      .with_header("location", "/elsewhere")
      .create_async()
      .await;
    let other = server.mock("POST", "/elsewhere").expect(0).create_async().await;
    let client = api_client_builder().no_proxy().build().expect("client");
    let resp = client
      .post(format!("{}/v1/chat/completions", server.url()))
      .header("x-api-key", "secret")
      .send()
      .await
      .expect("response");
    assert_eq!(resp.status(), 302);
    other.assert_async().await;
  }

  use super::*;

  #[test]
  fn empty_config_resolves_to_none() {
    let cfg = LlmConfig::default();
    assert_eq!(cfg.resolved_provider(), None);
    assert!(cfg.build().is_none());
  }

  #[test]
  fn only_openai_resolves_to_openai() {
    let cfg = LlmConfig {
      openai_base_url: Some("https://api.deepseek.com".into()),
      openai_api_key: Some("sk-test".into()),
      ..Default::default()
    };
    assert_eq!(cfg.resolved_provider(), Some(LlmProvider::OpenAi));
    let client = cfg.build().expect("should build openai");
    assert_eq!(client.provider(), LlmProvider::OpenAi);
  }

  #[test]
  fn only_anthropic_resolves_to_anthropic() {
    let cfg = LlmConfig {
      anthropic_base_url: Some("https://api.deepseek.com/anthropic".into()),
      anthropic_api_key: Some("sk-test".into()),
      ..Default::default()
    };
    assert_eq!(cfg.resolved_provider(), Some(LlmProvider::Anthropic));
    let client = cfg.build().expect("should build anthropic");
    assert_eq!(client.provider(), LlmProvider::Anthropic);
  }

  #[test]
  fn both_configured_prefers_openai() {
    let cfg = LlmConfig {
      openai_base_url: Some("https://o.example".into()),
      openai_api_key: Some("o-key".into()),
      anthropic_base_url: Some("https://a.example".into()),
      anthropic_api_key: Some("a-key".into()),
      ..Default::default()
    };
    assert_eq!(cfg.resolved_provider(), Some(LlmProvider::OpenAi));
  }

  #[test]
  fn base_url_without_key_does_not_resolve() {
    // 半配置（url 但缺 key）等同于未配置；防止运行时拿空 token 调上游。
    let cfg = LlmConfig { openai_base_url: Some("https://o.example".into()), ..Default::default() };
    assert_eq!(cfg.resolved_provider(), None);
    assert!(cfg.build().is_none());
  }

  #[test]
  fn key_without_base_url_does_not_resolve() {
    let cfg = LlmConfig { openai_api_key: Some("o-key".into()), ..Default::default() };
    assert_eq!(cfg.resolved_provider(), None);
  }

  #[test]
  fn whitespace_only_env_var_treated_as_empty() {
    // 仿真 `OPENAI_LLM_BASE_URL=   ` 这种粗心配置：trim 后变 None。
    let parsed = {
      let raw = "   ";
      Some(raw.trim().to_string()).filter(|s: &String| !s.is_empty())
    };
    assert_eq!(parsed, None);
  }
}
