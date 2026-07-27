use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;

#[allow(unused_imports)]
use crate::text::{sort_by_date_desc, DatedArticle, BOARD_ID};

/// 自动探测资产根目录。
#[allow(dead_code)]
fn get_asset_root() -> PathBuf {
  let mut path = PathBuf::from("assets");
  if !path.exists() {
    path = PathBuf::from("../../assets");
  }
  path
}

/// 一篇板块文章的摘要（落地页列表用）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArticleSummary {
  pub slug: String,
  pub title: String,
  pub description: String,
  pub date: String,
  pub subtopic: String,
  pub tags: Vec<String>,
}

impl DatedArticle for ArticleSummary {
  fn date(&self) -> &str {
    &self.date
  }
  fn title(&self) -> &str {
    &self.title
  }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct FrontMatter {
  #[serde(default)]
  title: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  date: String,
  #[serde(default)]
  subtopic: String,
  #[serde(default)]
  tags: Vec<String>,
}

#[allow(dead_code)]
fn parse_frontmatter(content: &str) -> FrontMatter {
  if !content.starts_with("---") {
    return FrontMatter::default();
  }
  let parts: Vec<&str> = content.splitn(3, "---").collect();
  if parts.len() < 3 {
    return FrontMatter::default();
  }
  serde_yaml::from_str(parts[1]).unwrap_or_default()
}

/// Phase 8.5：板块列表 mtime cache。
#[cfg(feature = "server")]
static LIST_CACHE: app_core::utils::DirListingCache<Vec<ArticleSummary>> =
  app_core::utils::DirListingCache::new();

#[cfg(feature = "server")]
fn build_article_list(dir: &std::path::Path) -> Vec<ArticleSummary> {
  let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
  let mut items = Vec::new();
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let slug = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let mdx = path.join("index.mdx");
    let md = path.join("index.md");
    let index_file = if mdx.exists() {
      mdx
    } else if md.exists() {
      md
    } else {
      continue;
    };
    let content = fs::read_to_string(&index_file).unwrap_or_default();
    let meta = parse_frontmatter(&content);
    items.push(ArticleSummary {
      slug,
      title: meta.title,
      description: meta.description,
      date: meta.date,
      subtopic: meta.subtopic,
      tags: meta.tags,
    });
  }
  sort_by_date_desc(&mut items);
  items
}

/// 扫描 `assets/topics/embedded/*/index.md`，按日期降序返回文章摘要。
#[server]
pub async fn list_embedded_articles() -> Result<Vec<ArticleSummary>, ServerFnError> {
  let dir = get_asset_root().join("topics").join(BOARD_ID);
  if !dir.exists() {
    return Ok(Vec::new());
  }
  let fp = app_core::utils::fingerprint_for_dir(&dir, |p| {
    matches!(p.file_name().and_then(|n| n.to_str()), Some("index.md" | "index.mdx"))
  });
  let cached = LIST_CACHE.get_or_rebuild(fp, || build_article_list(&dir));
  Ok((*cached).clone())
}

/// 读取单篇文章的原始 markdown（含 frontmatter）。
#[server]
pub async fn get_embedded_article(slug: String) -> Result<String, ServerFnError> {
  // 防御路径穿越：slug 只允许安全字符。
  if slug.is_empty() || slug.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
    return Err(ServerFnError::new("无效的文章标识".to_string()));
  }
  let dir = get_asset_root().join("topics").join(BOARD_ID).join(&slug);
  let mdx = dir.join("index.mdx");
  let md = dir.join("index.md");
  let filepath = if mdx.exists() {
    mdx
  } else if md.exists() {
    md
  } else {
    return Err(ServerFnError::new(format!("文章未找到: {}", slug)));
  };
  let raw =
    fs::read_to_string(&filepath).map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))?;
  // Phase 9.3：pre-stage content transformers chain（fail-open，空链路零开销直通）。
  Ok(app_core::engines::content_transformer::apply_default_pre(&raw, BOARD_ID).await)
}

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;

  #[test]
  fn frontmatter_parses_fields() {
    let md = "---\ntitle: Embassy 入门\ndescription: 异步固件\ndate: 2026-05-01\nsubtopic: embassy\ntags: [embassy, async]\n---\n# body";
    let fm = parse_frontmatter(md);
    assert_eq!(fm.title, "Embassy 入门");
    assert_eq!(fm.subtopic, "embassy");
    assert_eq!(fm.tags, vec!["embassy".to_string(), "async".to_string()]);
  }

  #[test]
  fn frontmatter_missing_is_default() {
    let fm = parse_frontmatter("no frontmatter here");
    assert!(fm.title.is_empty());
    assert!(fm.tags.is_empty());
  }
}
