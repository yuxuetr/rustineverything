use dioxus::fullstack::{post, ServerFnError};
#[allow(unused_imports)]
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;

/// 文档 frontmatter 元数据（类似 Docusaurus）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DocMeta {
  #[serde(default)]
  pub title: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub keywords: Vec<String>,
  #[serde(default)]
  pub sidebar_label: Option<String>, // 侧栏显示名称（覆盖 title）
  #[serde(default)]
  pub sidebar_position: Option<i32>, // 侧栏排序（越小越前）
  #[serde(default)]
  pub image: Option<String>, // OG 图片
  /// 子项排序方向："asc"（默认，升序）或 "desc"（降序）
  /// 在父目录的 index.md 中设置，控制该目录下子项的排序方向
  /// 适合：周报/日报等以递增编号但需要最新期优先的场景
  #[serde(default)]
  pub sort_children: Option<String>,
  /// 这篇文档对着哪个库的哪个版本写的（S3），页面上显示为「基于 tokio 1.54.0」
  #[serde(default)]
  pub source: Option<DocSource>,
}

/// frontmatter `source: { crate: tokio, version: "1.54.0" }`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocSource {
  #[serde(rename = "crate")]
  pub krate: String,
  /// 不加引号也行：serde_yaml 读进 String 时保留原文，`1.10` 不会变成 `1.1`
  pub version: String,
}

/// 文档树节点（最多三级）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocTreeNode {
  pub slug: String,
  pub title: String,
  pub path: String,
  pub has_content: bool,
  pub description: String,
  pub children: Vec<DocTreeNode>,
}

/// 文档内容响应（内容 + 元数据）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocContentResponse {
  pub content: String,
  pub meta: DocMeta,
  /// 同一个一级分类里按侧栏顺序的上一篇 / 下一篇（S2）
  pub prev: Option<DocLink>,
  pub next: Option<DocLink>,
}

/// 指向另一篇文档的链接
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocLink {
  pub path: String,
  pub title: String,
}

/// 数据结构：_meta.json 中的条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct MetaEntry {
  slug: String,
  title: String,
}

/// 解析 frontmatter（YAML between --- delimiters）
#[cfg(feature = "server")]
pub(crate) fn parse_doc_frontmatter(content: &str, file: &std::path::Path) -> (DocMeta, String) {
  let Some((yaml, body)) = widgets::split_frontmatter(content) else {
    return (DocMeta::default(), content.to_string());
  };
  let meta: DocMeta = serde_yaml::from_str(yaml).unwrap_or_else(|e| {
    tracing::warn!(file = %file.display(), error = %e, "doc frontmatter: invalid YAML, metadata ignored");
    DocMeta::default()
  });
  (meta, body.to_string())
}

/// 正文里第一个 `# ` 标题。
#[cfg(feature = "server")]
fn first_heading(body: &str) -> Option<String> {
  body.lines().find_map(|l| l.trim().strip_prefix("# ")).map(|t| t.trim().to_string())
}

/// 正文第一段纯文本（跳过标题、代码块、列表、引用、表格、HTML），去掉行内代码与加粗标记，
/// 截到 120 字。没有 frontmatter 的文档页用它当 meta description（Y8）。
#[cfg(feature = "server")]
fn first_paragraph(body: &str) -> Option<String> {
  let mut in_fence = false;
  for line in body.lines() {
    let t = line.trim();
    if t.starts_with("```") {
      in_fence = !in_fence;
      continue;
    }
    if in_fence || t.is_empty() || t.starts_with(['#', '-', '*', '>', '|', '<', '!']) {
      continue;
    }
    let text = t.replace("**", "").replace('`', "");
    return Some(text.chars().take(120).collect());
  }
  None
}

/// frontmatter 没写 title / description 时从正文补上，页面标题和摘要不再是空的（Y8）。
#[cfg(feature = "server")]
fn fill_missing_meta(meta: &mut DocMeta, body: &str) {
  if meta.title.is_empty() {
    meta.title = first_heading(body).unwrap_or_default();
  }
  if meta.description.is_empty() {
    meta.description = first_paragraph(body).unwrap_or_default();
  }
}

