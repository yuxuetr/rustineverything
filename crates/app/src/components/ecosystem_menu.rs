//! 生态 mega 菜单（桌面，≥lg）。
//!
//! 每个生态一个触发按钮 + 下拉面板（三栏：应用领域 / 学习资源 / 精选）。
//! 展开用纯 CSS：`group-hover` + `group-focus-within`，无 signal、无 hydration
//! 风险，且键盘 focus 进入即展开。面板顶部 `pt-2` 充当鼠标移动「桥」，避免
//! 按钮与面板间出现死区导致闪烁。移动端（<lg）不用本组件，改走抽屉里的分组列表。
//!
//! a11y：`aria-haspopup` / `aria-expanded`（CSS 驱动，标注语义）；焦点移出即收起。
//! Esc 关闭等增强留待后续打磨（见 docs/SITE_REDESIGN_SPEC.md §3.4）。

use dioxus::prelude::*;
use dioxus::router::Link;

use crate::i18n::{t, use_i18n};
use crate::routes::Route;
use crate::taxonomy::Ecosystem;

/// 单个生态的桌面 mega 菜单。`enabled` 为已启用模块 id 列表，用于隐藏被关闭的领域。
#[component]
pub fn EcosystemMenu(eco: Ecosystem, enabled: Vec<String>) -> Element {
  let lang = use_i18n();
  let label = t(lang(), eco.label_key);

  // 仅展示已启用模块对应的领域；全部关闭时仍渲染按钮（学习资源列仍有用）。
  let domains: Vec<_> =
    eco.domains.iter().filter(|d| enabled.iter().any(|e| e == d.module_id)).cloned().collect();

  let item_class = "block px-2 py-1.5 rounded-md text-sm text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors";
  let col_title =
    "text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-2";

  rsx! {
      div { class: "relative group h-14 flex items-center",
          button {
              class: "inline-flex items-center gap-1 px-2 h-14 text-slate-700 hover:text-slate-900 dark:text-slate-200 dark:hover:text-white transition-colors",
              aria_haspopup: "true",
              "{label}"
              svg {
                  class: "w-3.5 h-3.5 text-slate-400 transition-transform group-hover:rotate-180",
                  fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "2.5",
                  path { stroke_linecap: "round", stroke_linejoin: "round", d: "M19 9l-7 7-7-7" }
              }
          }

          // 面板：默认隐藏，hover / focus-within 显示。
          div {
              class: "invisible opacity-0 translate-y-1 group-hover:visible group-hover:opacity-100 group-hover:translate-y-0 group-focus-within:visible group-focus-within:opacity-100 group-focus-within:translate-y-0 absolute left-0 top-full z-50 pt-2 transition-all duration-150",
              div { class: "w-[34rem] rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-xl p-4 grid grid-cols-3 gap-4",
                  // 第一列：应用领域 / 方向
                  div {
                      p { class: "{col_title}", "{t(lang(), \"mega.col.domains\")}" }
                      div { class: "flex flex-col",
                          for d in domains.iter() {
                              Link { key: "{d.id}", to: d.route.clone(), class: "{item_class}", "{t(lang(), d.label_key)}" }
                          }
                      }
                  }
                  // 第二列：学习资源（M3 起改为生态过滤视图）
                  div {
                      p { class: "{col_title}", "{t(lang(), \"mega.col.learn\")}" }
                      div { class: "flex flex-col",
                          Link { to: Route::Docs {}, class: "{item_class}", "{t(lang(), \"mega.learn.docs\")}" }
                          Link { to: Route::Courses {}, class: "{item_class}", "{t(lang(), \"mega.learn.courses\")}" }
                          Link { to: Route::Cases {}, class: "{item_class}", "{t(lang(), \"mega.learn.cases\")}" }
                      }
                  }
                  // 第三列：生态简介 + 精选案例 CTA（M2/M3 接 cases.favorite 实时数据）
                  div { class: "rounded-lg bg-slate-50 dark:bg-slate-800/50 p-3 flex flex-col justify-between",
                      div {
                          p { class: "text-sm font-semibold text-slate-900 dark:text-white", "{label}" }
                          p { class: "text-xs text-slate-500 dark:text-slate-400 mt-1 leading-relaxed", "{t(lang(), eco.blurb_key)}" }
                      }
                      Link { to: Route::Cases {}, class: "mt-3 inline-flex items-center gap-1 text-xs font-medium text-[var(--color-primary)] hover:underline",
                          "{t(lang(), \"mega.featured.cta\")}"
                          svg { class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "2",
                              path { stroke_linecap: "round", stroke_linejoin: "round", d: "M9 5l7 7-7 7" }
                          }
                      }
                  }
              }
          }
      }
  }
}
