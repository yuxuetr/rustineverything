//! 首页分区组件（M2）。
//!
//! 围绕「Rust 生态 + AI 生态」双支柱组织首页：生态 pillars、精选案例、课程、
//! 社区动态等。数据来源沿用各模块 server fn；SEO 关键区用 `use_server_future`
//! + `SuspenseBoundary`（与内容页一致），社区/DB 区用 `use_resource`。
//! 详见 `docs/SITE_REDESIGN_SPEC.md` §4。

use dioxus::prelude::*;
use dioxus::router::Link;

use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use crate::taxonomy::ecosystems;

/// 两大生态 pillars：Rust 生态 | AI 生态，各列子领域 chips（链接到领域路由）。
/// `enabled` 用于隐藏被关闭模块对应的领域，与导航保持一致。
#[component]
pub fn EcosystemPillars(enabled: Vec<String>) -> Element {
  let lang = use_i18n();
  rsx! {
      section { class: "py-16 sm:py-20 bg-slate-50/60 dark:bg-slate-900/40 border-y border-slate-200/70 dark:border-slate-800",
          Container {
              div { class: "grid grid-cols-1 md:grid-cols-2 gap-6",
                  for eco in ecosystems() {
                      {
                          let domains: Vec<_> = eco.domains.iter().filter(|d| enabled.iter().any(|e| e == d.module_id)).cloned().collect();
                          rsx! {
                              div { key: "{eco.id}", class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-7 flex flex-col",
                                  h3 { class: "text-2xl font-extrabold tracking-tight text-flow", "{t(lang(), eco.label_key)}" }
                                  p { class: "mt-2 text-slate-600 dark:text-slate-400 leading-relaxed", "{t(lang(), eco.blurb_key)}" }
                                  div { class: "mt-5 flex flex-wrap gap-2",
                                      for d in domains.iter() {
                                          Link {
                                              key: "{d.id}",
                                              to: d.route.clone(),
                                              class: "inline-flex items-center rounded-full border border-slate-200 dark:border-slate-700 px-3 py-1 text-sm text-slate-700 dark:text-slate-200 hover:border-[var(--color-primary)] hover:text-[var(--color-primary)] transition-colors",
                                              "{t(lang(), d.label_key)}"
                                          }
                                      }
                                  }
                              }
                          }
                      }
                  }
              }
          }
      }
  }
}
