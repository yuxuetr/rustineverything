use dioxus::prelude::*;
use dioxus::router::{Link, Outlet};

use crate::components::view::Container;
use crate::i18n::{t, use_i18n, use_t, Language};
use crate::routes::Route;
use dioxus::document::eval;

/// Shared navbar layout.
#[component]
pub fn Navbar() -> Element {
  let route = use_route::<Route>();
  let mut lang = use_i18n();
  let mut is_dark = use_signal(|| false);

  // Dynamic Translations from WASM Plugins
  let t_blog = use_t("nav-blog");
  let t_podcast = use_t("nav-podcast");
  let t_forum = use_t("nav-forum");

  let link_class = move |target: Route| {
    let is_active = match (&route, &target) {
      (Route::Blog { .. }, Route::BlogIndex {}) => true,
      (Route::Topic { .. }, Route::TopicsIndex {}) => true,
      (current, target) => current == target,
    };

    if is_active {
      // Use the CSS variable defined by our Theme Plugin!
      "text-[var(--color-primary)] font-bold border-b-2 border-[var(--color-primary)] h-14 flex items-center"
    } else {
      "text-slate-700 hover:text-slate-900 dark:text-slate-200 dark:hover:text-white transition-colors h-14 flex items-center"
    }
  };

  // Initialize dark mode preference
  use_effect(move || {
    spawn(async move {
      let script = r#"
                let isDark = localStorage.theme === 'dark' || (!('theme' in localStorage) && window.matchMedia('(prefers-color-scheme: dark)').matches);
                if (isDark) {
                    document.documentElement.classList.add('dark');
                } else {
                    document.documentElement.classList.remove('dark');
                }
                dioxus.send(isDark);
            "#;
      let mut eval = eval(script);
      if let Ok(val) = eval.recv::<bool>().await {
        is_dark.set(val);
      }
    });
  });

  let toggle_dark = move |_| {
    let new_val = !is_dark();
    is_dark.set(new_val);
    let script = if new_val {
      "document.documentElement.classList.add('dark'); localStorage.theme = 'dark'"
    } else {
      "document.documentElement.classList.remove('dark'); localStorage.theme = 'light'"
    };
    let _ = eval(script);
  };

  let toggle_lang = move |_| {
    if lang() == Language::Zh {
      lang.set(Language::En);
    } else {
      lang.set(Language::Zh);
    }
  };

  rsx! {
      header { class: "sticky top-0 z-50 border-b border-slate-200/70 bg-white/80 backdrop-blur dark:bg-slate-950/70 dark:border-slate-800",
          Container {
              div { class: "h-14 flex items-center justify-between",
                  div { class: "flex items-center gap-6",
                      Link { to: Route::Home {}, class: "font-extrabold tracking-tight text-slate-900 dark:text-white", "Rust in Everything" }
                      nav { class: "hidden md:flex items-center gap-4 text-sm font-medium",
                          Link { to: Route::BlogIndex {}, class: link_class(Route::BlogIndex {}), "{t_blog}" }
                          Link { to: Route::Podcast {}, class: link_class(Route::Podcast {}), "{t_podcast}" }
                          Link { to: Route::TopicsIndex {}, class: link_class(Route::TopicsIndex {}), "{t_forum}" }
                      }
                  }

                  div { class: "flex items-center gap-3",
                      // Language Toggle
                      button {
                          onclick: toggle_lang,
                          class: "p-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors text-xs font-semibold",
                          if lang() == Language::Zh { "EN" } else { "中" }
                      }

                      // Dark Mode Toggle
                      button {
                          onclick: toggle_dark,
                          class: "p-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors",
                          if is_dark() {
                              // Sun icon (Click to switch to light)
                              svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                  path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" }
                              }
                          } else {
                              // Moon icon (Click to switch to dark)
                              svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                  path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" }
                              }
                          }
                      }

                      Link {
                          to: Route::Docs {},
                          class: "hidden sm:inline-flex items-center rounded-md bg-slate-900 px-3 py-2 text-sm font-semibold text-white hover:bg-slate-700 dark:bg-white dark:text-slate-900 dark:hover:bg-slate-200 transition-colors",
                          "{t(lang(), \"nav.start\")}"
                      }
                  }
              }
          }
      }

      main { class: "min-h-[calc(100vh-3.5rem)]",
          Outlet::<Route> {}
      }

      footer { class: "border-t border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950",
          Container {
              div { class: "py-10 text-sm text-slate-600 dark:text-slate-300 flex flex-col md:flex-row gap-3 md:items-center md:justify-between",
                  div {
                      span { class: "font-semibold text-slate-900 dark:text-white", "Rust in Everything" }
                      span { class: "mx-2", "·" }
                      span { "专注 Rust 技术栈" }
                  }
                  div { class: "flex gap-4",
                      Link { to: Route::TopicsIndex {}, class: "hover:text-slate-900 dark:hover:text-white transition-colors", "Topics" }
                      Link { to: Route::BlogIndex {}, class: "hover:text-slate-900 dark:hover:text-white transition-colors", "Blog" }
                      Link { to: Route::Docs {}, class: "hover:text-slate-900 dark:hover:text-white transition-colors", "Docs" }
                  }
              }
          }
      }
  }
}
