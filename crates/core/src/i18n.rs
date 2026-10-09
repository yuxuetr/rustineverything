use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Shared language enum used across crates (app, forum modules, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
  En,
  #[default]
  Zh,
}

impl Language {
  /// Pick between two literals by language. Handy for short inline bilingual text.
  pub fn pick<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
    match self {
      Language::En => en,
      Language::Zh => zh,
    }
  }
}

// 方案 A：翻译字典集中在 `assets/i18n/{zh,en}.ftl`，编译期内嵌进 app_core，
// 运行时解析一次并同步查表。app 与所有内容模块共用同一份 `t()`，
// 服务端 (SSR) 与客户端 (hydration) 结果一致，无往返、无闪烁。
const FTL_ZH: &str = include_str!("../../../assets/i18n/zh.ftl");
const FTL_EN: &str = include_str!("../../../assets/i18n/en.ftl");

static DICT_ZH: OnceLock<HashMap<String, String>> = OnceLock::new();
static DICT_EN: OnceLock<HashMap<String, String>> = OnceLock::new();

/// 解析简单的 `key = value` 字典：
/// - 空行与以 `#` 开头的行被忽略；
/// - 在第一个 `=` 处切分；key 两端去空白；
/// - value 仅去掉 `=` 后的单个前导空格，**保留尾随空格**，
///   以便像 `"Failed to load: "` 这样的分隔符得以保留。
fn parse_dict(src: &str) -> HashMap<String, String> {
  let mut map = HashMap::new();
  for line in src.lines() {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
      continue;
    }
    if let Some((key, value)) = line.split_once('=') {
      let key = key.trim();
      if key.is_empty() {
        continue;
      }
      let value = value.strip_prefix(' ').unwrap_or(value);
      map.insert(key.to_string(), value.to_string());
    }
  }
  map
}

fn dict(lang: Language) -> &'static HashMap<String, String> {
  match lang {
    Language::En => DICT_EN.get_or_init(|| parse_dict(FTL_EN)),
    Language::Zh => DICT_ZH.get_or_init(|| parse_dict(FTL_ZH)),
  }
}

/// 按语言翻译 `key`。缺失时回退到中文值，再回退到 `key` 本身（绝不 panic）。
pub fn t(lang: Language, key: &str) -> String {
  if let Some(v) = dict(lang).get(key) {
    return v.clone();
  }
  if lang != Language::Zh {
    if let Some(v) = dict(Language::Zh).get(key) {
      return v.clone();
    }
  }
  key.to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_skips_comments_and_blanks() {
    let src = "# comment\n\nfoo = bar\n  # indented comment\nbaz = qux\n";
    let map = parse_dict(src);
    assert_eq!(map.get("foo").map(String::as_str), Some("bar"));
    assert_eq!(map.get("baz").map(String::as_str), Some("qux"));
    assert_eq!(map.len(), 2);
  }

  #[test]
  fn parse_splits_on_first_equals_only() {
    let map = parse_dict("k = a = b\n");
    assert_eq!(map.get("k").map(String::as_str), Some("a = b"));
  }

  #[test]
  fn parse_preserves_trailing_space() {
    let map = parse_dict("k = val \n");
    assert_eq!(map.get("k").map(String::as_str), Some("val "));
  }

  #[test]
  fn pick_selects_by_language() {
    assert_eq!(Language::Zh.pick("中", "en"), "中");
    assert_eq!(Language::En.pick("中", "en"), "en");
  }

  #[test]
  fn t_returns_translations_and_falls_back() {
    assert_eq!(t(Language::Zh, "nav.blog"), "博客");
    assert_eq!(t(Language::En, "nav.blog"), "Blog");
    assert_eq!(t(Language::En, "does.not.exist"), "does.not.exist");
  }

  #[test]
  fn zh_and_en_key_sets_match() {
    let zh = parse_dict(FTL_ZH);
    let en = parse_dict(FTL_EN);
    let mut missing_in_en: Vec<&String> = zh.keys().filter(|k| !en.contains_key(*k)).collect();
    let mut missing_in_zh: Vec<&String> = en.keys().filter(|k| !zh.contains_key(*k)).collect();
    missing_in_en.sort();
    missing_in_zh.sort();
    assert!(missing_in_en.is_empty(), "en.ftl 缺少这些键: {:?}", missing_in_en);
    assert!(missing_in_zh.is_empty(), "zh.ftl 缺少这些键: {:?}", missing_in_zh);
  }
}
