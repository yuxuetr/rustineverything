//! [`ModerationPipeline`]：串行跑多个 [`AsyncModerationStage`]，应用阈值，
//! 早停于 Block。
//!
//! 行为与 `crates/core/src/engines/moderation.rs::ModerationEngine` 同义但
//! **异步**，能容纳 LLM 调用。两个引擎可以共存：sync 用于关键词规则，async
//! 用于 LLM。最终业务层在提交路径上调用 [`ModerationPipeline::evaluate`]。
//!
//! ## 默认安全
//! - `from_site_config` 读 `site.json::moderation`：
//!   - `enabled = false` → 返回空 pipeline，evaluate 总是 Allow
//!   - `llm_review = true` 但没有 LLM 配置（env 未设）→ 跳过 LLM stage（无法发请求）
//! - LLM 审核失败按 `on_llm_failure` 记为 Flag（默认，送人工复核）或 Block，
//!   不放行；空 stages 返回 Allow

use std::sync::Arc;

use app_core::engines::moderation::{ModerationLabel, ModerationThresholds, Verdict};
use app_core::settings::SiteConfig;
use llm::LlmClient;
use sdk::ModerationSubmission;

use crate::stage::AsyncModerationStage;
use crate::{LlmModerationStage, UrlBlocklistStage};

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
    Self { stages: Vec::new(), thresholds: ModerationThresholds::default() }
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

  /// 从 site.json + 一个共享 LlmClient 构造 pipeline。
  /// 默认 disabled / 未开 LLM 审核 / 缺 LLM 配置都安全返回（可能为空的）流水线。
  pub fn from_site_config(site: &SiteConfig, llm: Option<Arc<dyn LlmClient>>) -> Self {
    let mut pipeline = Self::new();

    // 阈值覆盖（site.json 中可选）。schema 校验失败 → 回退默认 + 告警，
    // 避免 block < flag / 越界值导致的反直觉判定。
    if let Some(cfg) = &site.moderation.thresholds {
      let mut t = ModerationThresholds::default();
      if let Some(v) = cfg.block_above {
        t.block_above = v;
      }
      if let Some(v) = cfg.flag_above {
        t.flag_above = v;
      }
      match t.validate() {
        Ok(()) => pipeline.thresholds = t,
        Err(e) => {
          tracing::warn!(
            error = %e,
            "moderation: site.json 阈值非法，回退默认 (block 0.9 / flag 0.5)"
          );
        }
      }
    }

    if !site.moderation.enabled {
      tracing::info!("moderation: disabled in site.json → empty pipeline");
      return pipeline;
    }

    // ── Layer 1：URL 黑名单（先注册，跑得最快，命中就早停） ──
    let blocklist = UrlBlocklistStage::new(site.moderation.url_blocklist.iter().cloned());
    if !blocklist.is_empty() {
      tracing::info!(
        patterns = blocklist.patterns().len(),
        "moderation: registered url-blocklist stage"
      );
      pipeline.register(blocklist);
    }

    // ── Layer 2：LLM 审核 ──
    if !site.moderation.llm_review {
      if pipeline.is_empty() {
        tracing::info!("moderation: enabled but no LLM review / no URL blocklist → empty pipeline");
      } else {
        tracing::info!("moderation: enabled with URL blocklist only (no LLM review)");
      }
      return pipeline;
    }
    let Some(llm) = llm else {
      if pipeline.is_empty() {
        tracing::warn!(
          "moderation: enabled but no LLM configured (env OPENAI_LLM_* / ANTHROPIC_LLM_* 都未设) 且无 URL blocklist → empty pipeline"
        );
      } else {
        tracing::warn!("moderation: LLM 未配置 → 只跑 URL 黑名单，跳过 LLM 审核");
      }
      return pipeline;
    };
    tracing::info!(provider = ?llm.provider(), "moderation: registered llm stage");
    pipeline.register(LlmModerationStage::new(llm, site.moderation.on_llm_failure));

    pipeline
  }

  /// 跑流水线。每个 stage 的结论先按阈值升级，Block 早停；否则 Flag 优先于
  /// Allow，同 label 取最高分。空 stages 总是 Allow。
  pub async fn evaluate(&self, submission: ModerationSubmission) -> Verdict {
    let mut best = Verdict::allow();
    for stage in &self.stages {
      let v = self.thresholds.apply(stage.evaluate(&submission).await);
      if v.label == ModerationLabel::Block {
        return v;
      }
      // 先比 label：审核失败的 Flag 分数为 0，不能输给前面 stage 的 Allow
      let is_flag = |x: &Verdict| x.label == ModerationLabel::Flag;
      if (is_flag(&v), v.score) > (is_flag(&best), best.score) {
        best = v;
      }
    }
    best
  }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // 测试 setup：Default + 逐字段赋值更易读
mod tests {
  use super::*;
  use app_core::settings::{ModerationSettings, ModerationThresholdsConfig};
  use async_trait::async_trait;

