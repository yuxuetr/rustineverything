//! 审核结果的数据类型与阈值：[`ModerationLabel`] / [`Verdict`] / [`ModerationThresholds`]。
//! 流水线本身（异步、多 stage、超时降级）在 `module-moderation`。

use serde::{Deserialize, Serialize};

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
    Self { score: 0.0, label: ModerationLabel::Allow, reason: String::new() }
  }

  pub fn block(score: f32, reason: impl Into<String>) -> Self {
    Self { score: score.clamp(0.0, 1.0), label: ModerationLabel::Block, reason: reason.into() }
  }

  pub fn flag(score: f32, reason: impl Into<String>) -> Self {
    Self { score: score.clamp(0.0, 1.0), label: ModerationLabel::Flag, reason: reason.into() }
  }

  pub fn is_block(&self) -> bool {
    self.label == ModerationLabel::Block
  }
}

/// Phase 4.1：阈值配置。pipeline 返回 Verdict 后，根据 score 升级 label：
/// - `score >= block_above` → Block
/// - `score >= flag_above`  → Flag
///
/// 否则保留原 label（不降级，避免「Stage 明确 Block 但分数低」的反直觉行为）。
///
/// 默认阈值 `block_above = 0.9 / flag_above = 0.5`，对绝大多数 LLM 审核
/// 提供商的「moderation score」语义是合理起点。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ModerationThresholds {
  pub block_above: f32,
  pub flag_above: f32,
}

impl Default for ModerationThresholds {
  fn default() -> Self {
    Self { block_above: 0.9, flag_above: 0.5 }
  }
}

impl ModerationThresholds {
  /// Schema 校验：阈值必须落在 `[0.0, 1.0]` 且 `block_above >= flag_above`
  /// （Block 比 Flag 更严格）。不允许 NaN。返回可读错误供日志/拒绝使用。
  ///
  /// 由 `ModerationPipeline::from_site_config` 在装载 `site.json` 时调用：
  /// 非法配置回退默认值并告警，避免「block < flag」这类把所有命中都判成
  /// Block 的反直觉行为。
  pub fn validate(&self) -> Result<(), String> {
    for (name, v) in [("block_above", self.block_above), ("flag_above", self.flag_above)] {
      if v.is_nan() {
        return Err(format!("{name} 不能是 NaN"));
      }
      if !(0.0..=1.0).contains(&v) {
        return Err(format!("{name} 必须在 [0.0, 1.0] 之间，当前为 {v}"));
      }
    }
    if self.block_above < self.flag_above {
      return Err(format!(
        "block_above ({}) 不能小于 flag_above ({})：Block 应比 Flag 更严格",
        self.block_above, self.flag_above
      ));
    }
    Ok(())
  }

  /// 把 verdict 按阈值「升级」：score >= block_above 强制 Block；
  /// score >= flag_above 强制 Flag。已是 Block 的不降级；Allow 的也按
  /// score 检查是否需要升级。
  pub fn apply(&self, mut v: Verdict) -> Verdict {
    if v.label == ModerationLabel::Block {
      return v;
    }
    if v.score >= self.block_above {
      v.label = ModerationLabel::Block;
    } else if v.score >= self.flag_above {
      v.label = ModerationLabel::Flag;
    }
    v
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn verdict_constructors_clamp_score() {
    let block = Verdict::block(2.0, "off the scale");
    assert!((block.score - 1.0).abs() < f32::EPSILON);

    let flag = Verdict::flag(-0.5, "negative");
    assert!((flag.score - 0.0).abs() < f32::EPSILON);
  }

  // ────────────────────────────────────────────────────────
  // Phase 4.1：阈值（block_above / flag_above）
  // ────────────────────────────────────────────────────────

  #[test]
  fn thresholds_default_is_0_9_block_0_5_flag() {
    let t = ModerationThresholds::default();
    assert!((t.block_above - 0.9).abs() < f32::EPSILON);
    assert!((t.flag_above - 0.5).abs() < f32::EPSILON);
  }

  #[test]
  fn thresholds_validate_accepts_sane_values() {
    assert!(ModerationThresholds::default().validate().is_ok());
    assert!(ModerationThresholds { block_above: 1.0, flag_above: 0.0 }.validate().is_ok());
    assert!(ModerationThresholds { block_above: 0.5, flag_above: 0.5 }.validate().is_ok());
  }

  #[test]
  fn thresholds_validate_rejects_out_of_range() {
    assert!(ModerationThresholds { block_above: 1.5, flag_above: 0.5 }.validate().is_err());
    assert!(ModerationThresholds { block_above: 0.9, flag_above: -0.1 }.validate().is_err());
  }

  #[test]
  fn thresholds_validate_rejects_block_below_flag() {
    let err = ModerationThresholds { block_above: 0.3, flag_above: 0.7 }.validate().unwrap_err();
    assert!(err.contains("block_above"));
  }

  #[test]
  fn thresholds_validate_rejects_nan() {
    assert!(ModerationThresholds { block_above: f32::NAN, flag_above: 0.5 }.validate().is_err());
  }

  #[test]
  fn threshold_upgrades_allow_to_flag() {
    let t = ModerationThresholds::default();
    // Allow with score=0.6 → 升级为 Flag
    let v = t.apply(Verdict { score: 0.6, label: ModerationLabel::Allow, reason: String::new() });
    assert_eq!(v.label, ModerationLabel::Flag);
  }

  #[test]
  fn threshold_upgrades_allow_to_block() {
    let t = ModerationThresholds::default();
    let v = t.apply(Verdict { score: 0.95, label: ModerationLabel::Allow, reason: String::new() });
    assert_eq!(v.label, ModerationLabel::Block);
  }

  #[test]
  fn threshold_upgrades_flag_to_block() {
    let t = ModerationThresholds::default();
    let v = t.apply(Verdict::flag(0.95, "high"));
    assert_eq!(v.label, ModerationLabel::Block);
  }

  #[test]
  fn threshold_never_downgrades_block() {
    // 已是 Block 的，即使 score 低于 block_above，仍保持 Block。
    let t = ModerationThresholds { block_above: 0.9, flag_above: 0.5 };
    let v = t.apply(Verdict::block(0.2, "explicit"));
    assert_eq!(v.label, ModerationLabel::Block);
  }

  #[test]
  fn engine_custom_thresholds_change_behavior() {
    // 默认阈值会把 score=0.6 升级为 Flag；提高阈值后保持 Allow。
    let lenient = ModerationThresholds { block_above: 0.99, flag_above: 0.95 };
    let v =
      lenient.apply(Verdict { score: 0.6, label: ModerationLabel::Allow, reason: String::new() });
    assert_eq!(v.label, ModerationLabel::Allow);
  }
}
