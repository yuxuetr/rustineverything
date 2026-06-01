use dioxus::fullstack::{post, ServerFnError};
#[allow(unused_imports)]
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
pub(crate) fn parse_doc_frontmatter(content: &str) -> (DocMeta, String) {
  if !content.starts_with("---") {
    return (DocMeta::default(), content.to_string());
  }
  let parts: Vec<&str> = content.splitn(3, "---").collect();
  if parts.len() < 3 {
    return (DocMeta::default(), content.to_string());
  }
  let meta: DocMeta = serde_yaml::from_str(parts[1]).unwrap_or_default();
  (meta, parts[2].to_string())
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
  let (meta, body) = parse_doc_frontmatter(&content);

  // 标题优先级：sidebar_label > frontmatter title > # heading > 目录名
  let title = meta
    .sidebar_label
    .clone()
    .or_else(|| if !meta.title.is_empty() { Some(meta.title.clone()) } else { None })
    .or_else(|| {
      for line in body.lines() {
        let trimmed = line.trim();
        if let Some(t) = trimmed.strip_prefix("# ") {
          return Some(t.trim().to_string());
        }
      }
      None
    });

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

#[post("/api/docs/tree")]
pub async fn list_doc_tree() -> Result<Vec<DocTreeNode>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let docs_dir = get_asset_root().join("docs");
    if !docs_dir.exists() {
      return Ok(vec![]);
    }
    Ok(scan_doc_dir(&docs_dir, "", 1))
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

#[post("/api/docs/content")]
pub async fn get_doc_content(path: String) -> Result<DocContentResponse, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let docs_dir = get_asset_root().join("docs").join(&path);
    let md = docs_dir.join("index.md");
    let mdx = docs_dir.join("index.mdx");
    let filepath = if md.exists() {
      md
    } else if mdx.exists() {
      mdx
    } else {
      return Err(ServerFnError::new(format!("文档未找到: {}", path)));
    };
    let raw =
      fs::read_to_string(&filepath).map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))?;
    let (meta, content) = parse_doc_frontmatter(&raw);
    // Phase 9.3：pre-stage content transformers chain（fail-open，空链路零开销直通）。
    let content =
      app_core::engines::content_transformer::apply_default_pre(&content, "doc").await;
    Ok(DocContentResponse { content, meta })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = path;
    Ok(DocContentResponse { content: String::new(), meta: DocMeta::default() })
  }
}

// ========== Tests ==========

#[cfg(all(test, feature = "server"))]
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

  /// 辅助：提取一棵子树的 slug 列表（保留顺序）
  fn slugs(nodes: &[DocTreeNode]) -> Vec<String> {
    nodes.iter().map(|n| n.slug.clone()).collect()
  }

  fn find<'a>(nodes: &'a [DocTreeNode], slug: &str) -> &'a DocTreeNode {
    nodes.iter().find(|n| n.slug == slug).unwrap_or_else(|| panic!("未找到节点: {}", slug))
  }

  #[test]
  fn test_frontmatter_parsing() {
    let raw = "---\ntitle: Hello\nkeywords: [a, b]\nsidebar_position: 5\n---\n\n# body";
    let (meta, body) = parse_doc_frontmatter(raw);
    assert_eq!(meta.title, "Hello");
    assert_eq!(meta.keywords, vec!["a", "b"]);
    assert_eq!(meta.sidebar_position, Some(5));
    assert!(body.trim_start().starts_with("# body"));
  }

  #[test]
  fn test_no_frontmatter_returns_default() {
    let (meta, body) = parse_doc_frontmatter("# Just heading\n\nsome body");
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
}
