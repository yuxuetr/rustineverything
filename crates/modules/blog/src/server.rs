use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;

/// 自动探测资产根目录
#[allow(dead_code)]
fn get_asset_root() -> PathBuf {
  let mut path = PathBuf::from("assets");
  if !path.exists() {
    path = PathBuf::from("../../assets");
  }
  path
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogPostSummary {
  pub slug: String,
  pub title: String,
  pub description: String,
  pub date: String,
  pub tags: Vec<String>,
}

/// Phase 8.5：博客列表的 mtime 缓存。fingerprint = 所有
/// `assets/posts/*/index.{md,mdx}` 的 (path, mtime) 排序。
/// 内容修改 / 新增 / 删除任意一种都会失配并触发重建。
#[cfg(feature = "server")]
static BLOG_LIST_CACHE: app_core::utils::DirListingCache<Vec<BlogPostSummary>> =
  app_core::utils::DirListingCache::new();

#[cfg(feature = "server")]
fn build_blog_list(posts_dir: &std::path::Path) -> Vec<BlogPostSummary> {
  let Ok(entries) = fs::read_dir(posts_dir) else { return Vec::new() };
  let mut posts = Vec::new();
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

    posts.push(BlogPostSummary {
      slug,
      title: meta.title,
      description: meta.description,
      date: meta.date,
      tags: meta.tags,
    });
  }
  posts.sort_by(|a, b| b.date.cmp(&a.date));
  posts
}

/// 扫描 assets/posts/ 下所有博客目录，解析 frontmatter 返回列表。
///
/// Phase 8.5：用 `BLOG_LIST_CACHE` 按 (index 文件路径, mtime) fingerprint 缓存。
/// 命中时省掉 N 次 `read_to_string` + `serde_yaml::from_str`。
#[server]
pub async fn list_blog_posts() -> Result<Vec<BlogPostSummary>, ServerFnError> {
  let posts_dir = get_asset_root().join("posts");
  if !posts_dir.exists() {
    return Ok(Vec::new());
  }
  let fp = app_core::utils::fingerprint_for_dir(&posts_dir, |p| {
    matches!(p.file_name().and_then(|n| n.to_str()), Some("index.md" | "index.mdx"))
  });
  let cached: std::sync::Arc<Vec<BlogPostSummary>> =
    BLOG_LIST_CACHE.get_or_rebuild(fp, || build_blog_list(&posts_dir));
  Ok((*cached).clone())
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

#[server]
pub async fn get_blog_content(id: String) -> Result<String, ServerFnError> {
  let posts_dir = get_asset_root().join("posts").join(&id);

  // 尝试 index.mdx 和 index.md
  let mdx = posts_dir.join("index.mdx");
  let md = posts_dir.join("index.md");
  let filepath = if mdx.exists() {
    mdx
  } else if md.exists() {
    md
  } else {
    return Err(ServerFnError::new(format!("文章未找到: {}", id)));
  };

  fs::read_to_string(&filepath).map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))
}
