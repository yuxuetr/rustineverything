//! 板块元数据 + 纯逻辑（无 IO / 无 dioxus），可独立单测。

/// 板块标识（= ModuleEngine spec id）。
pub const BOARD_ID: &str = "embedded";
/// 顶级导航标签。
pub const BOARD_LABEL: &str = "嵌入式";
/// SPA 路由前缀。
pub const BOARD_ROUTE: &str = "/embedded";
/// 落地页副标题。
pub const BOARD_TAGLINE: &str = "用 Rust 写裸机与实时系统：no_std、Embassy、RTIC 与主流 MCU 平台。";

/// 一个子主题（落地页用作筛选 chip + 介绍）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subtopic {
  pub slug: &'static str,
  pub label: &'static str,
  pub blurb: &'static str,
}

/// 一个精选 crate（落地页侧栏推荐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeaturedCrate {
  pub name: &'static str,
  pub blurb: &'static str,
  pub url: &'static str,
}

/// 本板块的子主题清单。
pub const SUBTOPICS: &[Subtopic] = &[
  Subtopic {
    slug: "no-std",
    label: "no_std",
    blurb: "脱离标准库，在没有操作系统的目标上运行 Rust。",
  },
  Subtopic {
    slug: "embassy",
    label: "Embassy",
    blurb: "嵌入式异步运行时，用 async/await 写中断驱动的固件。",
  },
  Subtopic {
    slug: "rtic", label: "RTIC", blurb: "基于硬件优先级的并发框架，零成本任务调度。"
  },
  Subtopic {
    slug: "hal",
    label: "HAL / PAC",
    blurb: "embedded-hal 抽象层与外设访问 crate，跨芯片复用驱动。",
  },
  Subtopic {
    slug: "defmt",
    label: "日志与调试",
    blurb: "defmt 高效日志 + probe-rs 烧录调试工作流。",
  },
  Subtopic {
    slug: "platforms",
    label: "平台",
    blurb: "RP2040 / STM32 / ESP32 / nRF 等主流 MCU 平台实践。",
  },
];

/// 本板块的精选 crate。
pub const FEATURED_CRATES: &[FeaturedCrate] = &[
  FeaturedCrate {
    name: "embassy", blurb: "嵌入式异步运行时与 HAL", url: "https://embassy.dev"
  },
  FeaturedCrate { name: "rtic", blurb: "实时中断驱动并发框架", url: "https://rtic.rs" },
  FeaturedCrate {
    name: "embedded-hal",
    blurb: "跨平台外设抽象 trait",
    url: "https://github.com/rust-embedded/embedded-hal",
  },
  FeaturedCrate {
    name: "defmt",
    blurb: "嵌入式高效结构化日志",
    url: "https://github.com/knurling-rs/defmt",
  },
  FeaturedCrate { name: "probe-rs", blurb: "烧录与调试工具链", url: "https://probe.rs" },
  FeaturedCrate {
    name: "heapless",
    blurb: "无堆分配的静态容量数据结构",
    url: "https://github.com/rust-embedded/heapless",
  },
];

/// 文章排序所需的最小契约，[`text`] 测试与 [`server`] 列表共用。
pub trait DatedArticle {
  fn date(&self) -> &str;
  fn title(&self) -> &str;
}

/// 归一化标签：trim + 小写，仅保留字母数字 / `-` / `_`。
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

/// 子主题 slug → label 查询。
pub fn subtopic_label(slug: &str) -> Option<&'static str> {
  SUBTOPICS.iter().find(|s| s.slug == slug).map(|s| s.label)
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
    assert_eq!(BOARD_ID, "embedded");
    assert!(BOARD_ROUTE.starts_with('/'));
    assert!(!BOARD_LABEL.is_empty());
    assert!(!BOARD_TAGLINE.is_empty());
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
      assert!(!s.label.is_empty());
      assert!(!s.blurb.is_empty());
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
      assert!(!c.blurb.is_empty());
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
  fn normalize_tag_lowercases_and_strips() {
    assert_eq!(normalize_tag(" No_STD!! "), "no_std");
    assert_eq!(normalize_tag("embedded-hal"), "embedded-hal");
  }

  #[test]
  fn normalize_tags_dedups_and_drops_empty() {
    let tags =
      vec!["Embassy".to_string(), "embassy".to_string(), "   ".to_string(), "RTIC".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["embassy".to_string(), "rtic".to_string()]);
  }

  #[test]
  fn subtopic_label_known_and_unknown() {
    assert_eq!(subtopic_label("embassy"), Some("Embassy"));
    assert_eq!(subtopic_label("does-not-exist"), None);
  }

  #[test]
  fn matches_query_empty_returns_true() {
    assert!(matches_query("t", "d", &[], ""));
    assert!(matches_query("t", "d", &[], "   "));
  }

  #[test]
  fn matches_query_hits_title_description_tags() {
    let tags = vec!["embassy".to_string(), "async".to_string()];
    assert!(matches_query("Embassy 入门", "异步固件", &tags, "embassy"));
    assert!(matches_query("Embassy 入门", "异步固件", &tags, "异步"));
    assert!(matches_query("Embassy 入门", "异步固件", &tags, "async"));
    assert!(!matches_query("Embassy 入门", "异步固件", &tags, "solana"));
  }

  #[test]
  fn matches_query_supports_chinese() {
    let tags = vec!["no-std".to_string()];
    assert!(matches_query("裸机 Rust", "no_std 实践", &tags, "裸机"));
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
