//! Phase 3.3：Minimal 布局 — 紧凑顶部条 + 无 Footer + 无主导航。
//!
//! 适合写作 / 阅读优先场景。仅保留 Logo（首页链接）+ 搜索 + 主题切换 +
//! 语言 + 暗色 + 登录入口；不渲染主导航栏列表，也不渲染 Footer。

use dioxus::prelude::*;
use dioxus::router::{Link, Outlet};

use crate::components::lang_picker::LangPicker;
use crate::components::theme_picker::ThemePicker;
use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use crate::routes::Route;
use dioxus::document::eval;
use module_search::search::SearchButton;

/// Minimal shell：极简顶部条，`Outlet::<Route>` 主导内容；无 Footer。
#[component]
pub fn MinimalShell() -> Element {
  let lang = use_i18n();
  let mut is_dark = use_signal(|| false);
  let mut show_auth_modal = crate::use_auth_modal();
  let session_user = crate::use_session_user();
  let mut show_user_menu = use_signal(|| false);

  // 与 Classic 共享 dark 模式初始化逻辑
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

  rsx! {
      div { class: "min-h-screen flex flex-col",
          header { class: "sticky top-0 z-50 border-b border-slate-200/70 bg-white/80 backdrop-blur dark:bg-slate-950/70 dark:border-slate-800",
              Container {
                  div { class: "h-12 flex items-center justify-between gap-4",
                      Link {
                          to: Route::Home {},
                          class: "font-extrabold tracking-tight text-flow text-sm",
                          "Rust in Everything"
                      }
                      div { class: "flex items-center gap-2",
                          SearchButton {}
                          ThemePicker {}
                          LangPicker {}
                          button {
                              onclick: toggle_dark,
                              class: "p-1.5 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors",
                              if is_dark() {
                                  svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" }
                                  }
                              } else {
                                  svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" }
                                  }
                              }
                          }
                          // 用户菜单：与 Classic 一致但样式更紧凑
                          if let Some(ref u) = session_user() {
                              div { class: "relative",
                                  button {
                                      onclick: move |_| show_user_menu.set(!show_user_menu()),
                                      class: "flex items-center gap-1.5 px-1.5 py-1 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                      if let Some(ref avatar) = u.avatar_url {
                                          img {
                                              src: "{avatar}",
                                              class: "w-6 h-6 shrink-0 rounded-full object-cover",
                                              width: "24",
                                              height: "24",
                                              alt: "{u.nickname}"
                                          }
                                      } else {
                                          div { class: "w-6 h-6 shrink-0 rounded-full bg-blue-600 flex items-center justify-center text-white text-xs font-bold",
                                              "{u.nickname.chars().next().unwrap_or('U')}"
                                          }
                                      }
                                  }
                                  if show_user_menu() {
                                      div { class: "absolute right-0 top-full mt-1 w-40 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-lg py-1 z-50",
                                          div { class: "px-3 py-1.5 text-xs text-slate-500 border-b border-slate-100 dark:border-slate-800",
                                              "{u.nickname}"
                                          }
                                          Link {
                                              to: Route::MyAnnotations {},
                                              class: "block px-3 py-1.5 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                              "我的标注"
                                          }
                                          if u.is_admin() {
                                              Link {
                                                  to: Route::AdminDashboard {},
                                                  class: "block px-3 py-1.5 text-sm font-semibold text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors",
                                                  "管理后台"
                                              }
                                          }
                                          a {
                                              href: "/api/auth/logout",
                                              class: "block px-3 py-1.5 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                              "{t(lang(), \"auth.logout\")}"
                                          }
                                      }
                                  }
                              }
                          } else {
                              button {
                                  onclick: move |_| show_auth_modal.set(true),
                                  class: "px-2 py-1 rounded-md text-xs font-medium text-slate-700 hover:text-slate-900 hover:bg-slate-100 dark:text-slate-300 dark:hover:text-white dark:hover:bg-slate-800 transition-colors",
                                  "{t(lang(), \"auth.sign_in\")}"
                              }
                          }
                      }
                  }
              }
          }

          main { class: "flex-1",
              Outlet::<Route> {}
          }
      }
  }
}
