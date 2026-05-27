//! Phase 3.4：`ModuleGate` — 把页面包成 module-switch 感知组件。
//!
//! 用法：
//! ```ignore
//! ModuleGate { id: "forum".to_string(),
//!     TopicsIndexPage {}
//! }
//! ```
//! 内部调用 server fn [`crate::server::is_module_enabled`]。
//! - **启用** → 直接渲染 `children`。
//! - **未启用** → 渲染统一占位（302 风格 banner + 返回首页链接）。
//! - **server fn 尚未返回** → 渲染 children（默认开放，避免首屏闪烁）。
//!
//! 这样将「模块停用」的判定集中到一处，路由结构本身不变。

use dioxus::prelude::*;
use dioxus::router::Link;

use crate::components::view::Container;
use crate::routes::Route;
use crate::server::is_module_enabled;

#[component]
pub fn ModuleGate(id: String, children: Element) -> Element {
  let id_for_res = id.clone();
  let enabled_res = use_resource(move || {
    let value = id_for_res.clone();
    async move { is_module_enabled(value).await.unwrap_or(true) }
  });

  // 默认渲染 children，仅当 server fn 明确返回 false 时切换为占位。
  let is_disabled = matches!(enabled_res.read().as_ref(), Some(false));

  if is_disabled {
    let id_label = id.clone();
    rsx! {
      section { class: "py-20 bg-white dark:bg-slate-950",
        Container {
          div { class: "max-w-xl mx-auto text-center",
            div { class: "text-5xl mb-4", "⚙️" }
            h1 { class: "text-2xl font-bold text-slate-900 dark:text-white mb-3",
              "模块已停用"
            }
            p { class: "text-slate-500 dark:text-slate-400 mb-8",
              "「{id_label}」模块在 site.json 中被设置为 enabled: false，目前不可访问。"
            }
            Link {
              to: Route::Home {},
              class: "inline-flex items-center rounded-md btn-flow px-4 py-2 text-sm font-semibold",
              "回到首页"
            }
          }
        }
      }
    }
  } else {
    rsx! { {children} }
  }
}