/// 从 index.md/index.mdx 提取元数据（标题、描述、排序等）
#[cfg(feature = "server")]
fn extract_doc_info(dir: &std::path::Path) -> (Option<String>, String, Option<i32>) {
  let (title, desc, pos, _) = extract_doc_info_full(dir);
  (title, desc, pos)
}

/// 完整提取（额外返回 sort_children）
#[cfg(feature = "server")]
fn extract_doc_info_full(
  dir: &std::path::Path,
) -> (Option<String>, String, Option<i32>, Option<String>) {
  let md = dir.join("index.md");
  let mdx = dir.join("index.mdx");
  let path = if md.exists() {
    md
  } else if mdx.exists() {
    mdx
  } else {
    return (None, String::new(), None, None);
  };
  let content = fs::read_to_string(&path).unwrap_or_default();
  let (meta, body) = parse_doc_frontmatter(&content, &path);

  // 标题优先级：sidebar_label > frontmatter title > # heading > 目录名
  let title = meta
    .sidebar_label
    .clone()
    .or_else(|| if !meta.title.is_empty() { Some(meta.title.clone()) } else { None })
    .or_else(|| first_heading(&body));

  (title, meta.description.clone(), meta.sidebar_position, meta.sort_children.clone())
}

/// 扫描目录生成文档树（递归，最多 3 级）
/// 优先级：_meta.json > 自动扫描目录（从 index.md 提取标题）
#[cfg(feature = "server")]
pub(crate) fn scan_doc_dir(
  dir: &std::path::Path,
  rel_prefix: &str,
  depth: u32,
) -> Vec<DocTreeNode> {
  if depth > 3 {
    return vec![];
  }

  // 优先读取 _meta.json
  let meta_path = dir.join("_meta.json");
  let entries: Vec<MetaEntry> = if meta_path.exists() {
    fs::read_to_string(&meta_path)
      .ok()
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default()
  } else {
    // 读取当前目录的 sort_children 设置
    let (_, _, _, sort_dir) = extract_doc_info_full(dir);
    let descending = sort_dir.as_deref().map(|s| s.eq_ignore_ascii_case("desc")).unwrap_or(false);

    // 自动扫描子目录，从 index.md 提取标题和排序
    let mut dirs: Vec<(String, String, String, Option<i32>)> = fs::read_dir(dir)
      .into_iter()
      .flatten()
      .flatten()
      .filter(|e| e.path().is_dir())
      .filter_map(|e| {
        let name = e.file_name().to_str()?.to_string();
        if name.starts_with('_') || name.starts_with('.') {
          return None;
        }
        let (title, desc, pos) = extract_doc_info(&e.path());
        let title = title.unwrap_or_else(|| name.clone());
        Some((name, title, desc, pos))
      })
      .collect();
    // 按 sidebar_position 排序，无 position 的按字母顺序排在后面
    dirs.sort_by(|a, b| {
      let ord = match (a.3, b.3) {
        (Some(pa), Some(pb)) => pa.cmp(&pb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
      };
      if descending {
        ord.reverse()
      } else {
        ord
      }
    });
    dirs.into_iter().map(|(slug, title, _, _)| MetaEntry { slug, title }).collect()
  };

  entries
    .into_iter()
    .map(|entry| {
      let child_dir = dir.join(&entry.slug);
      let rel_path = if rel_prefix.is_empty() {
        entry.slug.clone()
      } else {
        format!("{}/{}", rel_prefix, entry.slug)
      };
      let has_content = child_dir.join("index.md").exists() || child_dir.join("index.mdx").exists();
      let (_, desc, _) = extract_doc_info(&child_dir);
      let children =
        if child_dir.is_dir() { scan_doc_dir(&child_dir, &rel_path, depth + 1) } else { vec![] };
      DocTreeNode {
        slug: entry.slug,
        title: entry.title,
        path: rel_path,
        has_content,
        description: desc,
        children,
      }
    })
    .collect()
}

/// 整棵文档树（约 2000 个节点）。页面不用它：侧栏用 [`doc_nav`]，首页用 [`list_doc_volumes`]；
/// sitemap 在服务端直接调用。
#[post("/api/docs/tree")]
pub async fn list_doc_tree() -> Result<Vec<DocTreeNode>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    Ok((*cached_tree()).clone())
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

/// 侧栏数据：各卷的一级节点，只有 `volume` 这一卷带完整子树（见 [`nav_tree`]）。
#[post("/api/docs/nav")]
pub async fn doc_nav(volume: String) -> Result<Vec<DocTreeNode>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    Ok(nav_tree(&cached_tree(), &volume))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = volume;
    Ok(vec![])
  }
}

