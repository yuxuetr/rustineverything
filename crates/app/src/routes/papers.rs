//! 论文资讯页面（`papers` feature）。未编译时由 `absent_pages!` 生成同名 404 组件。

use dioxus::prelude::*;

use crate::components::module_gate::ModuleGate;
use module_papers::PapersIndexPage;

#[component]
pub fn Papers() -> Element {
  rsx! { ModuleGate { id: "papers".to_string(), PapersIndexPage {} } }
}
