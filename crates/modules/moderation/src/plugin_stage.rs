//! `PluginModerationStage`：把一个 WASM 插件 + 一个 LlmClient 串成一个
//! [`AsyncModerationStage`]。
//!
//! 调用顺序：
//! 1. **同步** 调插件 `moderation_build_prompt(submission_json)` 得 `Vec<LlmMessage>`
//! 2. **异步** 调 `LlmClient::chat(messages)` 得 LLM 文本
//! 3. **同步** 调插件 `moderation_parse_verdict(text)` 得 `ModerationVerdict`
//! 4. 把 SDK 的 `ModerationVerdict { label: String }` 映射为 core 的
//!    `Verdict { label: ModerationLabel }`
//!
//! 任一步骤失败 → 记 warning 日志 + 返回 [`Verdict::allow`]（fail-open），
//! 不阻塞用户提交。Block 必须由完整成功的流水线决定。

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use rustineverything_core::engines::moderation::{ModerationLabel, Verdict};
use rustineverything_core::PluginManager;
use rustineverything_llm::{LlmClient, LlmMessage};
use rustineverything_sdk::{moderation as moderation_abi, ModerationSubmission, ModerationVerdict};

use crate::stage::AsyncModerationStage;

/// 把一个 WASM 插件 + LlmClient 配成一个 stage。
///
/// 字段都 Arc / PathBuf，便于克隆放进异步任务。
pub struct PluginModerationStage {
  /// 显示名（用于日志）。从 manifest.id 推断；fallback 到文件名。
  name: String,
  /// `assets/plugins/<x>.wasm` 绝对路径
  plugin_path: PathBuf,
  /// 共享的 wasm runtime（默认 `rustineverything_core::shared_plugin_manager()`，
  /// 测试时可注入 Mock-ish 替身）
  plugin_manager: Arc<PluginManager>,
  /// 上游 LLM 客户端。`crates/llm` 已封好两种协议。
  llm: Arc<dyn LlmClient>,
}

impl PluginModerationStage {
  pub fn new(name: impl Into<String>, plugin_path: PathBuf, llm: Arc<dyn LlmClient>) -> Self {
    Self {
      name: name.into(),
      plugin_path,
      plugin_manager: Arc::new(PluginManager::new()),
      llm,
    }
  }

  /// 测试 / 共享场景：注入已存在的 PluginManager（一般是 shared singleton）。
  pub fn with_plugin_manager(mut self, m: Arc<PluginManager>) -> Self {
    self.plugin_manager = m;
    self
  }

  fn plugin_call(&self, func: &str, input: &str) -> Result<String, String> {
    self
      .plugin_manager
      .call_path_with_string(&self.plugin_path, func, input)
      .map_err(|e| e.to_string())
  }
}

/// 把 SDK 的字符串 label 映射为 core 的枚举；未知值视为 Allow。
fn map_label(s: &str) -> ModerationLabel {
  match s.trim().to_ascii_lowercase().as_str() {
    "block" => ModerationLabel::Block,
    "flag" => ModerationLabel::Flag,
    _ => ModerationLabel::Allow, // 未知值 fail-open
  }
}

/// 把 SDK Verdict 转 core Verdict。score 二次 clamp 防御。
fn into_core_verdict(v: ModerationVerdict) -> Verdict {
  Verdict {
    score: v.score.clamp(0.0, 1.0),
    label: map_label(&v.label),
    reason: v.reason,
  }
}

#[async_trait]
impl AsyncModerationStage for PluginModerationStage {
  fn name(&self) -> &str {
    &self.name
  }

