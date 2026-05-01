use dioxus::prelude::*;
use dioxus::router::Link;

use crate::routes::Route;

/// Homepage hero section.
#[component]
pub fn Hero() -> Element {
  rsx! {
      section { class: "w-full py-16 sm:py-20 bg-gradient-to-b from-slate-950 to-slate-900",
          div { class: "max-w-6xl mx-auto px-4 sm:px-6 lg:px-8",
              div { class: "max-w-3xl",
                  h1 { class: "text-4xl md:text-5xl font-extrabold tracking-tight text-white",
                      "专注 Rust 技术栈的学习与实战"
                  }
                  p { class: "mt-5 text-lg md:text-xl text-slate-300",
                      "文档、博客、课程、案例一站式聚合：AI、后端、前端、跨端、Web3、Wasm、嵌入式、命令行等。"
                  }

                  div { class: "mt-8 flex flex-col sm:flex-row gap-3",
                      Link {
                          to: Route::Docs {},
                          class: "inline-flex justify-center rounded-md bg-white px-5 py-3 text-sm font-semibold text-slate-900 hover:bg-slate-100",
                          "进入文档"
                      }
                      Link {
                          to: Route::BlogIndex {},
                          class: "inline-flex justify-center rounded-md border border-white/20 px-5 py-3 text-sm font-semibold text-white hover:bg-white/10",
                          "浏览博客"
                      }
                      Link {
                          to: Route::Cases {},
                          class: "inline-flex justify-center rounded-md border border-white/20 px-5 py-3 text-sm font-semibold text-white hover:bg-white/10",
                          "查看案例"
                      }
                  }

                  div { class: "mt-10 grid grid-cols-2 sm:grid-cols-4 gap-4 text-sm",
                      HeroStat { value: "文档".to_string(), label: "从零到一".to_string() }
                      HeroStat { value: "博客".to_string(), label: "持续更新".to_string() }
                      HeroStat { value: "课程".to_string(), label: "系统学习".to_string() }
                      HeroStat { value: "案例".to_string(), label: "可复用模板".to_string() }
                  }
              }
          }
      }
  }
}

#[derive(Clone, PartialEq, Props)]
struct HeroStatProps {
  value: String,
  label: String,
}

#[component]
fn HeroStat(props: HeroStatProps) -> Element {
  rsx! {
      div { class: "rounded-lg border border-white/10 bg-white/5 p-4",
          div { class: "text-white font-semibold", "{props.value}" }
          div { class: "text-slate-300 text-xs mt-1", "{props.label}" }
      }
  }
}