/// 文档首页的卷卡片（见 [`volume_cards`]）。
#[post("/api/docs/volumes")]
pub async fn list_doc_volumes() -> Result<Vec<DocVolume>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    Ok(volume_cards(&cached_tree()))
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

/// 文档首页一张卷卡片：卷节点（只留前 [`CARD_CHAPTERS`] 个章、不含更深层）与全卷页数。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocVolume {
  pub node: DocTreeNode,
  pub pages: usize,
}

/// 卡片列出的章数；多留一个，页面据此判断要不要显示「更多」。
pub const CARD_CHAPTERS: usize = 5;

/// 侧栏用的树：每卷只留一级节点，`volume` 那一卷保留完整子树；description 全部清空
/// （侧栏不显示）。整棵树连 description 序列化约 1.9 MB，曾让每个文档页 HTML 达 4 MB。
pub fn nav_tree(full: &[DocTreeNode], volume: &str) -> Vec<DocTreeNode> {
  fn strip(n: &DocTreeNode, deep: bool) -> DocTreeNode {
    DocTreeNode {
      slug: n.slug.clone(),
      title: n.title.clone(),
      path: n.path.clone(),
      has_content: n.has_content,
      description: String::new(),
      children: if deep { n.children.iter().map(|c| strip(c, true)).collect() } else { Vec::new() },
    }
  }
  full.iter().map(|n| strip(n, n.path == volume)).collect()
}

/// 有正文的节点数（含自身）
pub fn count_pages(node: &DocTreeNode) -> usize {
  node.has_content as usize + node.children.iter().map(count_pages).sum::<usize>()
}

/// 首页卡片：每卷保留 description 与前 `CARD_CHAPTERS + 1` 个章（章下不再展开）。
pub fn volume_cards(full: &[DocTreeNode]) -> Vec<DocVolume> {
  full
    .iter()
    .map(|n| DocVolume {
      pages: count_pages(n),
      node: DocTreeNode {
        children: n
          .children
          .iter()
          .take(CARD_CHAPTERS + 1)
          .map(|c| DocTreeNode { children: Vec::new(), description: String::new(), ..c.clone() })
          .collect(),
        ..n.clone()
      },
    })
    .collect()
}

/// 扫描结果按 (index.md / index.mdx / _meta.json, mtime) 缓存：扫一遍要读约 2000 个文件的
/// frontmatter，缓存命中时只剩一次目录遍历的 stat。
#[cfg(feature = "server")]
static TREE_CACHE: app_core::utils::DirListingCache<Vec<DocTreeNode>> =
  app_core::utils::DirListingCache::new();

#[cfg(feature = "server")]
fn cached_tree() -> std::sync::Arc<Vec<DocTreeNode>> {
  let docs_dir = app_core::utils::content_dir("docs");
  if !docs_dir.exists() {
    return std::sync::Arc::new(Vec::new());
  }
  let fp = app_core::utils::fingerprint_for_dir(&docs_dir, |p| {
    matches!(p.file_name().and_then(|n| n.to_str()), Some("index.md" | "index.mdx" | "_meta.json"))
  });
  TREE_CACHE.get_or_rebuild(fp, || scan_doc_dir(&docs_dir, "", 1))
}

