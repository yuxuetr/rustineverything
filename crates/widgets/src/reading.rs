//! 阅读页外壳与导航树（T3，设计见 docs/READING_LAYOUT_SPEC.md）：文档页与课节页共用。
//! 左侧是跨页导航，≥768px 时是 sticky 侧栏，以下收进抽屉，由正文上方的按钮打开。

use dioxus::prelude::*;
use dioxus_shadcn::{Sidebar, SidebarProvider, SidebarTrigger, Tree};

/// 桌面是 sticky 侧栏（`md:` 前缀只作用于桌面形态，抽屉形态由组件库定位）。
/// 抽屉要等 hydration 后才知道屏幕宽度，在那之前组件按桌面渲染：手机上先藏起来，
/// 免得一列侧栏挤在正文旁边（`data-mobile` 在抽屉形态下为 true）。
const SIDEBAR_CLASS: &str = "md:sticky md:top-14 md:h-[calc(100vh-3.5rem)] overflow-y-auto bg-white dark:bg-slate-950 border-slate-200 dark:border-slate-800 pt-8 pb-12 px-4 max-md:data-[mobile=false]:hidden";

/// 阅读页：左侧 `nav` + 正文 `children`。`path` 变化（换页）时关上抽屉；
/// `nav_label` 是抽屉按钮文字与侧栏名称，由调用方按语言传入。
#[component]
pub fn ReadingLayout(path: String, nav_label: String, nav: Element, children: Element) -> Element {
  let mut mobile_open = use_signal(|| false);
  use_effect(use_reactive!(|path| {
    let _ = path;
    mobile_open.set(false);
  }));
  rsx! {
      SidebarProvider {
          off_canvas: true,
          mobile_open: Some(mobile_open()),
          on_mobile_open_change: move |open| mobile_open.set(open),
          div { class: "max-w-7xl mx-auto flex",
              Sidebar { "aria-label": "{nav_label}", class: SIDEBAR_CLASS, {nav} }
              div { class: "flex-1 min-w-0 px-4 sm:px-6 lg:px-12 py-8",
                  SidebarTrigger { class: "md:hidden mb-6 w-auto gap-2 border border-slate-200 dark:border-slate-800 px-3",
                      svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", "aria-hidden": "true",
                          path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M4 6h16M4 12h16M4 18h16" }
                      }
                      "{nav_label}"
                  }
                  {children}
              }
          }
      }
  }
}

/// 由路由驱动的导航树：`active` 是当前页的项（选中），`expand` 是要展开的项（当前页的祖先）。
/// 换页时补进新的 `expand`，用户手动展开 / 折叠的保留。
///
/// 项的 `value` 约定：页面用它的站内路径（以 `/` 开头），只是分组的用别的值。键盘 Enter /
/// 点行的空白处只会「选中」，这里把选中页面变成导航；点的是行里的 `Link` 时路由已经切过去，
/// 与当前地址相同就不再 push，免得历史记录里多一条。
#[component]
pub fn NavTree(label: String, active: String, expand: Vec<String>, children: Element) -> Element {
  let mut expanded = use_signal(|| expand.clone());
  use_effect(use_reactive!(|expand| {
    let missing: Vec<String> =
      expand.into_iter().filter(|v| !expanded.peek().contains(v)).collect();
    if !missing.is_empty() {
      expanded.write().extend(missing);
    }
  }));
  rsx! {
      Tree {
          "aria-label": "{label}",
          expanded: Some(expanded()),
          on_expanded_change: move |v| expanded.set(v),
          selected: Some(active.clone()),
          on_selected_change: move |value: String| {
              if value.starts_with('/') && router().full_route_string() != value {
                  navigator().push(value);
              }
          },
          {children}
      }
  }
}
