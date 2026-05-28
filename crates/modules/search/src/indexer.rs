//! 索引源:从 `assets/posts/`、`assets/docs/` 与 PostgreSQL 中
//! 收集所有可被搜索的文档。
//!
//! 输出格式 [`IndexedDocument`] 与引擎层的 schema 一一对应。

use std::path::{Path, PathBuf};

use crate::text::{markdown_to_plain, truncate_chars};

/// 索引文档(无 tantivy 类型,便于纯逻辑测试)。
#[derive(Debug, Clone)]
pub struct IndexedDocument {
  pub kind: String,
  pub ref_id: String,
  pub title: String,
  pub body: String,
  pub url: String,
  pub created_at: String,
}

fn get_asset_root() -> PathBuf {
  let p = PathBuf::from("assets");
  if p.exists() {
    p
  } else {
    PathBuf::from("../../assets")
  }
}

/// 解析 frontmatter 中常见的标量字段(title / description / date),
/// 不引入 serde_yaml 依赖以保持兼容,简单按行扫描。
fn parse_frontmatter_kv(content: &str) -> std::collections::HashMap<String, String> {
  let mut map = std::collections::HashMap::new();
  if !content.starts_with("---") {
    return map;
  }
  let after = &content[3..];
  let end = match after.find("\n---") {
    Some(e) => e,
    None => return map,
  };
  let fm = &after[..end];
  for line in fm.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    if let Some((k, v)) = line.split_once(':') {
      let key = k.trim().to_string();
      let value = v.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
      map.insert(key, value);
    }
  }
  map
}

fn read_md_file(path: &Path) -> Option<(String, String, String)> {
  let raw = std::fs::read_to_string(path).ok()?;
  let fm = parse_frontmatter_kv(&raw);
  let title = fm.get("title").cloned().filter(|t| !t.is_empty()).unwrap_or_else(|| {
    // 退化:首个 # 标题
    for line in raw.lines() {
      if let Some(rest) = line.trim_start().strip_prefix("# ") {
        return rest.trim().to_string();
      }
    }
    // 再退化:文件名
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string()
  });
  let date = fm.get("date").cloned().unwrap_or_default();
  let body = markdown_to_plain(&raw);
  Some((title, body, date))
}

fn collect_blogs() -> Vec<IndexedDocument> {
  let mut out = Vec::new();
  let posts_dir = get_asset_root().join("posts");
  let entries = match std::fs::read_dir(&posts_dir) {
    Ok(e) => e,
    Err(_) => return out,
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let slug = match path.file_name().and_then(|s| s.to_str()) {
      Some(s) => s.to_string(),
      None => continue,
    };
    let mdx = path.join("index.mdx");
    let md = path.join("index.md");
    let chosen = if mdx.exists() {
      mdx
    } else if md.exists() {
      md
    } else {
      continue;
    };
    if let Some((title, body, date)) = read_md_file(&chosen) {
      out.push(IndexedDocument {
        kind: "blog".to_string(),
        ref_id: slug.clone(),
        title,
        body,
        url: format!("/blog/{}", slug),
        created_at: date,
      });
    }
  }
  out
}

fn collect_docs() -> Vec<IndexedDocument> {
  let mut out = Vec::new();
  let root = get_asset_root().join("docs");
  if !root.exists() {
    return out;
  }
  walk_docs(&root, &root, &mut out);
  out
}

fn walk_docs(base: &Path, dir: &Path, out: &mut Vec<IndexedDocument>) {
  let entries = match std::fs::read_dir(dir) {
    Ok(e) => e,
    Err(_) => return,
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    if name.starts_with('_') || name.starts_with('.') {
      continue;
    }
    // 当前目录的 index.{md,mdx}
    let mdx = path.join("index.mdx");
    let md = path.join("index.md");
    let chosen = if mdx.exists() {
      Some(mdx)
    } else if md.exists() {
      Some(md)
    } else {
      None
    };
    if let Some(file) = chosen {
      if let Some((title, body, date)) = read_md_file(&file) {
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        out.push(IndexedDocument {
          kind: "doc".to_string(),
          ref_id: rel_str.clone(),
          title,
          body,
          url: format!("/docs/{}", rel_str),
          created_at: date,
        });
      }
    }
    // 递归
    walk_docs(base, &path, out);
  }
}