/// 文档路径 `path`（相对 docs 根目录，来自公开接口的请求体）对应的
/// `index.md` / `index.mdx`。整条文件路径都过 `safe_join_under`，目录或
/// 文件本身是指向根外的符号链接也会被拒绝（SEC-03）。
#[cfg(feature = "server")]
fn resolve_doc_file(docs_root: &std::path::Path, path: &str) -> Option<PathBuf> {
  let dir = path.trim_end_matches('/');
  ["index.md", "index.mdx"].into_iter().find_map(|name| {
    let rel = if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") };
    app_core::utils::safe_join_under(docs_root, &rel).filter(|p| p.is_file())
  })
}

/// 文档页的最后修改日期（`index.md` 的 mtime，UTC `YYYY-MM-DD`），给 sitemap 的 lastmod。
/// 内容随 git / rsync 部署，只有改过的文件 mtime 会变。
#[cfg(feature = "server")]
pub fn doc_lastmod(path: &str) -> Option<String> {
  let file = resolve_doc_file(&app_core::utils::content_dir("docs"), path)?;
  let modified = std::fs::metadata(file).ok()?.modified().ok()?;
  Some(chrono::DateTime::<chrono::Utc>::from(modified).format("%Y-%m-%d").to_string())
}

/// `path` 在它所属的一级分类里按侧栏顺序（先序：分类首页、章、章下的节）的前后两篇，
/// 只取有正文的节点，不跨分类（S2）。`tree` 是整棵文档树（`scan_doc_dir(root, "", 1)`）。
#[cfg(feature = "server")]
fn doc_neighbors(tree: &[DocTreeNode], path: &str) -> (Option<DocLink>, Option<DocLink>) {
  let path = path.trim_end_matches('/');
  let Some(top) = path.split('/').next().filter(|s| !s.is_empty()) else {
    return (None, None);
  };
  let Some(root) = tree.iter().find(|n| n.path == top) else {
    return (None, None);
  };
  let mut pages = Vec::new();
  collect_pages(root, &mut pages);
  let Some(i) = pages.iter().position(|n| n.path == path) else {
    return (None, None);
  };
  let link = |n: &DocTreeNode| DocLink { path: n.path.clone(), title: n.title.clone() };
  (i.checked_sub(1).map(|j| link(pages[j])), pages.get(i + 1).map(|n| link(n)))
}

#[cfg(feature = "server")]
fn collect_pages<'a>(node: &'a DocTreeNode, out: &mut Vec<&'a DocTreeNode>) {
  if node.has_content {
    out.push(node);
  }
  for child in &node.children {
    collect_pages(child, out);
  }
}

/// 文档正文与 frontmatter；`Ok(None)` 表示没有这篇文档（页面据此回 404）。
#[post("/api/docs/content")]
pub async fn get_doc_content(path: String) -> Result<Option<DocContentResponse>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let docs_root = app_core::utils::content_dir("docs");
    let Some(filepath) = resolve_doc_file(&docs_root, &path) else {
      return Ok(None);
    };
    let raw =
      fs::read_to_string(&filepath).map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))?;
    let (mut meta, content) = parse_doc_frontmatter(&raw, &filepath);
    fill_missing_meta(&mut meta, &content);
    let (prev, next) = doc_neighbors(&cached_tree(), &path);
    Ok(Some(DocContentResponse { content, meta, prev, next }))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = path;
    Ok(None)
  }
}

// ========== Tests ==========

