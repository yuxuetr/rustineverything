//! Phase 3.3：Minimal 布局 — 紧凑顶部条 + 无 Footer + 无主导航。
//!
//! 适合写作 / 阅读优先场景。仅保留 Logo（首页链接）+ 搜索 + 主题切换 +
//! 语言 + 暗色 + 登录入口；不渲染主导航栏列表，也不渲染 Footer。

use dioxus::prelude::*;
use dioxus::router::{Link, Outlet};
use dioxus_shadcn::{Button, ButtonSize, ButtonVariant};

use crate::components::lang_picker::LangPicker;
use crate::components::theme_picker::ThemePicker;
use crate::components::user_menu::UserMenu;
use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use crate::routes::Route;
use module_search::search::SearchButton;

/// Minimal shell：极简顶部条，`Outlet::<Route>` 主导内容；无 Footer。
#[component]
pub fn MinimalShell() -> Element {
  let lang = use_i18n();
  let mut is_dark = use_signal(|| false);
  let mut show_auth_modal = crate::use_auth_modal();
  let session_user = crate::use_session_user();

  // 与 Classic 共享 dark 模式初始化逻辑
  use_effect(move || {
    let dark = widgets::browser::dark_mode_preference();
    widgets::browser::set_class(true, "dark", dark);
    is_dark.set(dark);
  });

  let toggle_dark = move |_| {
    let new_val = !is_dark();
    is_dark.set(new_val);
    widgets::browser::set_dark_mode(new_val);
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
                          // 紧凑条保持原 28px（FB-21：默认 min-h-10）
                          Button {
                              r#type: "button",
                              variant: ButtonVariant::Ghost,
                              size: ButtonSize::Icon,
                              class: "h-7 w-7 min-h-7 text-muted-foreground",
                              onclick: toggle_dark,
                              "aria-label": "{t(lang(), \"nav.toggle_dark\")}",
                              "aria-pressed": is_dark().to_string(),
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
                              UserMenu { user: u.clone(), compact: true, show_my_topics: false }
                          } else {
                              Button {
                                  r#type: "button",
                                  variant: ButtonVariant::Ghost,
                                  size: ButtonSize::Sm,
                                  class: "h-7 min-h-7 px-2 text-xs",
                                  onclick: move |_| show_auth_modal.set(true),
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
