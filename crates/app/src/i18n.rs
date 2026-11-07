use dioxus::prelude::*;

pub use app_core::i18n::Language;

pub fn use_i18n() -> Signal<Language> {
  use_context::<Signal<Language>>()
}

/// 界面语言 context。首屏语言在服务端就定下来（docs/I18N_URLS.md）：
/// `?lang=` > `site_lang` cookie > 中文。SSR 与 hydration 用同一规则，不再先中文后切。
pub fn init_i18n() {
  use_context_provider(|| Signal::new(initial_language()));
}

/// LangPicker 写入的 cookie 名
pub const LANG_COOKIE: &str = "site_lang";

/// 当前请求的 `lang` 查询参数与 `site_lang` cookie：服务端读请求，浏览器读 location / document.cookie。
fn initial_language() -> Language {
  #[cfg(feature = "server")]
  {
    use dioxus::fullstack::FullstackContext;
    let Some(ctx) = FullstackContext::current() else { return Language::Zh };
    let parts = ctx.parts_mut();
    let query = parts
      .uri
      .query()
      .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("lang=")).map(str::to_string));
    let cookie = parts
      .headers
      .get_all("cookie")
      .iter()
      .filter_map(|v| v.to_str().ok())
      .flat_map(|raw| raw.split(';'))
      .find_map(|kv| kv.trim().strip_prefix(LANG_COOKIE)?.strip_prefix('='))
      .map(str::to_string);
    drop(parts);
    pick_language(query.as_deref(), cookie.as_deref())
  }
  #[cfg(not(feature = "server"))]
  {
    pick_language(
      widgets::browser::query_param("lang").as_deref(),
      widgets::browser::cookie(LANG_COOKIE).as_deref(),
    )
  }
}

/// `?lang=` 优先，其次 cookie；只认 `en` / `zh`，其它值忽略。
pub fn pick_language(query: Option<&str>, cookie: Option<&str>) -> Language {
  let parse = |v: Option<&str>| match v {
    Some("en") => Some(Language::En),
    Some("zh") => Some(Language::Zh),
    _ => None,
  };
  parse(query).or(parse(cookie)).unwrap_or(Language::Zh)
}

/// 同步翻译。委托给 `app_core::i18n::t`（字典来自 `assets/i18n/{zh,en}.ftl`，
/// 编译期内嵌）。所有 key 维护在配置文件里，这里只做转发。
pub fn t(lang: Language, key: &str) -> String {
  app_core::i18n::t(lang, key)
}

#[cfg(test)]
mod tests {
  use super::{pick_language, Language};

  #[test]
  fn query_beats_cookie_and_unknown_values_are_ignored() {
    assert_eq!(pick_language(Some("en"), Some("zh")), Language::En);
    assert_eq!(pick_language(Some("zh"), Some("en")), Language::Zh);
    assert_eq!(pick_language(None, Some("en")), Language::En);
    assert_eq!(pick_language(Some("fr"), Some("en")), Language::En);
    assert_eq!(pick_language(None, None), Language::Zh);
  }
}
