use dioxus::prelude::*;
use dioxus::router::{Link, Outlet};

use crate::components::view::Container;
use crate::i18n::{t, use_i18n, use_t, Language};
use crate::routes::Route;
use dioxus::document::eval;
use rustineverything_module_search::search::SearchButton;

/// Shared navbar layout.
#[component]
pub fn Navbar() -> Element {
  let route = use_route::<Route>();
  let mut lang = use_i18n();
  let mut is_dark = use_signal(|| false);
  let mut show_auth_modal = crate::use_auth_modal();
  let session_user = crate::use_session_user();
  let mut show_user_menu = use_signal(|| false);

  // Dynamic Translations from WASM Plugins
  let t_blog = use_t("nav-blog");
  let t_podcast = use_t("nav-podcast");
  let t_forum = use_t("nav-forum");

  let link_class = move |target: Route| {
    let is_active = match (&route, &target) {
      (Route::Blog { .. }, Route::BlogIndex {}) => true,
      (Route::TopicsByTag { .. }, Route::TopicsIndex {}) => true,
      (Route::TopicDetail { .. }, Route::TopicsIndex {}) => true,
      (Route::TopicsNew {}, Route::TopicsIndex {}) => true,
      (Route::CaseDetail { .. }, Route::Cases {}) => true,
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
    div { class: "min-h-screen flex flex-col",
      header { class: "sticky top-0 z-50 border-b border-slate-200/70 bg-white/80 backdrop-blur dark:bg-slate-950/70 dark:border-slate-800",
          Container {
              div { class: "h-14 flex items-center justify-between",
                  div { class: "flex items-center gap-6",
                      Link { to: Route::Home {}, class: "font-extrabold tracking-tight text-flow", "Rust in Everything" }
                      nav { class: "hidden md:flex items-center gap-4 text-sm font-medium",
                          Link { to: Route::BlogIndex {}, class: link_class(Route::BlogIndex {}), "{t_blog}" }
                          Link { to: Route::Podcast {}, class: link_class(Route::Podcast {}), "{t_podcast}" }
                          Link { to: Route::Cases {}, class: link_class(Route::Cases {}), "{t(lang(), \"nav.cases\")}" }
                          Link { to: Route::TopicsIndex {}, class: link_class(Route::TopicsIndex {}), "{t_forum}" }
                      }
                  }

                  div { class: "flex items-center gap-3",
                      // Search
                      SearchButton {}

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

                      // User avatar / Sign In
                      if let Some(ref u) = session_user() {
                          // 已登录：头像 + 昵称 + 下拉菜单
                          div { class: "relative",
                              button {
                                  onclick: move |_| show_user_menu.set(!show_user_menu()),
                                  class: "flex items-center gap-2 px-2 py-1 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                  if let Some(ref avatar) = u.avatar_url {
                                      img {
                                          src: "{avatar}",
                                          class: "w-7 h-7 shrink-0 rounded-full object-cover",
                                          width: "28",
                                          height: "28",
                                          alt: "{u.nickname}"
                                      }
                                  } else {
                                      div { class: "w-7 h-7 shrink-0 rounded-full bg-blue-600 flex items-center justify-center text-white text-xs font-bold",
                                          "{u.nickname.chars().next().unwrap_or('U')}"
                                      }
                                  }
                                  span { class: "hidden sm:inline text-sm font-medium text-slate-700 dark:text-slate-200", "{u.nickname}" }
                              }
                              // 下拉菜单
                              if show_user_menu() {
                                  div { class: "absolute right-0 top-full mt-1 w-44 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-lg py-1 z-50",
                                      div { class: "px-3 py-2 text-xs text-slate-500 border-b border-slate-100 dark:border-slate-800",
                                          "{u.nickname}"
                                      }
                                      Link {
                                          to: Route::MyTopics {},
                                          class: "block px-3 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                          "我的话题"
                                      }
                                      Link {
                                          to: Route::MyAnnotations {},
                                          class: "block px-3 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                          "我的标注"
                                      }
                                      if u.is_admin() {
                                          div { class: "my-1 border-t border-slate-100 dark:border-slate-800" }
                                          Link {
                                              to: Route::AdminDashboard {},
                                              class: "block px-3 py-2 text-sm font-semibold text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors",
                                              "🛡️ 管理后台"
                                          }
                                      }
                                      div { class: "my-1 border-t border-slate-100 dark:border-slate-800" }
                                      a {
                                          href: "/api/auth/logout",
                                          class: "block px-3 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                          "{t(lang(), \"auth.logout\")}"
                                      }
                                  }
                              }
                          }
                      } else {
                          // 未登录：登录按钮
                          button {
                              onclick: move |_| show_auth_modal.set(true),
                              class: "inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium text-slate-700 hover:text-slate-900 hover:bg-slate-100 dark:text-slate-300 dark:hover:text-white dark:hover:bg-slate-800 transition-colors",
                              svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                  path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.501 20.118a7.5 7.5 0 0114.998 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.499-1.632z" }
                              }
                              "{t(lang(), \"auth.sign_in\")}"
                          }
                      }

                      Link {
                          to: Route::Docs {},
                          class: "hidden sm:inline-flex items-center rounded-md btn-flow px-3 py-2 text-sm font-semibold transition-all",
                          "{t(lang(), \"nav.start\")}"
                      }
                  }
              }
          }
      }

      main { class: "flex-1",
          Outlet::<Route> {}
      }

      footer { class: "border-t border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 shrink-0",
          Container {
              div { class: "py-5 text-sm text-slate-600 dark:text-slate-300 flex flex-col md:flex-row gap-3 md:items-center md:justify-between",
                  div {
                      span { class: "font-semibold text-flow", "Rust in Everything" }
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
    } // end min-h-screen flex-col
  }
}
