//! Phase 4.3-4.4：LLM 审核流水线 + WASM 插件运行时。
//!
//! ## 架构
//! ```text
//! 用户提交内容 (评论 / 话题 / 回复 / 标注)
//!         │
//!         ▼
//! ModerationPipeline (本 crate)
//!         │
//!         ├─ for each PluginModerationStage:
//!         │     1. plugin.moderation_build_prompt(submission) → Vec<LlmMessage>
//!         │     2. LlmClient.chat(messages) → LLM 文本（rustineverything-llm）
//!         │     3. plugin.moderation_parse_verdict(text) → Verdict
//!         │     4. ModerationThresholds::apply → label 升级
//!         │     5. 早停：任一 Block 立刻返回
//!         │
//!         ▼
//!  最终 Verdict (Allow / Flag / Block)
//! ```
//!
//! ## 设计
//! - **插件管 policy**：prompt 措辞、verdict 解析格式。
//! - **宿主管 transport**：复用 `crates/llm` 的 [`LlmClient`]，统一管端点 /
//!   超时 / 鉴权 / 协议（OpenAI 兼容或 Anthropic 兼容）。
//! - **fail-open**：插件加载失败 / LLM 失败 / 解析失败 → 当前 stage 返回
//!   Allow，记 warning 日志，不阻塞用户提交。Block 决定必须来自成功的
//!   完整流水线。
//! - **默认禁用**：`site.json::modules.moderation.enabled` 默认 false，
//!   plugins 数组默认空。即使开启 enabled，没有插件就什么也不做。
//!
//! ## 使用方式
//! ```ignore
//! use rustineverything_module_moderation::ModerationPipeline;
//! use rustineverything_sdk::ModerationSubmission;
//!
//! // 启动期一次性构造（启用且配了插件才有内容）
//! let pipeline = ModerationPipeline::from_site_config(&site_cfg, &asset_root);
//!
//! // 提交路径调用
//! let submission = ModerationSubmission::new(comment_body).with_kind("comment");
//! let verdict = pipeline.evaluate(submission).await;
//! match verdict.label {
//!     ModerationLabel::Block => return Err("内容被审核拒绝".into()),
//!     ModerationLabel::Flag  => /* 入库但打 flag 给 admin 复核 */,
//!     ModerationLabel::Allow => /* 正常入库 */,
//! }
//! ```

pub mod hook;
pub mod pipeline;
pub mod plugin_stage;
pub mod stage;
pub mod url_blocklist;

pub use hook::{
  absolutize_image_url, enqueue_if_flagged, evaluate_submission, evaluate_with_images,
  extract_image_urls, shared_pipeline,
};
pub use pipeline::ModerationPipeline;
pub use plugin_stage::PluginModerationStage;
pub use stage::AsyncModerationStage;
pub use url_blocklist::UrlBlocklistStage;

// 重导出常用类型，调用方只需要 use 本 crate 顶层。
pub use rustineverything_core::engines::moderation::{
  ModerationLabel, ModerationThresholds, Verdict,
};
pub use rustineverything_sdk::{ModerationSubmission, ModerationVerdict};
