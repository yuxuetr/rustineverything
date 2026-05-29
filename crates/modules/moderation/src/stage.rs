//! 异步审核 stage trait。LLM 调用必须 async，所以与 core 中的同步
//! [`app_core::engines::moderation::ModerationStage`] 平行存在。

use async_trait::async_trait;

use app_core::engines::moderation::Verdict;
use sdk::ModerationSubmission;

/// 异步 stage。流水线串行调用，可早停于 Block。
#[async_trait]
pub trait AsyncModerationStage: Send + Sync {
  fn name(&self) -> &str;

  /// 评估一条 submission。失败时实现方应自行 fail-open 返回 Allow + 写日志，
  /// 让流水线不被任一 stage 的故障阻塞用户提交。
  async fn evaluate(&self, submission: &ModerationSubmission) -> Verdict;
}

#[cfg(test)]
mod tests {
  use super::*;
  use app_core::engines::moderation::ModerationLabel;

  struct FixedStage(&'static str, Verdict);

  #[async_trait]
  impl AsyncModerationStage for FixedStage {
    fn name(&self) -> &str {
      self.0
    }
    async fn evaluate(&self, _submission: &ModerationSubmission) -> Verdict {
      self.1.clone()
    }
  }

  #[tokio::test]
  async fn fixed_stage_returns_its_verdict() {
    let s = FixedStage("test", Verdict::flag(0.7, "suspicious"));
    let v = s.evaluate(&ModerationSubmission::new("hi")).await;
    assert_eq!(v.label, ModerationLabel::Flag);
    assert_eq!(v.reason, "suspicious");
  }

  #[tokio::test]
  async fn stage_name_is_accessible() {
    let s = FixedStage("keyword", Verdict::allow());
    assert_eq!(s.name(), "keyword");
  }
}