#[cfg(feature = "server")]
async fn collect_topics() -> Result<Vec<IndexedDocument>, String> {
  use rustineverything_core::entities::topic;
  use sea_orm::EntityTrait;

  let db = match rustineverything_core::db::get_or_init_pool().await {
    Ok(db) => db,
    Err(e) => {
      // 数据库不可用时不应阻塞索引构建
      tracing::warn!(error = %e, "search: DB unavailable, skipping topics");
      return Ok(vec![]);
    }
  };
  let rows: Vec<topic::Model> = topic::Entity::find().all(&db).await.map_err(|e| e.to_string())?;
  let mut out = Vec::with_capacity(rows.len());
  for t in rows {
    let body = format!("{} {}", truncate_chars(&t.content, 4000), t.tag);
    out.push(IndexedDocument {
      kind: "topic".to_string(),
      ref_id: t.id.to_string(),
      title: t.title,
      body,
      url: format!("/topics/{}", t.id),
      created_at: t.created_at.format("%Y-%m-%d").to_string(),
    });
  }
  Ok(out)
}

#[cfg(feature = "server")]
fn collect_cases() -> Vec<IndexedDocument> {
  use rustineverything_module_cases::server::scan_cases;

  scan_cases()
    .into_iter()
    .map(|case| {
      let readme = case.readme_md.unwrap_or_default();
      let body =
        format!("{} {} {}", case.description, case.tags.join(" "), truncate_chars(&readme, 4000));
      IndexedDocument {
        kind: "case".to_string(),
        ref_id: case.slug.clone(),
        title: case.name,
        body,
        url: format!("/case/{}", case.slug),
        created_at: case.date_added,
      }
    })
    .collect()
}

/// 汇总所有可索引文档。
///
/// Phase 3.4：读取 `site.json::modules.<id>.enabled`，过滤掉关闭的模块。
/// kind → module id 映射：blog→blog / doc→docs / topic→forum / case→cases。
pub async fn collect_documents() -> Result<Vec<IndexedDocument>, String> {
  let mut all = Vec::new();
  all.extend(collect_blogs());
  all.extend(collect_docs());
  #[cfg(feature = "server")]
  {
    match collect_topics().await {
      Ok(mut t) => all.append(&mut t),
      Err(e) => tracing::warn!(error = %e, "search: failed to collect topics"),
    }
    all.extend(collect_cases());
  }

  #[cfg(feature = "server")]
  {
    let engine = rustineverything_core::engines::module::default_module_engine();
    let enabled = engine.enabled_ids();
    let is_on = |module_id: &str| enabled.iter().any(|s| s == module_id);
    all.retain(|d| match d.kind.as_str() {
      "blog" => is_on("blog"),
      "doc" => is_on("docs"),
      "topic" => is_on("forum"),
      "case" => is_on("cases"),
      // 未知 kind 默认保留：搜索引擎不应该错误地丢弃数据。
      _ => true,
    });
  }

  Ok(all)
}

