use dioxus::prelude::*;
use dioxus::router::Link;

use crate::i18n::{t, use_i18n};
use crate::routes::Route;

/// Homepage hero section.
#[component]
pub fn Hero() -> Element {
  let lang = use_i18n();
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
                      Link {
                          to: Route::Docs {},
                          class: "inline-flex justify-center rounded-md bg-white px-5 py-3 text-sm font-semibold text-slate-900 hover:bg-slate-100",
                          "{t(lang(), \"hero.btn.docs\")}"
                      }
                      Link {
                          to: Route::BlogIndex {},
                          class: "inline-flex justify-center rounded-md border border-white/20 px-5 py-3 text-sm font-semibold text-white hover:bg-white/10",
                          "{t(lang(), \"hero.btn.blog\")}"
                      }
                      Link {
                          to: Route::Cases {},
                          class: "inline-flex justify-center rounded-md border border-white/20 px-5 py-3 text-sm font-semibold text-white hover:bg-white/10",
                          "{t(lang(), \"hero.btn.cases\")}"
                      }
                  }

                  div { class: "mt-10 grid grid-cols-2 sm:grid-cols-4 gap-4 text-sm",
                      HeroStat { value: t(lang(), "hero.stat.docs.value"), label: t(lang(), "hero.stat.docs.label") }
                      HeroStat { value: t(lang(), "hero.stat.blog.value"), label: t(lang(), "hero.stat.blog.label") }
                      HeroStat { value: t(lang(), "hero.stat.course.value"), label: t(lang(), "hero.stat.course.label") }
                      HeroStat { value: t(lang(), "hero.stat.cases.value"), label: t(lang(), "hero.stat.cases.label") }
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
