//! [`ModerationEngine`] 骨架（Phase 1C.4）。
//!
//! 完整审核流水线（LLM/VLM 多 stage、超时降级、阈值配置）在 Phase 4
//! 落地。当前阶段定义最小接口：
//! - [`ModerationLabel`] / [`Verdict`]：审核结果数据类型。
//! - [`ModerationStage`] trait：单个审核阶段。
//! - [`ModerationEngine`]：保存 Vec<Box<dyn ModerationStage>>，串行调用 +
//!   早停（任一 Block 立即停止）。
//!
//! 此处接口为 `evaluate(submission: &str) -> Verdict`（同步 + 字符串），
//! 在 Phase 4 落地 LLM 调用时会改为 async + Submission 结构体（含图片 URL）。

use serde::{Deserialize, Serialize};

use super::{Engine, EngineContext};
use crate::error::AppResult;

/// 审核结果分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModerationLabel {
    /// 允许通过。
    Allow,
    /// 标记并入队人工复核（admin 见到，但用户已发表）。
    Flag,
    /// 完全阻止（用户提交被拒绝）。
    Block,
}

/// 审核结论：含分数、标签、可读理由。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verdict {
    /// 0.0 ~ 1.0 的违规倾向打分。供阈值过滤使用。
    pub score: f32,
    /// 决定的分类。
    pub label: ModerationLabel,
    /// 给用户/管理员看到的简短理由。
    pub reason: String,
}

impl Verdict {
    pub fn allow() -> Self {
        Self {
            score: 0.0,
            label: ModerationLabel::Allow,
            reason: String::new(),
        }
    }

    pub fn block(score: f32, reason: impl Into<String>) -> Self {
        Self {
            score: score.clamp(0.0, 1.0),
            label: ModerationLabel::Block,
            reason: reason.into(),
        }
    }

    pub fn flag(score: f32, reason: impl Into<String>) -> Self {
        Self {
            score: score.clamp(0.0, 1.0),
            label: ModerationLabel::Flag,
            reason: reason.into(),
        }
    }

    pub fn is_block(&self) -> bool {
        self.label == ModerationLabel::Block
    }
}

/// 单个审核阶段。Phase 4 落地时会拆分为 LLMStage / VLMStage / KeywordStage 等。
pub trait ModerationStage: Send + Sync {
    fn name(&self) -> &'static str;
    /// 评估某条提交（content 为纯文本）。返回该阶段的判定。
    fn evaluate(&self, content: &str) -> Verdict;
}

/// ModerationEngine：串行运行 stages，遇到 Block 立即返回。
pub struct ModerationEngine {
    stages: Vec<Box<dyn ModerationStage>>,
}

impl Default for ModerationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ModerationEngine {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn register<S: ModerationStage + 'static>(&mut self, stage: S) {
        self.stages.push(Box::new(stage));
    }

    pub fn stages(&self) -> Vec<&'static str> {
        self.stages.iter().map(|s| s.name()).collect()
    }

    /// 跑一遍流水线：任一 Block 立即返回；否则返回最后一个 stage 的判定
    /// （Allow 或最高分的 Flag）。空 stages 默认 Allow。
    pub fn evaluate(&self, content: &str) -> Verdict {
        let mut best: Option<Verdict> = None;
        for stage in &self.stages {
            let v = stage.evaluate(content);
            if v.is_block() {
                return v; // 早停
            }
            // 选择最高 score 的非 Allow 结果，便于上报
            if v.label == ModerationLabel::Flag {
                let replace = match &best {
                    Some(b) if b.score >= v.score => false,
                    _ => true,
                };
                if replace {
                    best = Some(v);
                }
            }
        }
        best.unwrap_or_else(Verdict::allow)
    }
}

impl Engine for ModerationEngine {
    fn name(&self) -> &'static str {
        "moderation"
    }

    fn init(&mut self, _ctx: &EngineContext) -> AppResult<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 假阶段：固定返回某个判定。
    struct FixedStage(&'static str, Verdict);
    impl ModerationStage for FixedStage {
        fn name(&self) -> &'static str {
            self.0
        }
        fn evaluate(&self, _content: &str) -> Verdict {
            self.1.clone()
        }
    }

    #[test]
    fn engine_name_is_moderation() {
        let e = ModerationEngine::new();
        assert_eq!(<ModerationEngine as Engine>::name(&e), "moderation");
    }

    #[test]
    fn empty_pipeline_allows_everything() {
        let e = ModerationEngine::new();
        let v = e.evaluate("anything");
        assert_eq!(v.label, ModerationLabel::Allow);
        assert_eq!(v.score, 0.0);
    }

    #[test]
    fn block_short_circuits() {
        let mut e = ModerationEngine::new();
        e.register(FixedStage("a", Verdict::block(0.95, "spam")));
        e.register(FixedStage(
            "b",
            Verdict::flag(0.5, "should not be reached"),
        ));
        let v = e.evaluate("test");
        assert_eq!(v.label, ModerationLabel::Block);
        assert_eq!(v.reason, "spam");
    }

    #[test]
    fn flag_takes_highest_score() {
        let mut e = ModerationEngine::new();
        e.register(FixedStage("a", Verdict::flag(0.3, "low")));
        e.register(FixedStage("b", Verdict::flag(0.8, "high")));
        e.register(FixedStage("c", Verdict::flag(0.5, "mid")));
        let v = e.evaluate("test");
        assert_eq!(v.label, ModerationLabel::Flag);
        assert_eq!(v.reason, "high");
    }

    #[test]
    fn all_allow_returns_allow() {
        let mut e = ModerationEngine::new();
        e.register(FixedStage("a", Verdict::allow()));
        e.register(FixedStage("b", Verdict::allow()));
        let v = e.evaluate("test");
        assert_eq!(v.label, ModerationLabel::Allow);
    }

    #[test]
    fn verdict_constructors_clamp_score() {
        let block = Verdict::block(2.0, "off the scale");
        assert!((block.score - 1.0).abs() < f32::EPSILON);

        let flag = Verdict::flag(-0.5, "negative");
        assert!((flag.score - 0.0).abs() < f32::EPSILON);
    }
}