/// Phase 3.4：按 module id 过滤已汇总的索引文档，便于上层显式调用。
///
/// `enabled_module_ids` 来自 [`rustineverything_core::engines::module::ModuleEngine::enabled_ids`]。
/// kind → module id 同 [`collect_documents`]。该函数纯逻辑，便于单测覆盖。
pub fn filter_documents_by_enabled(
  docs: Vec<IndexedDocument>,
  enabled_module_ids: &[String],
) -> Vec<IndexedDocument> {
  let is_on = |id: &str| enabled_module_ids.iter().any(|s| s == id);
  docs
    .into_iter()
    .filter(|d| match d.kind.as_str() {
      "blog" => is_on("blog"),
      "doc" => is_on("docs"),
      "topic" => is_on("forum"),
      "case" => is_on("cases"),
      _ => true,
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
      let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, content);
  }

  #[test]
  fn parse_frontmatter_kv_basic() {
    let raw = "---\ntitle: Hello\ndate: 2025-01-01\ndescription: \"a desc\"\n---\nbody";
    let fm = parse_frontmatter_kv(raw);
    assert_eq!(fm.get("title").map(|s| s.as_str()), Some("Hello"));
    assert_eq!(fm.get("date").map(|s| s.as_str()), Some("2025-01-01"));
    assert_eq!(fm.get("description").map(|s| s.as_str()), Some("a desc"));
  }

  #[test]
  fn parse_frontmatter_kv_no_fm() {
    let raw = "no fm here";
    assert!(parse_frontmatter_kv(raw).is_empty());
  }

  #[test]
  fn read_md_uses_h1_when_no_fm_title() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.md");
    write_file(&path, "# H1 Title\n\nbody content\n");
    let (title, body, date) = read_md_file(&path).expect("read");
    assert_eq!(title, "H1 Title");
    assert!(body.contains("body content"));
    assert!(date.is_empty());
  }

  #[test]
  fn read_md_uses_fm_title() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.md");
    write_file(&path, "---\ntitle: From FM\n---\n\n# Other\n");
    let (title, _, _) = read_md_file(&path).expect("read");
    assert_eq!(title, "From FM");
  }

  #[test]
  fn walk_docs_skips_underscore_and_hidden() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path();
    write_file(&base.join("real/index.md"), "# real\nx");
    write_file(&base.join("_skip/index.md"), "# skip\nx");
    write_file(&base.join(".hidden/index.md"), "# h\nx");
    let mut out = Vec::new();
    walk_docs(base, base, &mut out);
    let ids: Vec<&str> = out.iter().map(|d| d.ref_id.as_str()).collect();
    assert!(ids.contains(&"real"));
    assert!(!ids.iter().any(|s| s.contains("_skip")));
    assert!(!ids.iter().any(|s| s.contains(".hidden")));
  }

  #[test]
  fn walk_docs_nested() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path();
    write_file(&base.join("axum/basic/index.md"), "# basic\nbody");
    write_file(&base.join("axum/index.md"), "# axum\nbody");
    let mut out = Vec::new();
    walk_docs(base, base, &mut out);
    let ids: Vec<&str> = out.iter().map(|d| d.ref_id.as_str()).collect();
    assert!(ids.contains(&"axum"));
    assert!(ids.contains(&"axum/basic"));
    // url 拼接正确
    let basic = out.iter().find(|d| d.ref_id == "axum/basic").expect("found");
    assert_eq!(basic.url, "/docs/axum/basic");
  }

  #[test]
  fn case_documents_use_case_url_shape() {
    let doc = IndexedDocument {
      kind: "case".to_string(),
      ref_id: "demo".to_string(),
      title: "Demo".to_string(),
      body: "body".to_string(),
      url: "/case/demo".to_string(),
      created_at: "2026-01-01".to_string(),
    };
    assert_eq!(doc.kind, "case");
    assert_eq!(doc.url, "/case/demo");
  }

  fn doc(kind: &str, id: &str) -> IndexedDocument {
    IndexedDocument {
      kind: kind.to_string(),
      ref_id: id.to_string(),
      title: id.to_string(),
      body: String::new(),
      url: format!("/{}/{}", kind, id),
      created_at: String::new(),
    }
  }

  #[test]
  fn filter_documents_keeps_only_enabled_modules() {
    let all = vec![doc("blog", "a"), doc("doc", "b"), doc("topic", "1"), doc("case", "c")];
    let enabled = vec!["blog".to_string(), "docs".to_string()];
    let filtered = filter_documents_by_enabled(all, &enabled);
    let kinds: Vec<&str> = filtered.iter().map(|d| d.kind.as_str()).collect();
    assert!(kinds.contains(&"blog"));
    assert!(kinds.contains(&"doc"));
    assert!(!kinds.contains(&"topic"));
    assert!(!kinds.contains(&"case"));
  }

  #[test]
  fn filter_documents_drops_all_when_no_modules_enabled() {
    let all = vec![doc("blog", "a"), doc("doc", "b")];
    let filtered = filter_documents_by_enabled(all, &[]);
    assert!(filtered.is_empty());
  }

  #[test]
  fn filter_documents_keeps_unknown_kinds() {
    // 未来若新增 lesson / podcast 等 kind，不应被默认丢弃
    let all = vec![doc("lesson", "a"), doc("podcast", "b")];
    let filtered = filter_documents_by_enabled(all, &["blog".to_string()]);
    assert_eq!(filtered.len(), 2);
  }
}
