//! Rust AI 内容板块（Phase 6.2）。结构同其它内容模块：[`text`] 持元数据 +
//! 纯逻辑 + 单测，[`server`] 扫描 `assets/topics/ai/`，[`ai`] 提供 RSX 页面。

pub mod ai;
pub mod server;
pub mod text;

use rustineverything_sdk::AppModule;

pub struct AiModule;

impl AppModule for AiModule {
    fn name(&self) -> &'static str {
        "AI"
    }
}
