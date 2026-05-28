//! Phase 3.3：Routable layout 入口。
//!
//! `Navbar` 是 `routes/mod.rs::Route` `#[layout(Navbar)]` 的入口组件；
//! Dioxus Router 编译期固定该入口，但内部我们通过 server fn `get_active_layout`
//! 动态选择 [`crate::components::layouts::ClassicShell`] 或
//! [`crate::components::layouts::MinimalShell`]。
//!
//! 历史 Navbar 的实现已下沉到 `layouts/classic.rs`；本文件仅负责分发。

use dioxus::prelude::*;

use crate::components::layouts::{ClassicShell, MinimalShell};
use crate::server::get_active_layout;

/// Routable layout 入口：内部根据 `get_active_layout` 选择 shell。
///
/// 等待 server fn 返回前先按 `classic` 渲染，避免布局闪烁。
#[component]
pub fn Navbar() -> Element {
  let layout_res =
    use_resource(
      || async move { get_active_layout().await.unwrap_or_else(|_| "classic".to_string()) },
    );
  let layout: String = layout_res.read().as_ref().cloned().unwrap_or_else(|| "classic".to_string());

  rsx! {
      if layout == "minimal" {
          MinimalShell {}
      } else {
          ClassicShell {}
      }
  }
}