// clippy 的 allow-unwrap-in-tests 只认 `#[cfg(test)]`，组合 cfg 要手动放开
#[cfg(all(test, feature = "server"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
  use super::*;
  use std::path::Path;
  use tempfile::TempDir;

  /// 辅助：在指定目录下创建 index.md（可选带 frontmatter）
  fn write_index(dir: &Path, frontmatter: Option<&str>, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let content = match frontmatter {
      Some(fm) => format!("---\n{}\n---\n\n{}", fm, body),
      None => body.to_string(),
    };
    std::fs::write(dir.join("index.md"), content).unwrap();
  }

  /// Y8：缺 frontmatter 时 title 取首个 `#` 标题，description 取首段纯文本；已写的不覆盖。
  #[test]
  fn missing_meta_filled_from_body() {
    let body = "# Handler 函数\n\n```rust\nfn a() {}\n```\n\n- 列表\n\nHandler 是处理 **HTTP** 请求的 `async` 函数：\n";
    let mut meta = DocMeta::default();
    fill_missing_meta(&mut meta, body);
    assert_eq!(meta.title, "Handler 函数");
    assert_eq!(meta.description, "Handler 是处理 HTTP 请求的 async 函数：");
    let mut meta = DocMeta { title: "T".into(), description: "D".into(), ..Default::default() };
    fill_missing_meta(&mut meta, body);
    assert_eq!((meta.title.as_str(), meta.description.as_str()), ("T", "D"));
  }

  /// 辅助：提取一棵子树的 slug 列表（保留顺序）
  fn slugs(nodes: &[DocTreeNode]) -> Vec<String> {
    nodes.iter().map(|n| n.slug.clone()).collect()
  }

  fn find<'a>(nodes: &'a [DocTreeNode], slug: &str) -> &'a DocTreeNode {
    nodes.iter().find(|n| n.slug == slug).unwrap_or_else(|| panic!("未找到节点: {}", slug))
  }

  #[test]
  fn nav_tree_keeps_only_the_active_volume_deep() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();
    write_index(&docs.join("tokio"), Some("title: tokio\ndescription: d"), "");
    write_index(&docs.join("tokio/01-a"), Some("title: A\ndescription: da"), "");
    write_index(&docs.join("tokio/01-a/x"), Some("title: X"), "");
    write_index(&docs.join("axum"), Some("title: axum"), "");
    write_index(&docs.join("axum/01-b"), Some("title: B"), "");
    let full = scan_doc_dir(docs, "", 1);
    let nav = nav_tree(&full, "tokio");
    let tokio = nav.iter().find(|n| n.path == "tokio").unwrap();
    let axum = nav.iter().find(|n| n.path == "axum").unwrap();
    assert_eq!(tokio.children[0].children[0].path, "tokio/01-a/x");
    assert!(axum.children.is_empty(), "其它卷只留一级节点");
    assert!(tokio.description.is_empty() && tokio.children[0].description.is_empty());
    assert!(nav_tree(&full, "").iter().all(|n| n.children.is_empty()));

    let cards = volume_cards(&full);
    let t = cards.iter().find(|c| c.node.path == "tokio").unwrap();
    assert_eq!(t.pages, 3);
    assert_eq!(t.node.description, "d");
    assert!(t.node.children[0].children.is_empty());
  }

  #[test]
  fn neighbors_follow_sidebar_order_within_one_category() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();
    write_index(&docs.join("tokio"), Some("title: tokio"), "");
    write_index(&docs.join("tokio/02-b/y"), Some("title: Y\nsidebar_position: 1"), "");
    write_index(&docs.join("tokio/01-a"), Some("title: A\nsidebar_position: 1"), "");
    write_index(&docs.join("tokio/01-a/x"), Some("title: X"), "");
    write_index(&docs.join("axum"), Some("title: axum"), "");
    // 02-b 没有 index.md：不是一页，跳过
    let tree = scan_doc_dir(docs, "", 1);
    let order = |p: &str| {
      let (prev, next) = doc_neighbors(&tree, p);
      (prev.map(|l| l.path), next.map(|l| l.path))
    };
    assert_eq!(order("tokio"), (None, Some("tokio/01-a".into())));
    assert_eq!(order("tokio/01-a"), (Some("tokio".into()), Some("tokio/01-a/x".into())));
    assert_eq!(order("tokio/01-a/x"), (Some("tokio/01-a".into()), Some("tokio/02-b/y".into())));
    assert_eq!(order("tokio/02-b/y/"), (Some("tokio/01-a/x".into()), None), "不跨到 axum");
    assert_eq!(order("axum"), (None, None));
    assert_eq!(order(""), (None, None));
    assert_eq!(order("tokio/nope"), (None, None));
    let (_, next) = doc_neighbors(&tree, "tokio");
    assert_eq!(next.map(|l| l.title), Some("A".to_string()));
  }

  #[test]
  fn test_frontmatter_parsing() {
    let raw = "---\ntitle: Hello\nkeywords: [a, b]\nsidebar_position: 5\n---\n\n# body";
    let (meta, body) = parse_doc_frontmatter(raw, Path::new("t.md"));
    assert_eq!(meta.title, "Hello");
    assert_eq!(meta.keywords, vec!["a", "b"]);
    assert_eq!(meta.sidebar_position, Some(5));
    assert!(body.trim_start().starts_with("# body"));
  }

  #[test]
  fn source_frontmatter_parsed() {
    let (meta, _) = parse_doc_frontmatter(
      "---\ntitle: T\nsource: { crate: tokio, version: 1.54.0 }\n---\nbody",
      Path::new("t.md"),
    );
    assert_eq!(meta.source, Some(DocSource { krate: "tokio".into(), version: "1.54.0".into() }));
    assert_eq!(meta.title, "T");
    let (meta, _) = parse_doc_frontmatter(
      "---\ntitle: T\nsource: { crate: axum, version: 1.10 }\n---\n",
      Path::new("t.md"),
    );
    assert_eq!(meta.source.map(|s| s.version), Some("1.10".to_string()));
  }

  #[test]
  fn test_no_frontmatter_returns_default() {
    let (meta, body) = parse_doc_frontmatter("# Just heading\n\nsome body", Path::new("t.md"));
    assert_eq!(meta, DocMeta::default());
    assert!(body.contains("# Just heading"));
  }

  #[test]
  fn test_default_ascending_by_position() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    write_index(&docs.join("a"), Some("title: A\nsidebar_position: 3"), "# A");
    write_index(&docs.join("b"), Some("title: B\nsidebar_position: 1"), "# B");
    write_index(&docs.join("c"), Some("title: C\nsidebar_position: 2"), "# C");

    let tree = scan_doc_dir(docs, "", 1);
    assert_eq!(slugs(&tree), vec!["b", "c", "a"]);
  }

  #[test]
  fn test_descending_via_sort_children() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 父目录创建 index.md，并设置 sort_children: desc
    write_index(docs, Some("sort_children: desc"), "# root");

    write_index(&docs.join("issue-001"), Some("sidebar_position: 1"), "# 1");
    write_index(&docs.join("issue-002"), Some("sidebar_position: 2"), "# 2");
    write_index(&docs.join("issue-003"), Some("sidebar_position: 3"), "# 3");
    write_index(&docs.join("issue-005"), Some("sidebar_position: 5"), "# 5");
    write_index(&docs.join("issue-004"), Some("sidebar_position: 4"), "# 4");

    let tree = scan_doc_dir(docs, "", 1);
    assert_eq!(slugs(&tree), vec!["issue-005", "issue-004", "issue-003", "issue-002", "issue-001"]);
  }

  #[test]
  fn test_sort_children_case_insensitive() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 大写 DESC 也应被识别
    write_index(docs, Some("sort_children: DESC"), "# root");
    write_index(&docs.join("v1"), Some("sidebar_position: 1"), "# 1");
    write_index(&docs.join("v2"), Some("sidebar_position: 2"), "# 2");

    let tree = scan_doc_dir(docs, "", 1);
    assert_eq!(slugs(&tree), vec!["v2", "v1"]);
  }

  #[test]
  fn test_no_position_falls_back_to_alphabetical() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 都没有 sidebar_position
    write_index(&docs.join("zebra"), None, "# Zebra");
    write_index(&docs.join("apple"), None, "# Apple");
    write_index(&docs.join("mango"), None, "# Mango");

    let tree = scan_doc_dir(docs, "", 1);
    assert_eq!(slugs(&tree), vec!["apple", "mango", "zebra"]);
  }

  #[test]
  fn test_mixed_position_and_no_position() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    write_index(&docs.join("first"), Some("sidebar_position: 1"), "# 1");
    write_index(&docs.join("middle"), Some("sidebar_position: 5"), "# 5");
    write_index(&docs.join("zzz"), None, "# zzz");
    write_index(&docs.join("aaa"), None, "# aaa");

    let tree = scan_doc_dir(docs, "", 1);
    // 有 position 的在前面（按 position 排），无 position 的在后面（按字母排）
    assert_eq!(slugs(&tree), vec!["first", "middle", "aaa", "zzz"]);
  }

  #[test]
  fn test_three_level_nesting_with_independent_sort() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 一级：axum（默认 asc）与 weekly（desc）
    write_index(&docs.join("axum"), Some("sidebar_position: 1"), "# Axum");
    write_index(&docs.join("weekly"), Some("sidebar_position: 2\nsort_children: desc"), "# Weekly");

    // 二级：axum/basic（默认）与 axum/advance
    write_index(&docs.join("axum/basic"), Some("sidebar_position: 1"), "# basic");
    write_index(&docs.join("axum/advance"), Some("sidebar_position: 2"), "# advance");

    // 三级：axum/basic/router 与 handler
    write_index(&docs.join("axum/basic/router"), Some("sidebar_position: 1"), "# router");
    write_index(&docs.join("axum/basic/handler"), Some("sidebar_position: 2"), "# handler");

    // weekly 子项：递增编号
    write_index(&docs.join("weekly/issue-001"), Some("sidebar_position: 1"), "# 1");
    write_index(&docs.join("weekly/issue-002"), Some("sidebar_position: 2"), "# 2");
    write_index(&docs.join("weekly/issue-003"), Some("sidebar_position: 3"), "# 3");

    let tree = scan_doc_dir(docs, "", 1);

    // 顶层：axum 排在 weekly 前（默认 asc，不受子项的 sort_children 影响）
    assert_eq!(slugs(&tree), vec!["axum", "weekly"]);

    // axum 下面 —— 默认升序
    let axum = find(&tree, "axum");
    assert_eq!(slugs(&axum.children), vec!["basic", "advance"]);

    // axum/basic 下面 —— 默认升序
    let basic = find(&axum.children, "basic");
    assert_eq!(slugs(&basic.children), vec!["router", "handler"]);

    // weekly 下面 —— desc 降序
    let weekly = find(&tree, "weekly");
    assert_eq!(slugs(&weekly.children), vec!["issue-003", "issue-002", "issue-001"]);
  }

  #[test]
  fn test_sort_children_only_affects_direct_children() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 父目录 sort_children: desc
    write_index(docs, Some("sort_children: desc"), "# root");

    // 一级：子项 a/b 会被逆序
    write_index(&docs.join("a"), Some("sidebar_position: 1"), "# a");
    write_index(&docs.join("b"), Some("sidebar_position: 2"), "# b");

    // 二级：a 下面的 a-1/a-2 仍然应该是升序（父的 sort_children 不会传递）
    write_index(&docs.join("a/a-1"), Some("sidebar_position: 1"), "# a1");
    write_index(&docs.join("a/a-2"), Some("sidebar_position: 2"), "# a2");

    let tree = scan_doc_dir(docs, "", 1);
    // 一级逆序
    assert_eq!(slugs(&tree), vec!["b", "a"]);
    // 二级仍然升序
    let a = find(&tree, "a");
    assert_eq!(slugs(&a.children), vec!["a-1", "a-2"]);
  }

  #[test]
  fn test_path_propagation_in_nested_tree() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    write_index(&docs.join("axum"), Some("sidebar_position: 1"), "# Axum");
    write_index(&docs.join("axum/basic"), Some("sidebar_position: 1"), "# basic");
    write_index(&docs.join("axum/basic/router"), Some("sidebar_position: 1"), "# router");

    let tree = scan_doc_dir(docs, "", 1);
    let axum = find(&tree, "axum");
    assert_eq!(axum.path, "axum");
    let basic = find(&axum.children, "basic");
    assert_eq!(basic.path, "axum/basic");
    let router = find(&basic.children, "router");
    assert_eq!(router.path, "axum/basic/router");
  }

  #[test]
  fn test_max_depth_three_levels() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 创建 4 级嵌套，验证只扫描前 3 级
    write_index(&docs.join("l1/l2/l3/l4"), None, "# deep");
    write_index(&docs.join("l1/l2/l3"), None, "# l3");
    write_index(&docs.join("l1/l2"), None, "# l2");
    write_index(&docs.join("l1"), None, "# l1");

    let tree = scan_doc_dir(docs, "", 1);
    let l1 = find(&tree, "l1");
    let l2 = find(&l1.children, "l2");
    let l3 = find(&l2.children, "l3");
    // l3 是第 3 级，其 children 应为空（第 4 级被截断）
    assert!(l3.children.is_empty(), "超过 3 级的节点应不被扫描");
  }

  #[test]
  fn test_sidebar_label_overrides_title() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    write_index(
      &docs.join("x"),
      Some("title: Long Title For SEO\nsidebar_label: Short"),
      "# heading",
    );

    let tree = scan_doc_dir(docs, "", 1);
    let x = find(&tree, "x");
    // 侧栏显示使用 sidebar_label
    assert_eq!(x.title, "Short");
  }

  #[test]
  fn test_title_falls_back_to_h1_then_slug() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    // 无 frontmatter，只有 # 标题
    write_index(&docs.join("with-h1"), None, "# From Heading");
    // 有 frontmatter 但不含标题相关字段，也没 h1
    write_index(&docs.join("no-title"), Some("description: just desc"), "some body");

    let tree = scan_doc_dir(docs, "", 1);
    assert_eq!(find(&tree, "with-h1").title, "From Heading");
    // 都拿不到时退化到目录名
    assert_eq!(find(&tree, "no-title").title, "no-title");
  }

  #[test]
  fn test_underscore_and_hidden_dirs_skipped() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path();

    write_index(&docs.join("visible"), None, "# visible");
    write_index(&docs.join("_private"), None, "# private");
    write_index(&docs.join(".hidden"), None, "# hidden");

    let tree = scan_doc_dir(docs, "", 1);
    assert_eq!(slugs(&tree), vec!["visible"]);
  }

  #[test]
  fn test_empty_dir_returns_empty_tree() {
    let tmp = TempDir::new().unwrap();
    let tree = scan_doc_dir(tmp.path(), "", 1);
    assert!(tree.is_empty());
  }

  #[test]
  fn resolve_doc_file_stays_inside_docs_root() {
    // SEC-03：公开接口，path 来自请求体
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path().join("docs");
    write_index(&docs.join("guide/intro"), None, "intro");
    write_index(&docs.join("mdx-only"), None, "x");
    std::fs::rename(docs.join("mdx-only/index.md"), docs.join("mdx-only/index.mdx")).unwrap();
    write_index(&tmp.path().join("courses/paid/lesson"), None, "secret");
    write_index(&tmp.path().join("outside"), None, "secret");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tmp.path().join("outside"), docs.join("link")).unwrap();

    assert!(resolve_doc_file(&docs, "guide/intro").is_some());
    assert!(resolve_doc_file(&docs, "mdx-only").is_some_and(|p| p.ends_with("index.mdx")));
    let outside = tmp.path().join("outside");
    for bad in [
      "../courses/paid/lesson",
      "guide/../../courses/paid/lesson",
      outside.to_str().unwrap(),
      "..\\courses\\paid\\lesson",
      "link",
      "%2e%2e/courses/paid/lesson",
      "guide/missing",
    ] {
      assert_eq!(resolve_doc_file(&docs, bad), None, "{bad}");
    }
  }
}
