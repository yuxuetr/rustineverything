use std::cmp::Ordering;

pub const KNOWN_CATEGORIES: &[&str] = &[
  "frontend",
  "backend",
  "fullstack",
  "cli",
  "embedded",
  "ai",
  "web3",
  "library",
  "tool",
  "desktop",
];

pub const KNOWN_TAGS: &[&str] = &[
  "axum",
  "actix",
  "dioxus",
  "tauri",
  "leptos",
  "tokio",
  "sea-orm",
  "wasm",
  "cli",
  "embedded",
  "web3",
  "ai",
  "fullstack",
  "library",
  "opensource",
  "commercial",
  "favorite",
];

pub trait CaseSortable {
  fn favorite(&self) -> bool;
  fn stars(&self) -> i64;
  fn date_added(&self) -> &str;
  fn name(&self) -> &str;
}

pub fn normalize_tag(raw: &str) -> String {
  raw
    .trim()
    .to_ascii_lowercase()
    .chars()
    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
    .collect()
}

pub fn normalize_category(raw: &str) -> Option<String> {
  let value = normalize_tag(raw);
  if KNOWN_CATEGORIES.iter().any(|c| *c == value) {
    Some(value)
  } else {
    None
  }
}

pub fn is_known_tag(tag: &str) -> bool {
  KNOWN_TAGS.contains(&tag)
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
  let mut out: Vec<String> =
    tags.iter().map(|tag| normalize_tag(tag)).filter(|tag| !tag.is_empty()).collect();
  out.sort();
  out.dedup();
  out
}

pub fn matches_query(
  name: &str,
  description: &str,
  category: &str,
  tags: &[String],
  query: &str,
) -> bool {
  let q = query.trim().to_lowercase();
  if q.is_empty() {
    return true;
  }
  name.to_lowercase().contains(&q)
    || description.to_lowercase().contains(&q)
    || category.to_lowercase().contains(&q)
    || tags.iter().any(|tag| tag.to_lowercase().contains(&q))
}

pub fn compare_cases<A: CaseSortable, B: CaseSortable>(a: &A, b: &B) -> Ordering {
  b.favorite()
    .cmp(&a.favorite())
    .then_with(|| b.stars().cmp(&a.stars()))
    .then_with(|| b.date_added().cmp(a.date_added()))
    .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
}

pub fn humanize_slug(slug: &str) -> String {
  slug
    .split(['-', '_'])
    .filter(|part| !part.is_empty())
    .map(|part| {
      let mut chars = part.chars();
      match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug)]
  struct SortCase {
    name: String,
    favorite: bool,
    stars: i64,
    date_added: String,
  }

  impl CaseSortable for SortCase {
    fn favorite(&self) -> bool {
      self.favorite
    }

    fn stars(&self) -> i64 {
      self.stars
    }

    fn date_added(&self) -> &str {
      &self.date_added
    }

    fn name(&self) -> &str {
      &self.name
    }
  }

  #[test]
  fn normalize_tag_keeps_safe_chars() {
    assert_eq!(normalize_tag(" Sea ORM!! "), "seaorm");
    assert_eq!(normalize_tag("sea-orm_core"), "sea-orm_core");
  }

  #[test]
  fn normalize_tags_dedups_and_drops_empty() {
    let tags = vec!["Rust!".to_string(), "rust".to_string(), "  ".to_string(), "AI".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["ai".to_string(), "rust".to_string()]);
  }

  #[test]
  fn normalize_category_accepts_known() {
    assert_eq!(normalize_category(" AI ").as_deref(), Some("ai"));
    assert_eq!(normalize_category("backend").as_deref(), Some("backend"));
  }

  #[test]
  fn normalize_category_rejects_unknown() {
    assert!(normalize_category("unknown").is_none());
  }

  #[test]
  fn matches_query_checks_name_description_category_and_tags() {
    let tags = vec!["axum".to_string(), "sea-orm".to_string()];
    assert!(matches_query("RealWorld", "Backend API", "backend", &tags, "real"));
    assert!(matches_query("RealWorld", "Backend API", "backend", &tags, "api"));
    assert!(matches_query("RealWorld", "Backend API", "backend", &tags, "back"));
    assert!(matches_query("RealWorld", "Backend API", "backend", &tags, "orm"));
    assert!(!matches_query("RealWorld", "Backend API", "backend", &tags, "dioxus"));
  }

  #[test]
  fn matches_query_supports_chinese() {
    let tags = vec!["ai".to_string()];
    assert!(matches_query("Rust AI 案例", "本地模型推理", "ai", &tags, "模型"));
  }

  #[test]
  fn compare_cases_prioritizes_favorite() {
    let a = SortCase {
      name: "A".to_string(),
      favorite: false,
      stars: 100,
      date_added: "2026-01-01".to_string(),
    };
    let b = SortCase {
      name: "B".to_string(),
      favorite: true,
      stars: 1,
      date_added: "2024-01-01".to_string(),
    };
    assert_eq!(compare_cases(&a, &b), Ordering::Greater);
  }

  #[test]
  fn compare_cases_then_stars_date_and_name() {
    let mut cases = [SortCase {
        name: "beta".to_string(),
        favorite: false,
        stars: 5,
        date_added: "2025-01-01".to_string(),
      },
      SortCase {
        name: "alpha".to_string(),
        favorite: false,
        stars: 5,
        date_added: "2025-01-01".to_string(),
      },
      SortCase {
        name: "zeta".to_string(),
        favorite: false,
        stars: 10,
        date_added: "2024-01-01".to_string(),
      },
      SortCase {
        name: "newer".to_string(),
        favorite: false,
        stars: 5,
        date_added: "2026-01-01".to_string(),
      }];
    cases.sort_by(compare_cases);
    let names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
    assert_eq!(names, vec!["zeta", "newer", "alpha", "beta"]);
  }

  #[test]
  fn humanize_slug_title_cases_words() {
    assert_eq!(humanize_slug("dioxus-fullstack_template"), "Dioxus Fullstack Template");
  }
}
