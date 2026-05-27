//! [`ModerationPipeline`]：串行跑多个 [`AsyncModerationStage`]，应用阈值，
//! 早停于 Block。
//!
//! 行为与 `crates/core/src/engines/moderation.rs::ModerationEngine` 同义但
//! **异步**，能容纳 LLM 调用。两个引擎可以共存：sync 用于关键词规则，async
//! 用于 LLM。最终业务层在提交路径上调用 [`ModerationPipeline::evaluate`]。
//!
//! ## 默认安全
//! - `from_site_config` 读 `site.json::moderation`：
//!   - `enabled = false` 或 `plugins` 为空 → 返回空 pipeline，evaluate 总是 Allow
//!   - 任一插件文件不存在 → 跳过该 stage，记 warning（不阻塞启动）
//!   - 没有 LLM 配置（env 未设）→ 跳过全部 plugin stage（无法发请求）
//! - `evaluate` 上的每个 stage 自己 fail-open；pipeline 也对空 stages 返回 Allow

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustineverything_core::engines::moderation::{
  ModerationLabel, ModerationThresholds, Verdict,
};
use rustineverything_core::settings::SiteConfig;
use rustineverything_llm::LlmClient;
use rustineverything_sdk::ModerationSubmission;

use crate::stage::AsyncModerationStage;
use crate::PluginModerationStage;

pub struct ModerationPipeline {
  stages: Vec<Box<dyn AsyncModerationStage>>,
  thresholds: ModerationThresholds,
}

impl Default for ModerationPipeline {
  fn default() -> Self {
    Self::new()
  }
}

impl ModerationPipeline {
  pub fn new() -> Self {
    Self {
      stages: Vec::new(),
      thresholds: ModerationThresholds::default(),
    }
  }

  pub fn with_thresholds(mut self, t: ModerationThresholds) -> Self {
    self.thresholds = t;
    self
  }

