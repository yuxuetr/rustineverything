//! 嵌入式 Rust 内容板块（Phase 6.1）。
//!
//! 结构与其它内容模块一致：[`text`] 持纯逻辑 + 板块元数据 + 单测，
//! [`server`] 扫描 `assets/topics/embedded/` 下的 markdown 文章，
//! [`embedded`] 提供 RSX 落地页与详情页（用 `<a href>` 导航，避免对
//! app `Route` 的循环依赖）。内容渲染复用 [`rustineverything_widgets::Markdown`]。

pub mod embedded;
pub mod server;
pub mod text;

use rustineverything_sdk::AppModule;

pub struct EmbeddedModule;

impl AppModule for EmbeddedModule {
    fn name(&self) -> &'static str {
        "Embedded"
    }
}
