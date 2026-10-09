//! 板块元数据 + 纯逻辑（无 IO / 无 dioxus），可独立单测。
//!
//! 方案 A：展示文案（label / blurb / tagline）统一由 `app_core::i18n` 从
//! `assets/i18n/{zh,en}.ftl` 提供。这里只保留结构性数据（slug / crate 名 / url）。

pub const BOARD_ID: &str = "cli";
pub const BOARD_ROUTE: &str = "/cli";

/// 一个子主题。展示用 label / blurb 经 i18n key `{BOARD_ID}.sub.{slug}.*` 查表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subtopic {
  pub slug: &'static str,
}

/// 一个精选 crate。`name` 为品牌名（语言中性），`url` 为外链；blurb 经 i18n key
/// `{BOARD_ID}.crate.{normalize_tag(name)}.blurb` 查表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeaturedCrate {
  pub name: &'static str,
  pub url: &'static str,
}

pub const SUBTOPICS: &[Subtopic] = &[
  Subtopic { slug: "args" },
  Subtopic { slug: "tui" },
  Subtopic { slug: "output" },
  Subtopic { slug: "config" },
  Subtopic { slug: "testing" },
  Subtopic { slug: "distribution" },
];

pub const FEATURED_CRATES: &[FeaturedCrate] = &[
  FeaturedCrate { name: "clap", url: "https://github.com/clap-rs/clap" },
  FeaturedCrate { name: "ratatui", url: "https://ratatui.rs" },
  FeaturedCrate { name: "indicatif", url: "https://github.com/console-rs/indicatif" },
  FeaturedCrate { name: "console", url: "https://github.com/console-rs/console" },
  FeaturedCrate { name: "crossterm", url: "https://github.com/crossterm-rs/crossterm" },
  FeaturedCrate { name: "assert_cmd", url: "https://github.com/assert-rs/assert_cmd" },
];

/// 文章排序所需的最小契约，[`text`] 测试与 [`server`] 列表共用。
pub trait DatedArticle {
  fn date(&self) -> &str;
  fn title(&self) -> &str;
}

/// 归一化标签：trim + 小写，仅保留字母数字 / `-` / `_`。
/// 也用于把 crate `name` 映射为稳定的 i18n key 片段。
pub fn normalize_tag(raw: &str) -> String {
  raw
    .trim()
    .to_ascii_lowercase()
    .chars()
    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
    .collect()
}

/// 归一化一组标签：去空、去重、排序。
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
  let mut out: Vec<String> =
    tags.iter().map(|t| normalize_tag(t)).filter(|t| !t.is_empty()).collect();
  out.sort();
  out.dedup();
  out
}

/// 全文匹配：标题 / 描述 / 标签任一命中即返回 true。空查询返回 true。
pub fn matches_query(title: &str, description: &str, tags: &[String], query: &str) -> bool {
  let q = query.trim().to_lowercase();
  if q.is_empty() {
    return true;
  }
  title.to_lowercase().contains(&q)
    || description.to_lowercase().contains(&q)
    || tags.iter().any(|t| t.to_lowercase().contains(&q))
}

/// 按日期降序、再按标题升序排序（稳定）。
pub fn sort_by_date_desc<T: DatedArticle>(items: &mut [T]) {
  items.sort_by(|a, b| {
    b.date().cmp(a.date()).then_with(|| a.title().to_lowercase().cmp(&b.title().to_lowercase()))
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  struct A {
    date: String,
    title: String,
  }
  impl DatedArticle for A {
    fn date(&self) -> &str {
      &self.date
    }
    fn title(&self) -> &str {
      &self.title
    }
  }

  #[test]
  fn board_constants_well_formed() {
    assert_eq!(BOARD_ID, "cli");
    assert!(BOARD_ROUTE.starts_with('/'));
  }

  #[test]
  fn subtopics_have_unique_slugs() {
    let mut slugs: Vec<&str> = SUBTOPICS.iter().map(|s| s.slug).collect();
    let n = slugs.len();
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), n, "子主题 slug 应唯一");
  }

  #[test]
  fn subtopics_are_non_empty() {
    assert!(SUBTOPICS.len() >= 4);
    for s in SUBTOPICS {
      assert!(!s.slug.is_empty());
    }
  }

  #[test]
  fn subtopic_slugs_are_url_safe() {
    for s in SUBTOPICS {
      assert_eq!(normalize_tag(s.slug), s.slug, "slug 应已是归一化形态");
    }
  }

  #[test]
  fn featured_crates_use_https() {
    assert!(FEATURED_CRATES.len() >= 4);
    for c in FEATURED_CRATES {
      assert!(c.url.starts_with("https://"), "{} 应为 https URL", c.name);
      assert!(!c.name.is_empty());
    }
  }

  #[test]
  fn featured_crates_unique_names() {
    let mut names: Vec<&str> = FEATURED_CRATES.iter().map(|c| c.name).collect();
    let n = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), n);
  }

  #[test]
  fn featured_crate_i18n_keys_unique() {
    let mut keys: Vec<String> = FEATURED_CRATES.iter().map(|c| normalize_tag(c.name)).collect();
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n, "crate 的 i18n key（normalize_tag(name)）应唯一");
  }

  #[test]
  fn normalize_tag_lowercases_and_strips() {
    assert_eq!(normalize_tag(" Clap!! "), "clap");
    assert_eq!(normalize_tag("assert_cmd"), "assert_cmd");
  }

  #[test]
  fn normalize_tags_dedups_and_drops_empty() {
    let tags =
      vec!["Clap".to_string(), "clap".to_string(), "   ".to_string(), "Ratatui".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["clap".to_string(), "ratatui".to_string()]);
  }

  #[test]
  fn matches_query_empty_returns_true() {
    assert!(matches_query("t", "d", &[], ""));
    assert!(matches_query("t", "d", &[], "   "));
  }

  #[test]
  fn matches_query_hits_title_description_tags() {
    let tags = vec!["clap".to_string(), "args".to_string()];
    assert!(matches_query("用 clap 解析参数", "子命令与标志", &tags, "clap"));
    assert!(matches_query("用 clap 解析参数", "子命令与标志", &tags, "子命令"));
    assert!(matches_query("用 clap 解析参数", "子命令与标志", &tags, "args"));
    assert!(!matches_query("用 clap 解析参数", "子命令与标志", &tags, "solana"));
  }

  #[test]
  fn matches_query_supports_chinese() {
    let tags = vec!["tui".to_string()];
    assert!(matches_query("终端 UI", "ratatui 交互界面", &tags, "终端"));
  }

  #[test]
  fn sort_by_date_desc_orders_newest_first_then_title() {
    let mut items = vec![
      A { date: "2026-01-01".into(), title: "beta".into() },
      A { date: "2026-03-01".into(), title: "zeta".into() },
      A { date: "2026-01-01".into(), title: "alpha".into() },
    ];
    sort_by_date_desc(&mut items);
    let order: Vec<&str> = items.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(order, vec!["zeta", "alpha", "beta"]);
  }
}
