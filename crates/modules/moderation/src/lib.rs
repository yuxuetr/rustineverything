//! LLM 审核流水线。
//!
//! ## 架构
//! ```text
//! 用户提交内容 (评论 / 话题 / 回复 / 标注)
//!         │
//!         ▼
//! ModerationPipeline (本 crate)
//!         │
//!         ├─ UrlBlocklistStage：链接命中黑名单 → Block（不调 LLM）
//!         ├─ LlmModerationStage：
//!         │     1. build_messages(submission) → Vec<LlmMessage>
//!         │     2. LlmClient.chat(messages) → 模型回复（crates/llm）
//!         │     3. parse_verdict(reply) → Verdict
//!         ├─ ModerationThresholds::apply → label 升级
//!         └─ 早停：任一 Block 立刻返回
//!         │
//!         ▼
//!  最终 Verdict (Allow / Flag / Block)
//! ```
//!
//! ## 设计
//! - **transport 在 `crates/llm`**：端点 / 超时 / 鉴权 / 协议（OpenAI 兼容或
//!   Anthropic 兼容）由 env 选择，本 crate 只管提示词与结论解析。
//! - **失败不放行**：LLM 失败 / 回复无法解析 → 按 `on_llm_failure` 记为 Flag
//!   （默认：照常发布并进审核队列）或 Block（拒绝提交），warning 日志带累计失败次数。
//! - **默认禁用**：`site.json::moderation.enabled` 与 `llm_review` 默认 false。
//!
//! ## 使用方式
//! ```ignore
//! use module_moderation::ModerationPipeline;
//! use sdk::ModerationSubmission;
//!
//! let pipeline = ModerationPipeline::from_site_config(&site_cfg, llm);
//! let submission = ModerationSubmission::new(comment_body).with_kind("comment");
//! let verdict = pipeline.evaluate(submission).await;
//! match verdict.label {
//!     ModerationLabel::Block => return Err("内容被审核拒绝".into()),
//!     ModerationLabel::Flag  => /* 入库但打 flag 给 admin 复核 */,
//!     ModerationLabel::Allow => /* 正常入库 */,
//! }
//! ```

pub mod hook;
pub mod llm_stage;
pub mod pipeline;
pub mod stage;
pub mod url_blocklist;

pub use hook::{
  absolutize_image_url, enqueue_if_flagged, evaluate_submission, evaluate_with_images,
  extract_image_urls, reload_pipeline, shared_pipeline,
};
pub use llm_stage::LlmModerationStage;
pub use pipeline::ModerationPipeline;
pub use stage::AsyncModerationStage;
pub use url_blocklist::UrlBlocklistStage;

// 重导出常用类型，调用方只需要 use 本 crate 顶层。
pub use app_core::engines::moderation::{ModerationLabel, ModerationThresholds, Verdict};
pub use sdk::ModerationSubmission;
