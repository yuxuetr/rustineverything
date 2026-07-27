use dioxus::prelude::*;

pub use app_core::i18n::Language;

pub fn use_i18n() -> Signal<Language> {
  use_context::<Signal<Language>>()
}

pub fn init_i18n() {
  use_context_provider(|| Signal::new(Language::Zh));
}

/// 同步翻译。委托给 `app_core::i18n::t`（字典来自 `assets/i18n/{zh,en}.ftl`，
/// 编译期内嵌）。所有 key 维护在配置文件里，这里只做转发。
pub fn t(lang: Language, key: &str) -> String {
  app_core::i18n::t(lang, key)
}
