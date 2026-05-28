//! Blog 模块入口。MDX 渲染管道在 Phase 2.1 之后由 [`rustineverything-widgets`]
//! 提供，本 crate 仅保留博客元数据扫描 / 内容读取的 server fn。
//!
//! 内容页 (`Blog`) 直接 `use rustineverything_widgets::Markdown` 即可。

pub mod server;

use rustineverything_sdk::AppModule;

pub struct BlogModule;

impl AppModule for BlogModule {
  fn name(&self) -> &'static str {
    "Blog"
  }
}
