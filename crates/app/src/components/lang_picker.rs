//! 语言切换下拉（LangPicker）。
//!
//! 把原先的「单按钮 toggle（EN / 中）」升级为下拉菜单，视觉上与
//! [`crate::components::theme_picker::ThemePicker`] 对齐。
//!
//! 语言通过数据驱动的 [`LANGUAGES`] 列表声明 —— 后续支持更多语言时，
//! 只需在 `app_core::i18n::Language` 增加枚举值，并在此追加一行即可，
//! 无需改动渲染逻辑。

use dioxus::document::eval;
use dioxus::prelude::*;

use crate::i18n::{use_i18n, Language};

/// 语言 cookie 名（与 App 启动时的恢复逻辑保持一致）。
/// 论坛等模块用 `<a href>` 整页跳转会重置内存中的语言信号，
/// 写入 cookie 后可在下次加载时恢复，避免回退到中文。
const LANG_COOKIE_NAME: &str = "site_lang";

/// 可选语言表：`(枚举, 顶部按钮短标签, 下拉项完整名称)`。
///
/// 顺序即下拉展示顺序。新增语言只需在此追加一行（并扩展 `Language`）。
const LANGUAGES: &[(Language, &str, &str)] =
  &[(Language::Zh, "中", "中文"), (Language::En, "EN", "English")];

/// 语言下拉。按钮展示当前语言短标签，点击展开列表选择。
#[component]
pub fn LangPicker() -> Element {
  let mut open = use_signal(|| false);
  let mut lang = use_i18n();

  let current = lang();
  let current_label =
    LANGUAGES.iter().find(|(l, _, _)| *l == current).map(|(_, short, _)| *short).unwrap_or("中");

  rsx! {
      div { class: "relative",
          button {
              onclick: move |_| open.set(!open()),
              class: "flex items-center gap-1 px-2 py-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors text-xs font-semibold",
              title: "切换语言 / Language",
              svg {
                  class: "w-4 h-4",
                  fill: "none",
                  stroke: "currentColor",
                  view_box: "0 0 24 24",
                  path {
                      stroke_linecap: "round",
                      stroke_linejoin: "round",
                      stroke_width: "2",
                      d: "M3 5h12M9 3v2m1.048 9.5A18.022 18.022 0 016.412 9m6.088 9h7M11 21l5-10 5 10M12.751 5C11.783 10.77 8.07 15.61 3 18.129"
                  }
              }
              span { "{current_label}" }
          }
          if open() {
              div { class: "absolute right-0 top-full mt-1 w-40 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-lg py-1 z-50",
                  div { class: "px-3 py-1.5 text-[10px] uppercase tracking-wider text-slate-400",
                      "语言"
                  }
                  for (l, _short, full) in LANGUAGES.iter() {
                      {
                          let lang_val = *l;
                          let is_active = current == lang_val;
                          let full_label = *full;
                          rsx! {
                              button {
                                  key: "{full_label}",
                                  onclick: move |_| {
                                      lang.set(lang_val);
                                      open.set(false);
                                      // 持久化语言选择：写 cookie，让整页跳转 / 刷新后仍保持。
                                      let code = if lang_val == Language::En { "en" } else { "zh" };
                                      let _ = eval(&format!(
                                          "document.cookie = '{name}={code}; path=/; max-age=31536000; samesite=lax';",
                                          name = LANG_COOKIE_NAME,
                                      ));
                                  },
                                  class: format_args!(
                                      "w-full text-left px-3 py-1.5 text-sm transition-colors flex items-center justify-between {}",
                                      if is_active {
                                          "text-blue-600 dark:text-blue-400 font-semibold bg-blue-50 dark:bg-blue-900/30"
                                      } else {
                                          "text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                                      }
                                  ),
                                  span { "{full_label}" }
                                  if is_active {
                                      span { class: "text-xs", "✓" }
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