  pub fn register<S: AsyncModerationStage + 'static>(&mut self, stage: S) {
    self.stages.push(Box::new(stage));
  }

  pub fn stage_names(&self) -> Vec<String> {
    self.stages.iter().map(|s| s.name().to_string()).collect()
  }

  pub fn is_empty(&self) -> bool {
    self.stages.is_empty()
  }

  /// 从 site.json + assets 路径 + 一个共享 LlmClient 构造 pipeline。
  /// 默认 disabled / 空 plugin 列表 / 文件缺失等场景都安全返回空流水线。
  pub fn from_site_config(
    site: &SiteConfig,
    plugin_dir: &Path,
    llm: Option<Arc<dyn LlmClient>>,
  ) -> Self {
    let mut pipeline = Self::new();

    // 阈值覆盖（site.json 中可选）
    if let Some(cfg) = &site.moderation.thresholds {
      let mut t = ModerationThresholds::default();
      if let Some(v) = cfg.block_above {
        t.block_above = v;
      }
      if let Some(v) = cfg.flag_above {
        t.flag_above = v;
      }
      pipeline.thresholds = t;
    }

    if !site.moderation.enabled {
      tracing::info!("moderation: disabled in site.json → empty pipeline");
      return pipeline;
    }
    if site.moderation.plugins.is_empty() {
      tracing::info!("moderation: enabled but no plugins configured → empty pipeline");
      return pipeline;
    }
    let Some(llm) = llm else {
      tracing::warn!(
        "moderation: enabled but no LLM configured (env OPENAI_LLM_* / ANTHROPIC_LLM_* 都未设) → empty pipeline"
      );
      return pipeline;
    };

    for plugin_name in &site.moderation.plugins {
      let path: PathBuf = plugin_dir.join(plugin_name);
      if !path.exists() {
        tracing::warn!(plugin = %plugin_name, "moderation: plugin file missing → skipping");
        continue;
      }
      let stage = PluginModerationStage::new(plugin_name.clone(), path, llm.clone());
      tracing::info!(stage = %plugin_name, "moderation: registered plugin stage");
      pipeline.register(stage);
    }

    pipeline
  }

  /// 跑流水线。Block 早停；否则返回最高分的非 Allow（或 Allow，如果所有 stage 都 Allow）。
  /// 最后统一应用阈值。空 stages 总是 Allow。
  pub async fn evaluate(&self, submission: ModerationSubmission) -> Verdict {
    if self.stages.is_empty() {
      return Verdict::allow();
    }
    let mut best: Option<Verdict> = None;
    for stage in &self.stages {
      let v = stage.evaluate(&submission).await;
      if v.label == ModerationLabel::Block {
        // 阈值升级一遍即返回（Block 不会被降级）
        return self.thresholds.apply(v);
      }
      // 取最高 score 的非 Allow
      let replace = match &best {
        Some(b) if b.score >= v.score => false,
        _ => true,
      };
      if replace {
        best = Some(v);
      }
    }
    self.thresholds.apply(best.unwrap_or_else(Verdict::allow))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use async_trait::async_trait;
  use rustineverything_core::settings::{ModerationSettings, ModerationThresholdsConfig};

  struct StubStage(&'static str, Verdict);

  #[async_trait]
  impl AsyncModerationStage for StubStage {
    fn name(&self) -> &str {
      self.0
    }
    async fn evaluate(&self, _: &ModerationSubmission) -> Verdict {
      self.1.clone()
    }
  }

  #[tokio::test]
  async fn empty_pipeline_returns_allow() {
    let p = ModerationPipeline::new();
    let v = p.evaluate(ModerationSubmission::new("x")).await;
    assert_eq!(v.label, ModerationLabel::Allow);
  }

  #[tokio::test]
  async fn block_short_circuits() {
    let mut p = ModerationPipeline::new();
    p.register(StubStage("a", Verdict::block(0.95, "spam")));
    p.register(StubStage("b", Verdict::flag(0.5, "should not reach")));
    let v = p.evaluate(ModerationSubmission::new("x")).await;
    assert_eq!(v.label, ModerationLabel::Block);
    assert_eq!(v.reason, "spam");
  }

  #[tokio::test]
  async fn highest_flag_wins() {
    let mut p = ModerationPipeline::new();
    p.register(StubStage("a", Verdict::flag(0.3, "low")));
    p.register(StubStage("b", Verdict::flag(0.8, "high")));
    p.register(StubStage("c", Verdict::flag(0.5, "mid")));
    let v = p.evaluate(ModerationSubmission::new("x")).await;
    assert_eq!(v.label, ModerationLabel::Flag);
    assert_eq!(v.reason, "high");
  }

  #[tokio::test]
  async fn thresholds_upgrade_allow_to_block() {
    let mut p = ModerationPipeline::new().with_thresholds(ModerationThresholds {
      block_above: 0.9,
      flag_above: 0.5,
    });
    p.register(StubStage("a", Verdict {
      score: 0.95,
      label: ModerationLabel::Allow,
      reason: "weak signal".to_string(),
    }));
    let v = p.evaluate(ModerationSubmission::new("x")).await;
    assert_eq!(v.label, ModerationLabel::Block);
  }

  // ── site.json driven construction ──────────────────────────

  #[test]
  fn disabled_in_site_config_yields_empty_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: false,
      plugins: vec!["moderation_llm_default.wasm".into()],
      thresholds: None,
    };
    let plugin_dir = std::path::PathBuf::from("/nonexistent/plugins");
    let p = ModerationPipeline::from_site_config(&site, &plugin_dir, None);
    assert!(p.is_empty());
  }

  #[test]
  fn enabled_but_no_plugins_yields_empty_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: true,
      plugins: vec![],
      thresholds: None,
    };
    let p = ModerationPipeline::from_site_config(&site, std::path::Path::new("/tmp"), None);
    assert!(p.is_empty());
  }

  #[test]
  fn enabled_with_plugins_but_no_llm_yields_empty_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: true,
      plugins: vec!["x.wasm".into()],
      thresholds: None,
    };
    // 即使插件存在，没有 LLM 也无法工作 → 安全空 pipeline
    let p = ModerationPipeline::from_site_config(&site, std::path::Path::new("/tmp"), None);
    assert!(p.is_empty());
  }

  #[test]
  fn thresholds_partial_override_keeps_other_default() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: false,
      plugins: vec![],
      thresholds: Some(ModerationThresholdsConfig {
        block_above: Some(0.75),
        flag_above: None, // 保留默认 0.5
      }),
    };
    let p = ModerationPipeline::from_site_config(&site, std::path::Path::new("/tmp"), None);
    assert!((p.thresholds.block_above - 0.75).abs() < f32::EPSILON);
    assert!((p.thresholds.flag_above - 0.5).abs() < f32::EPSILON);
  }
}
