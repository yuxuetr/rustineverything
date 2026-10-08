//! 语言切换下拉（LangPicker）。
//!
//! 把原先的「单按钮 toggle（EN / 中）」升级为下拉菜单，视觉上与
//! [`crate::components::theme_picker::ThemePicker`] 对齐。
//!
//! 语言通过数据驱动的 [`LANGUAGES`] 列表声明 —— 后续支持更多语言时，
//! 只需在 `app_core::i18n::Language` 增加枚举值，并在此追加一行即可，
//! 无需改动渲染逻辑。

use dioxus::prelude::*;
use dioxus_shadcn::{
  Dropdown, DropdownContent, DropdownLabel, DropdownRadioGroup, DropdownRadioItem, DropdownTrigger,
};

use crate::i18n::{t, use_i18n, Language};

/// 语言 cookie 名（与 App 启动时的恢复逻辑保持一致）。
/// 论坛等模块用 `<a href>` 整页跳转会重置内存中的语言信号，
/// 写入 cookie 后可在下次加载时恢复，避免回退到中文。
const LANG_COOKIE_NAME: &str = "site_lang";

/// 可选语言表：`(枚举, cookie 值, 顶部按钮短标签, 下拉项完整名称)`。
///
/// 顺序即下拉展示顺序。新增语言只需在此追加一行（并扩展 `Language`）。
const LANGUAGES: &[(Language, &str, &str, &str)] =
  &[(Language::Zh, "zh", "中", "中文"), (Language::En, "en", "EN", "English")];

/// 语言下拉。按钮展示当前语言短标签，点击展开列表选择。
#[component]
pub fn LangPicker() -> Element {
  let mut lang = use_i18n();

  let current = lang();
  let (current_code, current_label) = LANGUAGES
    .iter()
    .find(|(l, ..)| *l == current)
    .map(|(_, code, short, _)| (*code, *short))
    .unwrap_or(("zh", "中"));

  rsx! {
      Dropdown {
          DropdownTrigger {
              class: "flex items-center gap-1 px-2 py-1 rounded-md hover:bg-accent text-muted-foreground transition-colors text-xs font-semibold",
              title: "{t(current, \"lang.toggle\")}",
              svg {
                  class: "w-4 h-4",
                  fill: "none",
                  stroke: "currentColor",
                  view_box: "0 0 24 24",
                  "aria-hidden": "true",
                  path {
                      stroke_linecap: "round",
                      stroke_linejoin: "round",
                      stroke_width: "2",
                      d: "M3 5h12M9 3v2m1.048 9.5A18.022 18.022 0 016.412 9m6.088 9h7M11 21l5-10 5 10M12.751 5C11.783 10.77 8.07 15.61 3 18.129"
                  }
              }
              span { "{current_label}" }
          }
          // `fixed` 见 theme_picker.rs（FB-19）。
          DropdownContent { class: "fixed w-40",
              DropdownLabel { class: "text-[10px] uppercase tracking-wider", "{t(current, \"lang.heading\")}" }
              DropdownRadioGroup {
                  value: current_code.to_string(),
                  on_value_change: move |code: String| {
                      if let Some((l, code, ..)) = LANGUAGES.iter().find(|(_, c, ..)| *c == code) {
                          lang.set(*l);
                          // 持久化语言选择：写 cookie，让整页跳转 / 刷新后仍保持。
                          widgets::browser::set_cookie(LANG_COOKIE_NAME, code, 31_536_000);
                      }
                  },
                  for (_, code, _, full) in LANGUAGES.iter() {
                      DropdownRadioItem { key: "{code}", value: *code, "{full}" }
                  }
              }
          }
      }
  }
}
