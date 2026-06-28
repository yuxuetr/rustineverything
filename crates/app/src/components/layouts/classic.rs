//! Phase 3.3：Classic 布局 — 完整 Navbar + Footer。
//!
//! 内容与 Phase 3.0 之前的 `Navbar` 组件一致：左侧 Logo + 主导航，右侧搜索 /
//! ThemePicker / 语言 / 暗色 / 用户菜单，下方 main 嵌入 `Outlet::<Route>`，
//! 末尾 Footer。
//!
//! 与 [`super::minimal::MinimalShell`] 互为备选；由 `Navbar` 根据 server fn
//! `get_active_layout` 切换。

use dioxus::prelude::*;
use dioxus::router::{Link, Outlet};

use crate::components::ecosystem_menu::EcosystemMenu;
use crate::components::lang_picker::LangPicker;
use crate::components::theme_picker::ThemePicker;
use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use crate::routes::Route;
use crate::server::enabled_module_ids;
use crate::taxonomy::ecosystems;
use dioxus::document::eval;
use module_search::search::SearchButton;

/// Classic shell：完整 Navbar+Footer。`Outlet::<Route>` 嵌于 main 中。
#[component]
pub fn ClassicShell() -> Element {
  let route = use_route::<Route>();
  let lang = use_i18n();
  let mut is_dark = use_signal(|| false);
  let mut show_auth_modal = crate::use_auth_modal();
  let session_user = crate::use_session_user();
  let mut show_user_menu = use_signal(|| false);
  // Phase 9.4：mobile 抽屉开关。md:hidden 显示一个 hamburger button，
  // 点击展开 header 下方的纵向 nav，让窄屏用户也能跳到板块。
  let mut show_mobile_menu = use_signal(|| false);

  // 同步翻译（方案 A）：读 `lang` 信号即可随语言切换重渲染，无服务端往返。
  let t_blog = t(lang(), "nav.blog");
  let t_podcast = t(lang(), "nav.podcast");
  let t_forum = t(lang(), "nav.forum");

  // Phase 3.4：站点模块开关。默认全开（避免首屏闪烁）。
  //
  // Phase 8.7：fallback 列表改从 `default_module_specs()` 单一源派生，
  // 不再在 navbar 里硬编码 11 个 module id —— 加 12th 模块只动 ModuleSpec
  // 一处即可在 nav 出现。
  fn all_default_ids() -> Vec<String> {
    app_core::engines::module::default_module_specs().into_iter().map(|s| s.id).collect()
  }
  let enabled_res =
    use_resource(
      || async move { enabled_module_ids().await.unwrap_or_else(|_| all_default_ids()) },
    );
  let enabled: Vec<String> = enabled_res.read().as_ref().cloned().unwrap_or_else(all_default_ids);
  let on_blog = enabled.iter().any(|s| s == "blog");
  let on_podcast = enabled.iter().any(|s| s == "podcast");
  let on_cases = enabled.iter().any(|s| s == "cases");
  let on_course = enabled.iter().any(|s| s == "course");
  let on_forum = enabled.iter().any(|s| s == "forum");
  let on_docs = enabled.iter().any(|s| s == "docs");

  let link_class = move |target: Route| {
    let is_active = match (&route, &target) {
      (Route::Blog { .. }, Route::BlogIndex {}) => true,
      (Route::TopicsByTag { .. }, Route::TopicsIndex {}) => true,
      (Route::TopicDetail { .. }, Route::TopicsIndex {}) => true,
      (Route::TopicsNew {}, Route::TopicsIndex {}) => true,
      (Route::CaseDetail { .. }, Route::Cases {}) => true,
      (Route::EmbeddedArticle { .. }, Route::Embedded {}) => true,
      (Route::AiArticle { .. }, Route::Ai {}) => true,
      (Route::Web3Article { .. }, Route::Web3 {}) => true,
      (Route::WasmArticle { .. }, Route::Wasm {}) => true,
      (Route::CliArticle { .. }, Route::Cli {}) => true,
      (current, target) => current == target,
    };

    if is_active {
      "px-2 text-[var(--color-primary)] font-bold border-b-2 border-[var(--color-primary)] h-14 flex items-center"
    } else {
      "px-2 text-slate-700 hover:text-slate-900 dark:text-slate-200 dark:hover:text-white transition-colors h-14 flex items-center"
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

  rsx! {
      div { class: "min-h-screen flex flex-col",
          header { class: "sticky top-0 z-50 border-b border-slate-200/70 bg-white/80 backdrop-blur dark:bg-slate-950/70 dark:border-slate-800",
              // 顶栏加宽容器（比正文 max-w-7xl 更宽）：双生态 mega + 4 内容入口 +
              // 完整站名 + 右侧控件，加宽后 justify-between 才有富余拉开间距。
              div { class: "mx-auto max-w-[88rem] px-4 sm:px-6 lg:px-8",
                  div { class: "h-14 flex items-center justify-between gap-4",
                      div { class: "flex items-center gap-3 min-w-0",
                          Link {
                              to: Route::Home {},
                              // mobile (<sm) 用 max-w-32 + truncate 防止站名挤占右侧按钮组；
                              // sm–lg 区间横向导航走 hamburger（见下），空间充足故完整显示；
                              // lg 起横向导航出现，shrink-0 确保标题不被挤压截断。
                              class: "font-extrabold tracking-tight text-flow whitespace-nowrap inline-block truncate max-w-32 sm:max-w-none lg:shrink-0",
                              "Rust in Everything"
                          }
                          // 双生态为主：Rust 生态▾ / AI 生态▾ mega 菜单 + 4 个内容类型入口。
                          // 领域（嵌入式/Web3/… · 大模型/推理/…）收进 mega，不再平铺顶层。
                          nav { class: "hidden lg:flex items-center gap-1 text-sm font-medium",
                              for eco in ecosystems() {
                                  EcosystemMenu { key: "{eco.id}", eco: eco.clone(), enabled: enabled.clone() }
                              }
                              if on_cases {
                                  Link { to: Route::Cases {}, class: link_class(Route::Cases {}), "{t(lang(), \"nav.cases\")}" }
                              }
                              if on_course {
                                  Link { to: Route::Courses {}, class: link_class(Route::Courses {}), "{t(lang(), \"nav.course\")}" }
                              }
                              if on_blog {
                                  Link { to: Route::BlogIndex {}, class: link_class(Route::BlogIndex {}), "{t_blog}" }
                              }
                              if on_forum {
                                  Link { to: Route::TopicsIndex {}, class: link_class(Route::TopicsIndex {}), "{t_forum}" }
                              }
                          }
                      }

                      div { class: "flex items-center gap-2 sm:gap-3",
                          // Phase 9.4: mobile hamburger（lg:hidden）。点击展开 header
                          // 下方的板块抽屉，让窄屏用户能直接跳到 8 个板块。
                          button {
                              class: "lg:hidden p-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-300 transition-colors",
                              onclick: move |_| show_mobile_menu.set(!show_mobile_menu()),
                              aria_label: "Toggle navigation menu",
                              if show_mobile_menu() {
                                  svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M6 18L18 6M6 6l12 12" }
                                  }
                              } else {
                                  svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M4 6h16M4 12h16M4 18h16" }
                                  }
                              }
                          }

                          // Search
                          SearchButton {}

                          // Phase 3.1: Theme switcher
                          ThemePicker {}

                          // Language Picker（下拉，便于后续支持更多语言）
                          LangPicker {}

                          // Dark Mode Toggle
                          button {
                              onclick: toggle_dark,
                              class: "p-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors",
                              if is_dark() {
                                  svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" }
                                  }
                              } else {
                                  svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" }
                                  }
                              }
                          }

                          // User avatar / Sign In
                          if let Some(ref u) = session_user() {
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
                                  if show_user_menu() {
                                      div { class: "absolute right-0 top-full mt-1 w-44 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-lg py-1 z-50",
                                          div { class: "px-3 py-2 text-xs text-slate-500 border-b border-slate-100 dark:border-slate-800",
                                              "{u.nickname}"
                                          }
                                          if on_forum {
                                              Link {
                                                  to: Route::MyTopics {},
                                                  class: "block px-3 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                                  "{t(lang(), \"user.my_topics\")}"
                                              }
                                          }
                                          Link {
                                              to: Route::MyAnnotations {},
                                              class: "block px-3 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                                              "{t(lang(), \"user.my_annotations\")}"
                                          }
                                          if u.is_admin() {
                                              div { class: "my-1 border-t border-slate-100 dark:border-slate-800" }
                                              Link {
                                                  to: Route::AdminDashboard {},
                                                  class: "block px-3 py-2 text-sm font-semibold text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors",
                                                  "{t(lang(), \"nav.admin\")}"
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
                              button {
                                  onclick: move |_| show_auth_modal.set(true),
                                  class: "inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium text-slate-700 hover:text-slate-900 hover:bg-slate-100 dark:text-slate-300 dark:hover:text-white dark:hover:bg-slate-800 transition-colors whitespace-nowrap",
                                  svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.501 20.118a7.5 7.5 0 0114.998 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.499-1.632z" }
                                  }
                                  "{t(lang(), \"auth.sign_in\")}"
                              }
                          }

                          if on_docs {
                              Link {
                                  to: Route::Docs {},
                                  class: "hidden sm:inline-flex items-center rounded-md btn-flow px-3 py-2 text-sm font-semibold whitespace-nowrap transition-all",
                                  "{t(lang(), \"nav.start\")}"
                              }
                          }
                      }
                  }

                  // Mobile 抽屉（<lg）：双生态分组（标题 + 该生态领域）在前，
                  // 内容类型入口（案例/课程/博客/播客/论坛）在后；点链接后自动收起。
                  if show_mobile_menu() {
                      nav { class: "lg:hidden border-t border-slate-200/70 dark:border-slate-800 py-2 flex flex-col text-sm font-medium",
                          // 两个生态：标题 + 已启用领域链接
                          for eco in ecosystems() {
                              {
                                  let domains: Vec<_> = eco.domains.iter().filter(|d| enabled.iter().any(|e| e == d.module_id)).cloned().collect();
                                  rsx! {
                                      if !domains.is_empty() {
                                          p { key: "{eco.id}-h", class: "px-2 pt-2 pb-1 text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500", "{t(lang(), eco.label_key)}" }
                                          for d in domains.iter() {
                                              Link { key: "{d.id}", to: d.route.clone(), class: "px-3 py-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors", onclick: move |_| show_mobile_menu.set(false), "{t(lang(), d.label_key)}" }
                                          }
                                      }
                                  }
                              }
                          }
                          div { class: "my-2 border-t border-slate-200/70 dark:border-slate-800" }
                          // 内容类型入口
                          if on_cases {
                              Link { to: Route::Cases {}, class: "px-2 py-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors", onclick: move |_| show_mobile_menu.set(false), "{t(lang(), \"nav.cases\")}" }
                          }
                          if on_course {
                              Link { to: Route::Courses {}, class: "px-2 py-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors", onclick: move |_| show_mobile_menu.set(false), "{t(lang(), \"nav.course\")}" }
                          }
                          if on_blog {
                              Link { to: Route::BlogIndex {}, class: "px-2 py-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors", onclick: move |_| show_mobile_menu.set(false), "{t_blog}" }
                          }
                          if on_podcast {
                              Link { to: Route::Podcast {}, class: "px-2 py-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors", onclick: move |_| show_mobile_menu.set(false), "{t_podcast}" }
                          }
                          if on_forum {
                              Link { to: Route::TopicsIndex {}, class: "px-2 py-2 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors", onclick: move |_| show_mobile_menu.set(false), "{t_forum}" }
                          }
                          if on_docs {
                              Link { to: Route::Docs {}, class: "px-2 py-2 mt-1 rounded-md btn-flow text-center font-semibold transition-all", onclick: move |_| show_mobile_menu.set(false), "{t(lang(), \"nav.start\")}" }
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
                          span { "{t(lang(), \"footer.tagline\")}" }
                      }
                      div { class: "flex gap-4",
                          if on_forum {
                              Link { to: Route::TopicsIndex {}, class: "hover:text-slate-900 dark:hover:text-white transition-colors", "Topics" }
                          }
                          if on_blog {
                              Link { to: Route::BlogIndex {}, class: "hover:text-slate-900 dark:hover:text-white transition-colors", "Blog" }
                          }
                          if on_docs {
                              Link { to: Route::Docs {}, class: "hover:text-slate-900 dark:hover:text-white transition-colors", "Docs" }
                          }
                      }
                  }
              }
          }
      }
  }
}
// trigger rebuild 1780294934
