use dioxus::prelude::*;
use dioxus::router::Link;

use crate::i18n::{t, use_i18n};
use crate::routes::Route;

/// Homepage hero section. `enabled`：已启用模块，关掉的模块不出按钮。
#[component]
pub fn Hero(enabled: Vec<String>) -> Element {
  let lang = use_i18n();
  let on = |id: &str| enabled.iter().any(|e| e == id);
  rsx! {
      section { class: "w-full py-16 sm:py-20 bg-gradient-to-b from-slate-950 to-slate-900",
          div { class: "max-w-6xl mx-auto px-4 sm:px-6 lg:px-8",
              div { class: "max-w-3xl",
                  h1 { class: "text-4xl md:text-5xl font-extrabold tracking-tight text-flow-light",
                      "{t(lang(), \"hero.title\")}"
                  }
                  p { class: "mt-5 text-lg md:text-xl text-slate-300",
                      "{t(lang(), \"hero.subtitle\")}"
                  }

                  div { class: "mt-8 flex flex-col sm:flex-row gap-3",
                      // 案例（差异化核心）作主 CTA，课程（变现核心）次之，文档兜底。
                      if on("cases") {
                          Link {
                              to: Route::Cases {},
                              class: "inline-flex justify-center rounded-md bg-white px-5 py-3 text-sm font-semibold text-slate-900 hover:bg-slate-100",
                              "{t(lang(), \"hero.btn.cases\")}"
                          }
                      }
                      if on("course") {
                          Link {
                              to: Route::Courses {},
                              class: "inline-flex justify-center rounded-md border border-white/20 px-5 py-3 text-sm font-semibold text-white hover:bg-white/10",
                              "{t(lang(), \"hero.btn.courses\")}"
                          }
                      }
                      if on("docs") {
                          Link {
                              to: Route::Docs {},
                              class: "inline-flex justify-center rounded-md border border-white/20 px-5 py-3 text-sm font-semibold text-white hover:bg-white/10",
                              "{t(lang(), \"hero.btn.docs\")}"
                          }
                      }
                  }
              }
          }
      }
  }
}