  async fn evaluate(&self, submission: &ModerationSubmission) -> Verdict {
    // ── 1. 让插件构造 prompt ────────────────────────────────
    let submission_json = match serde_json::to_string(submission) {
      Ok(j) => j,
      Err(e) => {
        tracing::warn!(stage = %self.name, error = %e, "moderation: failed to serialize submission");
        return Verdict::allow();
      }
    };

    let messages_json = match self.plugin_call(moderation_abi::FN_BUILD_PROMPT, &submission_json) {
      Ok(s) => s,
      Err(e) => {
        tracing::warn!(stage = %self.name, error = %e, "moderation: plugin build_prompt failed → allow");
        return Verdict::allow();
      }
    };
    let messages: Vec<LlmMessage> = match serde_json::from_str(&messages_json) {
      Ok(m) => m,
      Err(e) => {
        tracing::warn!(stage = %self.name, error = %e, body = %messages_json, "moderation: plugin returned invalid LlmMessage JSON → allow");
        return Verdict::allow();
      }
    };
    if messages.is_empty() {
      tracing::warn!(stage = %self.name, "moderation: plugin returned 0 messages → allow");
      return Verdict::allow();
    }

    // ── 2. 宿主调 LLM ───────────────────────────────────────
    let llm_text = match self.llm.chat(messages).await {
      Ok(t) => t,
      Err(e) => {
        tracing::warn!(stage = %self.name, error = %e, "moderation: LLM call failed → allow");
        return Verdict::allow();
      }
    };

    // ── 3. 让插件解析 verdict ───────────────────────────────
    let verdict_json = match self.plugin_call(moderation_abi::FN_PARSE_VERDICT, &llm_text) {
      Ok(s) => s,
      Err(e) => {
        tracing::warn!(stage = %self.name, error = %e, "moderation: plugin parse_verdict failed → allow");
        return Verdict::allow();
      }
    };
    let sdk_verdict: ModerationVerdict = match serde_json::from_str(&verdict_json) {
      Ok(v) => v,
      Err(e) => {
        tracing::warn!(stage = %self.name, error = %e, body = %verdict_json, "moderation: plugin returned invalid Verdict JSON → allow");
        return Verdict::allow();
      }
    };

    tracing::debug!(
      stage = %self.name,
      score = sdk_verdict.score,
      label = %sdk_verdict.label,
      "moderation: stage verdict"
    );
    into_core_verdict(sdk_verdict)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use rustineverything_core::engines::moderation::ModerationLabel;
  use rustineverything_core::error::{AppError, AppResult};
  use rustineverything_llm::LlmProvider;

  // ── 工具：手写 LlmClient 替身（避免真实 HTTP / mockito 在这层） ──

  struct StubLlm {
    /// 期待返回的文本（Ok）；测异常路径时改为 None
    response: Option<String>,
  }

  #[async_trait]
  impl LlmClient for StubLlm {
    fn provider(&self) -> LlmProvider {
      LlmProvider::OpenAi
    }
    async fn chat(&self, _messages: Vec<LlmMessage>) -> AppResult<String> {
      match &self.response {
        Some(s) => Ok(s.clone()),
        None => Err(AppError::other("stub LLM forced failure")),
      }
    }
  }

  // ── 工具：手写「插件」替身（实际是个跑在 host 内的函数对） ──
  //
  // 用真实 wasm 在单测里太重，端到端验证留给 examples/plugin-moderation-*
  // 的集成测试。这层用 trait object 替代 PluginManager 的字符串调用。

  #[test]
  fn map_label_normalizes_case_and_whitespace() {
    assert_eq!(map_label("Block"), ModerationLabel::Block);
    assert_eq!(map_label("  flag  "), ModerationLabel::Flag);
    assert_eq!(map_label("ALLOW"), ModerationLabel::Allow);
    assert_eq!(map_label("garbled"), ModerationLabel::Allow); // 未知 → fail-open
    assert_eq!(map_label(""), ModerationLabel::Allow);
  }

  #[test]
  fn into_core_verdict_clamps_score() {
    let v = into_core_verdict(ModerationVerdict {
      score: 2.5,
      label: "block".to_string(),
      reason: "too much".to_string(),
    });
    assert!((v.score - 1.0).abs() < f32::EPSILON);
    assert_eq!(v.label, ModerationLabel::Block);
  }

  #[test]
  fn into_core_verdict_negative_score_clamped() {
    let v = into_core_verdict(ModerationVerdict {
      score: -0.5,
      label: "flag".to_string(),
      reason: String::new(),
    });
    assert!((v.score - 0.0).abs() < f32::EPSILON);
    assert_eq!(v.label, ModerationLabel::Flag);
  }

  #[tokio::test]
  async fn nonexistent_plugin_path_fails_open() {
    // 给一个不存在的 wasm 路径 → 第一步 build_prompt 调用就会出错，应该 fail-open
    let stage = PluginModerationStage::new(
      "nonexistent",
      PathBuf::from("/definitely/does/not/exist.wasm"),
      Arc::new(StubLlm {
        response: Some(r#"{"score":0.9,"label":"block","reason":"x"}"#.to_string()),
      }),
    );
    let v = stage
      .evaluate(&ModerationSubmission::new("anything"))
      .await;
    assert_eq!(v.label, ModerationLabel::Allow);
    assert_eq!(v.score, 0.0);
  }
}
