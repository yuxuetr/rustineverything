//! 5 个内容板块（embedded / ai / web3 / wasm / cli）的共享实现（阶段 O1）。
//!
//! 板块 crate 仍各自独立（Phase 6 决策），但只持有自己的 [`Board`] 常量；
//! 文章扫描（[`server`]）与落地页 / 详情页（[`ui`]）在这里只写一份。
//! 某个板块需要分歧时，在它自己的 crate 里写页面，不给这里加开关。
//!
//! 展示文案（label / tagline / 子主题 / crate 简介）由 `app_core::i18n` 从
//! `assets/i18n/{zh,en}.ftl` 按 `{board.id}.*` key 提供。

pub mod server;
pub mod ui;

use serde::{Deserialize, Serialize};

pub use ui::{BoardArticle, BoardIndex};

/// 一个内容板块的结构性数据。文章来自 `assets/topics/{id}/<slug>/index.md(x)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board {
  /// 模块 id，也是 `assets/topics/` 下的目录名与 i18n key 前缀。
  pub id: &'static str,
  /// 落地页路由，文章页为 `{route}/{slug}`。
  pub route: &'static str,
  /// 子主题 slug；展示文案经 `{id}.sub.{slug}.label / .blurb` 查表。
  pub subtopics: &'static [&'static str],
  pub featured_crates: &'static [FeaturedCrate],
}

/// 一个精选 crate。`name` 为品牌名（语言中性）；简介经
/// `{board.id}.crate.{normalize_tag(name)}.blurb` 查表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeaturedCrate {
  pub name: &'static str,
  pub url: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArticleSummary {
  pub slug: String,
  pub title: String,
  pub description: String,
  pub date: String,
  pub subtopic: String,
  pub tags: Vec<String>,
}

impl Board {
  /// 配置自检，返回所有问题（空 = 合法）。各板块 crate 的单测调用它。
  pub fn problems(&self) -> Vec<String> {
    let mut out = Vec::new();
    if !app_core::utils::is_safe_slug(self.id) {
      out.push(format!("id `{}` 不是合法 slug", self.id));
    }
    if self.route != format!("/{}", self.id) {
      out.push(format!("route `{}` 应为 `/{}`", self.route, self.id));
    }
    for s in self.subtopics {
      if normalize_tag(s) != *s {
        out.push(format!("子主题 `{s}` 不是归一化形态"));
      }
    }
    if has_duplicates(self.subtopics.iter().map(|s| s.to_string())) {
      out.push("子主题 slug 重复".to_string());
    }
    for c in self.featured_crates {
      if !c.url.starts_with("https://") {
        out.push(format!("crate `{}` 的 url 不是 https", c.name));
      }
    }
    if has_duplicates(self.featured_crates.iter().map(|c| normalize_tag(c.name))) {
      out.push("精选 crate 的 i18n key（normalize_tag(name)）重复".to_string());
    }
    out
  }
}

fn has_duplicates(items: impl Iterator<Item = String>) -> bool {
  let mut v: Vec<String> = items.collect();
  let n = v.len();
  v.sort();
  v.dedup();
  v.len() != n
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
pub fn sort_by_date_desc(items: &mut [ArticleSummary]) {
  items.sort_by(|a, b| {
    b.date.cmp(&a.date).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  const GOOD: Board = Board {
    id: "ai",
    route: "/ai",
    subtopics: &["llm", "inference"],
    featured_crates: &[FeaturedCrate {
      name: "candle",
      url: "https://github.com/huggingface/candle",
    }],
  };

  fn summary(date: &str, title: &str) -> ArticleSummary {
    ArticleSummary {
      slug: title.to_string(),
      title: title.to_string(),
      description: String::new(),
      date: date.to_string(),
      subtopic: String::new(),
      tags: Vec::new(),
    }
  }

  #[test]
  fn valid_board_has_no_problems() {
    assert_eq!(GOOD.problems(), Vec::<String>::new());
  }

  #[test]
  fn problems_catch_each_misconfiguration() {
    let bad = Board {
      id: "a/i",
      route: "/x",
      subtopics: &["LLM", "dup", "dup"],
      featured_crates: &[
        FeaturedCrate { name: "Candle", url: "http://x" },
        FeaturedCrate { name: "candle!", url: "https://y" },
      ],
    };
    let p = bad.problems();
    assert_eq!(p.len(), 6, "{p:?}");
  }

  #[test]
  fn normalize_tag_lowercases_and_strips() {
    assert_eq!(normalize_tag(" Candle!! "), "candle");
    assert_eq!(normalize_tag("safe-tensors"), "safe-tensors");
    assert_eq!(normalize_tag("ink!"), "ink");
  }

  #[test]
  fn matches_query_empty_returns_true() {
    assert!(matches_query("t", "d", &[], ""));
    assert!(matches_query("t", "d", &[], "   "));
  }

  #[test]
  fn matches_query_hits_title_description_tags() {
    let tags = vec!["candle".to_string(), "llm".to_string()];
    assert!(matches_query("Candle 推理", "本地大模型", &tags, "candle"));
    assert!(matches_query("Candle 推理", "本地大模型", &tags, "大模型"));
    assert!(matches_query("Candle 推理", "本地大模型", &tags, "llm"));
    assert!(!matches_query("Candle 推理", "本地大模型", &tags, "solana"));
  }

  #[test]
  fn sort_by_date_desc_orders_newest_first_then_title() {
    let mut items = vec![
      summary("2026-01-01", "beta"),
      summary("2026-03-01", "zeta"),
      summary("2026-01-01", "alpha"),
    ];
    sort_by_date_desc(&mut items);
    let order: Vec<&str> = items.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(order, vec!["zeta", "alpha", "beta"]);
  }
}