  struct StubStage(&'static str, Verdict);

  /// 每次调用都失败的 LLM。
  struct NoLlm;

  #[async_trait]
  impl LlmClient for NoLlm {
    fn provider(&self) -> llm::LlmProvider {
      llm::LlmProvider::OpenAi
    }
    async fn chat(&self, _: Vec<llm::LlmMessage>) -> app_core::error::AppResult<String> {
      Err(app_core::error::AppError::other("LLM down"))
    }
  }

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
  async fn zero_score_flag_beats_earlier_allow() {
    // 审核失败的 Flag 没有分数；不能因为先跑的 stage 给了 Allow(0.0) 就被吞掉
    let mut p = ModerationPipeline::new();
    p.register(StubStage("a", Verdict::allow()));
    p.register(StubStage("b", Verdict::flag(0.0, "review")));
    let v = p.evaluate(ModerationSubmission::new("x")).await;
    assert_eq!((v.label, v.reason.as_str()), (ModerationLabel::Flag, "review"));
  }

  #[tokio::test]
  async fn thresholds_upgrade_allow_to_block() {
    let mut p = ModerationPipeline::new()
      .with_thresholds(ModerationThresholds { block_above: 0.9, flag_above: 0.5 });
    p.register(StubStage(
      "a",
      Verdict { score: 0.95, label: ModerationLabel::Allow, reason: "weak signal".to_string() },
    ));
    let v = p.evaluate(ModerationSubmission::new("x")).await;
    assert_eq!(v.label, ModerationLabel::Block);
  }

  // ── site.json driven construction ──────────────────────────

  #[test]
  fn disabled_in_site_config_yields_empty_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings { enabled: false, llm_review: true, ..Default::default() };
    let p = ModerationPipeline::from_site_config(&site, None);
    assert!(p.is_empty());
  }

  #[test]
  fn enabled_without_llm_review_yields_empty_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings { enabled: true, ..Default::default() };
    let p = ModerationPipeline::from_site_config(&site, None);
    assert!(p.is_empty());
  }

  #[test]
  fn llm_review_without_llm_client_yields_empty_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings { enabled: true, llm_review: true, ..Default::default() };
    // 没有 LLM 客户端就无法审核 → URL blocklist 也为空 → 整体空 pipeline
    let p = ModerationPipeline::from_site_config(&site, None);
    assert!(p.is_empty());
  }

  #[test]
  fn url_blocklist_only_yields_one_stage_no_llm_required() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: true,
      url_blocklist: vec!["scam.com".to_string()],
      ..Default::default()
    };
    // 没传 llm，但 URL 黑名单不依赖 LLM
    let p = ModerationPipeline::from_site_config(&site, None);
    assert!(!p.is_empty());
    assert_eq!(p.stage_names(), vec!["url-blocklist".to_string()]);
  }

  #[tokio::test]
  async fn url_blocklist_blocks_via_pipeline() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: true,
      url_blocklist: vec!["scam.com".to_string()],
      ..Default::default()
    };
    let p = ModerationPipeline::from_site_config(&site, None);
    let v = p.evaluate(ModerationSubmission::new("点 https://scam.com/x 拿福利")).await;
    assert_eq!(v.label, ModerationLabel::Block);
  }

  #[test]
  fn url_blocklist_runs_before_llm() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: true,
      llm_review: true,
      url_blocklist: vec!["scam.com".to_string()],
      ..Default::default()
    };
    // LLM 未配置：URL 黑名单照样跑
    let p = ModerationPipeline::from_site_config(&site, None);
    assert_eq!(p.stage_names(), vec!["url-blocklist"]);
    // LLM 已配置：黑名单在前，LLM 在后
    let p = ModerationPipeline::from_site_config(&site, Some(Arc::new(NoLlm)));
    assert_eq!(p.stage_names(), vec!["url-blocklist", "llm"]);
  }

  #[tokio::test]
  async fn llm_failure_is_not_allowed_through() {
    use app_core::settings::LlmFailureAction;
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: true,
      llm_review: true,
      url_blocklist: vec!["scam.com".to_string()],
      ..Default::default()
    };
    let p = ModerationPipeline::from_site_config(&site, Some(Arc::new(NoLlm)));
    let v = p.evaluate(ModerationSubmission::new("正常评论")).await;
    assert_eq!(v.label, ModerationLabel::Flag, "默认送人工复核");

    site.moderation.on_llm_failure = LlmFailureAction::Reject;
    let p = ModerationPipeline::from_site_config(&site, Some(Arc::new(NoLlm)));
    let v = p.evaluate(ModerationSubmission::new("正常评论")).await;
    assert_eq!(v.label, ModerationLabel::Block);
  }

  #[test]
  fn llm_stage_needs_llm_review_switch() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings { enabled: true, ..Default::default() };
    let p = ModerationPipeline::from_site_config(&site, Some(Arc::new(NoLlm)));
    assert!(p.is_empty(), "配了 LLM 但没开 llm_review 不应调用 LLM");
  }

  #[test]
  fn thresholds_partial_override_keeps_other_default() {
    let mut site = SiteConfig::default();
    site.moderation = ModerationSettings {
      enabled: false,
      thresholds: Some(ModerationThresholdsConfig {
        block_above: Some(0.75),
        flag_above: None, // 保留默认 0.5
      }),
      ..Default::default()
    };
    let p = ModerationPipeline::from_site_config(&site, None);
    assert!((p.thresholds.block_above - 0.75).abs() < f32::EPSILON);
    assert!((p.thresholds.flag_above - 0.5).abs() < f32::EPSILON);
  }
}
