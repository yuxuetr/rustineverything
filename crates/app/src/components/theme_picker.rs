//! Phase 3.1：ThemePicker — Navbar 中的主题切换下拉。
//!
//! 通过 `list_available_themes` server fn 拉取插件目录下的可用主题，
//! 点击后调用 `set_user_theme` 写入 cookie 并 bump `ThemeVersion` Signal，
//! 触发上层 `theme_css` `use_resource` 重新请求合并 CSS。

use dioxus::document::eval;
use dioxus::prelude::*;

use crate::i18n::{t, use_i18n};
use crate::server::{list_available_themes, set_user_theme, ThemeInfo};

/// 主题 cookie 名（与后端 `THEME_COOKIE_NAME` 保持一致）。
const THEME_COOKIE_NAME: &str = "site_theme";

/// 去掉主题名的 `Theme ` 前缀，只留简短名（如 `Theme Catppuccin` → `Catppuccin`），
/// 避免按钮 / 下拉项过长换行。
fn short_theme_label(label: &str) -> String {
  label.strip_prefix("Theme ").unwrap_or(label).trim().to_string()
}

/// 全局主题版本号：每次切换主题 +1，App 组件中的 `use_resource` 依赖该值，
/// 使聚合 CSS 在用户切换主题后立即重取。
#[derive(Clone, Copy)]
pub struct ThemeVersion(pub Signal<u32>);

/// 在 App 根上挂载 `ThemeVersion` context，返回 Signal 以便上层订阅。
pub fn use_theme_version_provider() -> Signal<u32> {
  let sig = use_signal(|| 0u32);
  use_context_provider(|| ThemeVersion(sig));
  sig
}

/// 子组件读取当前 ThemeVersion Signal。
pub fn use_theme_version() -> Signal<u32> {
  use_context::<ThemeVersion>().0
}

/// 主题下拉。展示当前激活主题，点击展开列表，点击列表项写 cookie + bump version。
#[component]
pub fn ThemePicker() -> Element {
  let mut open = use_signal(|| false);
  let mut version = use_theme_version();
  let lang = use_i18n();

  // 拉取可用主题列表，依赖 version 以便切换后刷新激活态。
  let themes_res = use_resource(move || {
    let _v = version();
    async move { list_available_themes().await.unwrap_or_default() }
  });
  let themes: Vec<ThemeInfo> = themes_res.read().as_ref().cloned().unwrap_or_default();

  // 找到当前激活主题用于按钮 label（只取去前缀后的短名）
  let active_label = themes
    .iter()
    .find(|t| t.is_active)
    .map(|t| short_theme_label(&t.label))
    .unwrap_or_else(|| t(lang(), "theme.heading"));

  // 不显示 picker 当只有 ≤ 1 个主题（无切换意义）
  if themes.len() <= 1 {
    return rsx! {};
  }

  // 切换主题：写 cookie 后 bump version 以迫使上层重拼 CSS。
  // 该闭包被后续多个 button.onclick 共享使用，所以需要 Copy + impl Fn。
  //
  // 持久化采用「双写」策略，修复「切不动 + 刷新回退」：
  // 1. `set_user_theme`：服务端校验文件名并下发 Set-Cookie（桌面端走 reqwest
  //    cookie jar，跨平台兜底）。
  // 2. `document.cookie`：Web 端直接写入非 HttpOnly cookie，**等待写入完成**再
  //    bump version，确保紧接着的聚合 CSS 重新请求一定携带新 cookie；刷新后 SSR
  //    也能读到，主题保持不变。
  let switch = use_callback(move |filename: String| {
    open.set(false);
    spawn(async move {
      // 1) 服务端校验 + Set-Cookie（best-effort，失败仅记日志，不阻断切换）。
      if let Err(e) = set_user_theme(filename.clone()).await {
        tracing::error!(error = %e, "theme picker: set_user_theme failed");
      }

      // 2) 客户端兜底写 cookie，并等待 JS 执行完成。
      let trimmed = filename.trim();
      let js = if trimmed.is_empty() {
        format!(
          "document.cookie = '{name}=; path=/; max-age=0; samesite=lax'; dioxus.send(true);",
          name = THEME_COOKIE_NAME
        )
      } else {
        format!(
          "document.cookie = '{name}={value}; path=/; max-age=31536000; samesite=lax'; dioxus.send(true);",
          name = THEME_COOKIE_NAME,
          value = trimmed
        )
      };
      let mut handle = eval(&js);
      let _ = handle.recv::<bool>().await;

      // 3) cookie 就绪后再触发上层重新拉取聚合 CSS。
      version.with_mut(|v| *v += 1);
    });
  });

  rsx! {
      div { class: "relative",
          button {
              onclick: move |_| open.set(!open()),
              class: "flex items-center gap-1 px-2 py-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors text-xs font-semibold",
              title: "{t(lang(), \"theme.toggle\")}",
              svg {
                  class: "w-4 h-4",
                  fill: "none",
                  stroke: "currentColor",
                  view_box: "0 0 24 24",
                  path {
                      stroke_linecap: "round",
                      stroke_linejoin: "round",
                      stroke_width: "2",
                      d: "M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01"
                  }
              }
              span { class: "hidden sm:inline whitespace-nowrap", "{active_label}" }
          }
          if open() {
              div { class: "absolute right-0 top-full mt-1 w-48 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-lg py-1 z-50",
                  div { class: "px-3 py-1.5 text-[10px] uppercase tracking-wider text-slate-400",
                      "{t(lang(), \"theme.heading\")}"
                  }
                  for t in themes.iter() {
                      {
                          let is_active = t.is_active;
                          let filename = t.filename.clone();
                          let label = short_theme_label(&t.label);
                          rsx! {
                              button {
                                  key: "{filename}",
                                  onclick: move |_| switch.call(filename.clone()),
                                  class: format_args!(
                                      "w-full text-left px-3 py-1.5 text-sm transition-colors flex items-center justify-between {}",
                                      if is_active {
                                          "text-blue-600 dark:text-blue-400 font-semibold bg-blue-50 dark:bg-blue-900/30"
                                      } else {
                                          "text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                                      }
                                  ),
                                  span { "{label}" }
                                  if is_active {
                                      span { class: "text-xs", "✓" }
                                  }
                              }
                          }
                      }
                  }
                  div { class: "my-1 border-t border-slate-100 dark:border-slate-800" }
                  button {
                      onclick: move |_| switch.call(String::new()),
                      class: "w-full text-left px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                      "{t(lang(), \"theme.reset\")}"
                  }
              }
          }
      }
  }
}
