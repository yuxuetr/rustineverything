//! 板块文章读取：扫描 `assets/topics/<board>/<slug>/index.md(x)`。
//! 5 个板块共用同一对 server fn，板块 id 作为参数。

use dioxus::prelude::*;
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use crate::sort_by_date_desc;
use crate::ArticleSummary;

#[allow(dead_code)]
fn topics_root() -> PathBuf {
  let mut path = PathBuf::from("assets");
  if !path.exists() {
    path = PathBuf::from("../../assets");
  }
  path.join("topics")
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, Default)]
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

/// 文章目录下的正文文件，`index.mdx` 优先。
#[allow(dead_code)]
fn index_file(dir: &Path) -> Option<PathBuf> {
  ["index.mdx", "index.md"].iter().map(|f| dir.join(f)).find(|p| p.exists())
}

/// Phase 8.5：列表 mtime cache。整个 `topics/` 一份指纹，按板块分组缓存。
#[cfg(feature = "server")]
static LIST_CACHE: app_core::utils::DirListingCache<
  std::collections::BTreeMap<String, Vec<ArticleSummary>>,
> = app_core::utils::DirListingCache::new();

#[cfg(feature = "server")]
fn build_article_list(dir: &Path) -> Vec<ArticleSummary> {
  let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
  let mut items = Vec::new();
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let slug = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let Some(file) = index_file(&path) else { continue };
    let meta = parse_frontmatter(&fs::read_to_string(&file).unwrap_or_default());
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

#[cfg(feature = "server")]
fn build_all(root: &Path) -> std::collections::BTreeMap<String, Vec<ArticleSummary>> {
  let Ok(entries) = fs::read_dir(root) else { return Default::default() };
  entries
    .flatten()
    .filter(|e| e.path().is_dir())
    .filter_map(|e| {
      let id = e.file_name().to_str()?.to_string();
      Some((id, build_article_list(&e.path())))
    })
    .collect()
}

/// 某板块的文章摘要，按日期降序。
#[server]
pub async fn list_board_articles(board: String) -> Result<Vec<ArticleSummary>, ServerFnError> {
  if !app_core::utils::is_safe_slug(&board) {
    return Err(ServerFnError::new("无效的板块标识".to_string()));
  }
  let root = topics_root();
  let fp = app_core::utils::fingerprint_for_dir(&root, |p| {
    matches!(p.file_name().and_then(|n| n.to_str()), Some("index.md" | "index.mdx"))
  });
  let all = LIST_CACHE.get_or_rebuild(fp, || build_all(&root));
  Ok(all.get(&board).cloned().unwrap_or_default())
}

/// 读取单篇文章的原始 markdown（含 frontmatter）。不存在（含非法标识）返回 `None`。
#[cfg(feature = "server")]
fn read_article(root: &Path, board: &str, slug: &str) -> std::io::Result<Option<String>> {
  if !app_core::utils::is_safe_slug(board) || !app_core::utils::is_safe_slug(slug) {
    return Ok(None);
  }
  match index_file(&root.join(board).join(slug)) {
    Some(file) => fs::read_to_string(file).map(Some),
    None => Ok(None),
  }
}

/// 单篇文章原文。`Ok(None)` = 不存在（页面回 404），`Err` = 读取出错（回 500）。
#[server]
pub async fn get_board_article(
  board: String,
  slug: String,
) -> Result<Option<String>, ServerFnError> {
  read_article(&topics_root(), &board, &slug).map_err(|e| {
    tracing::error!(board = %board, slug = %slug, error = %e, "board: read article failed");
    ServerFnError::new("读取失败".to_string())
  })
}

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;

  #[test]
  fn frontmatter_parses_fields() {
    let md = "---\ntitle: Candle 推理\ndescription: 本地大模型\ndate: 2026-05-01\nsubtopic: llm\ntags: [candle, llm]\n---\n# body";
    let fm = parse_frontmatter(md);
    assert_eq!(fm.title, "Candle 推理");
    assert_eq!(fm.subtopic, "llm");
    assert_eq!(fm.tags, vec!["candle".to_string(), "llm".to_string()]);
  }

  #[test]
  fn frontmatter_missing_is_default() {
    let fm = parse_frontmatter("no frontmatter here");
    assert!(fm.title.is_empty());
    assert!(fm.tags.is_empty());
  }

  #[test]
  fn read_article_distinguishes_missing_from_present() {
    let tmp = std::env::temp_dir().join(format!("board-read-{}", std::process::id()));
    fs::create_dir_all(tmp.join("ai/real")).unwrap();
    fs::write(tmp.join("ai/real/index.md"), "# hi").unwrap();
    assert_eq!(read_article(&tmp, "ai", "real").unwrap().as_deref(), Some("# hi"));
    assert_eq!(read_article(&tmp, "ai", "missing").unwrap(), None);
    assert_eq!(read_article(&tmp, "ai", "../ai").unwrap(), None);
    assert_eq!(read_article(&tmp, "nope", "real").unwrap(), None);
    fs::remove_dir_all(&tmp).unwrap();
  }

  /// 内容校验：frontmatter 解析失败会静默退化为空标题（曾因描述里未转义的 `"` 发生）。
  #[test]
  fn every_real_article_has_a_title() {
    let all = build_all(&topics_root());
    assert!(!all.is_empty(), "找不到 assets/topics");
    let untitled: Vec<String> = all
      .iter()
      .flat_map(|(b, items)| items.iter().map(move |a| (b, a)))
      .filter(|(_, a)| a.title.is_empty() || a.date.is_empty())
      .map(|(b, a)| format!("{b}/{}", a.slug))
      .collect();
    assert!(untitled.is_empty(), "frontmatter 解析失败或缺 title/date: {untitled:?}");
  }

  #[test]
  fn build_all_groups_articles_by_board() {
    let tmp = std::env::temp_dir().join(format!("board-test-{}", std::process::id()));
    let write = |rel: &str, body: &str| {
      let p = tmp.join(rel);
      fs::create_dir_all(p.parent().unwrap()).unwrap();
      fs::write(p, body).unwrap();
    };
    write("ai/old/index.md", "---\ntitle: Old\ndate: 2026-01-01\n---\n");
    write("ai/new/index.mdx", "---\ntitle: New\ndate: 2026-02-01\n---\n");
    write("ai/no-index/notes.md", "ignored");
    write("cli/one/index.md", "---\ntitle: One\n---\n");
    let all = build_all(&tmp);
    let ai: Vec<&str> = all["ai"].iter().map(|a| a.slug.as_str()).collect();
    assert_eq!(ai, vec!["new", "old"]);
    assert_eq!(all["cli"].len(), 1);
    fs::remove_dir_all(&tmp).unwrap();
  }
}
