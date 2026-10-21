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
  /// frontmatter 的 `updated`（规范化后）；没有则为空。sitemap / feed 用它作更新时间。
  pub updated: String,
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
    // 文章页只认 `[A-Za-z0-9_-]`（`post_file`）；不合法的目录名进列表只会生成死链（Z4）。
    if !app_core::utils::is_safe_slug(&slug) {
      tracing::warn!(dir = %path.display(), "blog: directory name is not a valid slug; skipped");
      continue;
    }

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
    let (meta, _) = widgets::parse_mdx(&content);
    if meta.draft {
      continue;
    }

    posts.push(BlogPostSummary {
      slug,
      title: meta.title,
      description: meta.description.unwrap_or_default(),
      date: meta.date.unwrap_or_default(),
      updated: meta.updated.unwrap_or_default(),
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

/// 文章源文件 `posts/<id>/index.mdx`（优先）或 `index.md`。`id` 必须是单段 slug：
/// server fn 可被直接调用，`../courses/...` 这类 id 会读到付费课时等站内其他文件。
#[allow(dead_code)]
fn post_file(asset_root: &std::path::Path, id: &str) -> Option<PathBuf> {
  if !app_core::utils::is_safe_slug(id) {
    return None;
  }
  let dir = asset_root.join("posts").join(id);
  ["index.mdx", "index.md"].into_iter().map(|name| dir.join(name)).find(|p| p.is_file())
}

/// 文章源文本；`Ok(None)` 表示没有这篇文章（页面据此回 404）。
#[server]
pub async fn get_blog_content(id: String) -> Result<Option<String>, ServerFnError> {
  let Some(filepath) = post_file(&get_asset_root(), &id) else {
    return Ok(None);
  };

  let raw =
    fs::read_to_string(&filepath).map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))?;
  // 草稿不对外（Z3）。
  if widgets::parse_mdx(&raw).0.draft {
    return Ok(None);
  }
  Ok(Some(raw))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn post_file_stays_inside_posts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets");
    assert!(post_file(&root, "welcome").is_some());
    // 站内确实存在的 index.md，但不在 posts/ 下（含付费课时）
    for id in ["../docs", "../courses/rust-basics/02-ownership/01-borrow-rules", "..", "welcome/.."]
    {
      assert_eq!(post_file(&root, id), None, "{id}");
    }
  }

  fn write_post(dir: &std::path::Path, slug: &str, frontmatter: &str) {
    let d = dir.join(slug);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("index.md"), format!("---\n{frontmatter}\n---\nbody")).unwrap();
  }

  /// Z3 / Z4：草稿与非法目录名不进列表；日期规范化后再排序（`2019-3-5` 不能排在 `2019-10-01` 之后）。
  #[cfg(feature = "server")]
  #[test]
  fn blog_list_skips_drafts_and_sorts_normalized_dates() {
    let dir = std::env::temp_dir().join(format!("blog-list-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_post(&dir, "old", "title: Old\ndate: 2019-3-5 10:20:00\ntags: rust");
    write_post(&dir, "new", "title: New\ndate: 2019-10-01");
    write_post(&dir, "wip", "title: WIP\ndate: 2030-01-01\ndraft: true");
    write_post(&dir, "中文 标题.v2", "title: Bad slug\ndate: 2030-01-01");
    let posts = build_blog_list(&dir);
    let slugs: Vec<&str> = posts.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, ["new", "old"]);
    assert_eq!(posts[1].date, "2019-03-05");
    assert_eq!(posts[1].tags, ["rust"]);
    let _ = std::fs::remove_dir_all(&dir);
  }
}
