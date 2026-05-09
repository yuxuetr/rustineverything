//! `rustineverything-widgets`：内容渲染共享组件。
//!
//! ## 内容
//! - [`mdx`]：MDX 渲染管道（GFM / 数学 / Mermaid / 代码 + Copy / 标注 block-id）。
//! - [`registry`]：MDX 嵌入组件注册表（Phase 2.1 引入 / Phase 2.2 完整接入）。
//!
//! widgets crate 不依赖任何 `crates/modules/*`，业务模块通过 [`registry::register`]
//! 在 app 启动时注入自定义 MDX 组件（如 `<PodcastCard id="1" />`）。
//!
//! ## 典型用法
//! ```ignore
//! // 在 app/src/main.rs 启动期：
//! rustineverything_module_podcast::register_components();
//!
//! // 各内容页：
//! use rustineverything_widgets::Markdown;
//! rsx! { Markdown { content: text, blog_id: "welcome".to_string() } }
//! ```

pub mod mdx;
pub mod registry;

// 重导出最常用 API，方便调用方仅依赖 widgets 顶层路径。
pub use mdx::{parse_mdx, Markdown, MarkdownProps, PostMetadata};
pub use registry::{
    clear_for_tests, list_registered, register, registered_count, ComponentRegistry, MdxComponent,
};
