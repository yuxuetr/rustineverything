//! 索引源:从 `assets/posts/`、`assets/docs/` 与 PostgreSQL 中
//! 收集所有可被搜索的文档。
//!
//! 输出格式 [`IndexedDocument`] 与引擎层的 schema 一一对应。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

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

/// 带 mtime 的文件来源文档。Phase 7.3.3 用于 mtime 差分增量索引。
#[derive(Debug, Clone)]
pub struct VersionedDoc {
  pub doc: IndexedDocument,
  pub mtime_secs: u64,
}

/// 持久化的索引清单。与 tantivy 索引文件并存于 `SEARCH_INDEX_DIR/manifest.json`。
///
/// - `files`：文件来源 (`blog`/`doc`/`embedded` 等) 的 `doc_uid → mtime_secs`，
///   用于 mtime 差分（mtime 不变 → 跳过；不同 → upsert；磁盘缺失 → delete）。
/// - `dyn_uids`：动态来源（cases / topics 等）uid 集合，仅用于检测删除：
///   动态来源每次都全量 upsert（没有可靠的 version key），但靠 manifest
///   记录上次见过的 uid 集合，本次缺失的 → delete。
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexManifest {
  #[serde(default = "default_manifest_version")]
  pub version: u32,
  #[serde(default)]
  pub files: BTreeMap<String, u64>,
  #[serde(default)]
  pub dyn_uids: BTreeSet<String>,
}

fn default_manifest_version() -> u32 {
  1
}

impl IndexManifest {
  pub const FILE_NAME: &'static str = "manifest.json";

  /// 加载磁盘上的清单；不存在或解析失败时返回默认空清单（视作首次构建）。
  pub fn load(dir: &Path) -> Self {
    let path = dir.join(Self::FILE_NAME);
    let Ok(raw) = std::fs::read_to_string(&path) else {
      return Self::default();
    };
    serde_json::from_str(&raw).unwrap_or_else(|e| {
      tracing::warn!(path = %path.display(), error = %e, "search: manifest parse failed, treating as empty");
      Self::default()
    })
  }

  /// 原子写入：先写 `.tmp`，再 `rename` 替换正式文件。
  pub fn save(&self, dir: &Path) -> Result<(), String> {
    let final_path = dir.join(Self::FILE_NAME);
    let tmp_path = dir.join(format!("{}.tmp", Self::FILE_NAME));
    let raw = serde_json::to_string_pretty(self)
      .map_err(|e| format!("search: manifest serialize failed: {}", e))?;
    std::fs::write(&tmp_path, raw)
      .map_err(|e| format!("search: manifest write {} failed: {}", tmp_path.display(), e))?;
    std::fs::rename(&tmp_path, &final_path)
      .map_err(|e| format!("search: manifest rename failed: {}", e))
  }
}

/// 增量 reindex 的差分结果：要 upsert 的文档 + 要 delete 的 uid + 下一版 manifest。
#[derive(Debug, Default, Clone)]
pub struct ReindexDiff {
  pub upserts: Vec<IndexedDocument>,
  pub deletes: Vec<String>,
  pub next: IndexManifest,
}

/// 计算增量差分（纯函数，便于单测）。
///
/// `current_files` 来自磁盘扫描（带 mtime），`current_dyn` 来自动态来源
/// （DB / 多文件聚合，无 mtime）。
pub fn diff_for_reindex(
  prev: &IndexManifest,
  current_files: Vec<VersionedDoc>,
  current_dyn: Vec<IndexedDocument>,
) -> ReindexDiff {
  let mut upserts = Vec::new();
  let mut deletes = Vec::new();
  let mut next_files = BTreeMap::new();

  // 文件来源：按 mtime 差分。
  let mut current_uids = BTreeSet::new();
  for VersionedDoc { doc, mtime_secs } in current_files {
    let uid = format!("{}:{}", doc.kind, doc.ref_id);
    let changed = match prev.files.get(&uid) {
      Some(prev_mtime) => *prev_mtime != mtime_secs,
      None => true,
    };
    if changed {
      upserts.push(doc);
    }
    next_files.insert(uid.clone(), mtime_secs);
    current_uids.insert(uid);
  }
  // prev.files 中本次未见的 uid → 已被删除
  for prev_uid in prev.files.keys() {
    if !current_uids.contains(prev_uid) {
      deletes.push(prev_uid.clone());
    }
  }

  // 动态来源：全量 upsert，并按上次 uid 集合检测删除。
  let mut next_dyn = BTreeSet::new();
  for d in current_dyn {
    let uid = format!("{}:{}", d.kind, d.ref_id);
    next_dyn.insert(uid);
    upserts.push(d);
  }
  for prev_uid in prev.dyn_uids.iter() {
    if !next_dyn.contains(prev_uid) {
      deletes.push(prev_uid.clone());
    }
  }

  ReindexDiff {
    upserts,
    deletes,
    next: IndexManifest { version: 1, files: next_files, dyn_uids: next_dyn },
  }
}

/// 文件 mtime（自 UNIX epoch 的秒数）。读不到时返回 0。
fn file_mtime_secs(path: &Path) -> u64 {
  std::fs::metadata(path)
    .and_then(|m| m.modified())
    .ok()
    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
    .map(|d| d.as_secs())
    .unwrap_or(0)
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

fn collect_blogs_versioned() -> Vec<VersionedDoc> {
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
      let mtime = file_mtime_secs(&chosen);
      out.push(VersionedDoc {
        doc: IndexedDocument {
          kind: "blog".to_string(),
          ref_id: slug.clone(),
          title,
          body,
          url: format!("/blog/{}", slug),
          created_at: date,
        },
        mtime_secs: mtime,
      });
    }
  }
  out
}

/// Phase 6 内容板块（embedded/ai/web3/wasm/cli）。每个板块的 `kind` 即其
/// module id，url 形如 `/<board>/<slug>`。文章来自 `assets/topics/<board>/`。
const BOARD_IDS: &[&str] = &["embedded", "ai", "web3", "wasm", "cli"];

fn collect_boards_versioned() -> Vec<VersionedDoc> {
  let mut out = Vec::new();
  for board in BOARD_IDS {
    let dir = get_asset_root().join("topics").join(board);
    let entries = match std::fs::read_dir(&dir) {
      Ok(e) => e,
      Err(_) => continue,
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
        let mtime = file_mtime_secs(&chosen);
        out.push(VersionedDoc {
          doc: IndexedDocument {
            kind: board.to_string(),
            ref_id: slug.clone(),
            title,
            body,
            url: format!("/{}/{}", board, slug),
            created_at: date,
          },
          mtime_secs: mtime,
        });
      }
    }
  }
  out
}

fn collect_docs_versioned() -> Vec<VersionedDoc> {
  let mut out = Vec::new();
  let root = get_asset_root().join("docs");
  if !root.exists() {
    return out;
  }
  walk_docs(&root, &root, &mut out);
  out
}

fn walk_docs(base: &Path, dir: &Path, out: &mut Vec<VersionedDoc>) {
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
        let mtime = file_mtime_secs(&file);
        out.push(VersionedDoc {
          doc: IndexedDocument {
            kind: "doc".to_string(),
            ref_id: rel_str.clone(),
            title,
            body,
            url: format!("/docs/{}", rel_str),
            created_at: date,
          },
          mtime_secs: mtime,
        });
      }
    }
    // 递归
    walk_docs(base, &path, out);
  }
}

#[cfg(feature = "server")]
async fn collect_topics() -> Result<Vec<IndexedDocument>, String> {
  use app_core::entities::topic;
  use sea_orm::EntityTrait;

  let db = match app_core::db::get_or_init_pool().await {
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
  use module_cases::server::scan_cases;

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
  let file_docs = collect_file_documents();
  #[cfg(feature = "server")]
  let dyn_docs = collect_dyn_documents().await;
  #[cfg(not(feature = "server"))]
  let dyn_docs: Vec<IndexedDocument> = Vec::new();

  let mut all: Vec<IndexedDocument> = file_docs.into_iter().map(|v| v.doc).collect();
  all.extend(dyn_docs);

  #[cfg(feature = "server")]
  {
    let enabled = app_core::engines::module::default_module_engine().enabled_ids();
    all = filter_documents_by_enabled(all, &enabled);
  }

  Ok(all)
}

/// 收集所有文件来源的索引文档（带 mtime，供 Phase 7.3.3 增量索引使用）。
///
/// 来源：blog / boards (`embedded`/`ai`/...) / doc。不包含 cases / topics
/// 等动态来源（见 [`collect_dyn_documents`]）。**不**应用模块开关过滤；
/// 由调用方在 diff/写入前用 [`filter_versioned_by_enabled`] 处理。
pub fn collect_file_documents() -> Vec<VersionedDoc> {
  let mut out = Vec::new();
  out.extend(collect_blogs_versioned());
  out.extend(collect_boards_versioned());
  out.extend(collect_docs_versioned());
  out
}

/// 收集所有动态来源（multi-file 聚合 / DB）文档：cases + topics。
///
/// 这些来源没有可靠的单文件 mtime（cases 是目录聚合，topics 来自 DB），
/// 因此增量索引时一律走「全量 upsert + 上次 uid 集合差分检测删除」路径。
#[cfg(feature = "server")]
pub async fn collect_dyn_documents() -> Vec<IndexedDocument> {
  let mut out = Vec::new();
  match collect_topics().await {
    Ok(mut t) => out.append(&mut t),
    Err(e) => tracing::warn!(error = %e, "search: failed to collect topics"),
  }
  out.extend(collect_cases());
  out
}

/// 按模块开关过滤 [`VersionedDoc`]（与 [`filter_documents_by_enabled`] 同语义）。
pub fn filter_versioned_by_enabled(
  docs: Vec<VersionedDoc>,
  enabled_module_ids: &[String],
) -> Vec<VersionedDoc> {
  let is_on = |id: &str| enabled_module_ids.iter().any(|s| s == id);
  docs
    .into_iter()
    .filter(|v| match v.doc.kind.as_str() {
      "blog" => is_on("blog"),
      "doc" => is_on("docs"),
      "topic" => is_on("forum"),
      "case" => is_on("cases"),
      k if BOARD_IDS.contains(&k) => is_on(k),
      _ => true,
    })
    .collect()
}

/// Phase 3.4：按 module id 过滤已汇总的索引文档，便于上层显式调用。
///
/// `enabled_module_ids` 来自 [`app_core::engines::module::ModuleEngine::enabled_ids`]。
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
      k if BOARD_IDS.contains(&k) => is_on(k),
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
    let ids: Vec<&str> = out.iter().map(|v| v.doc.ref_id.as_str()).collect();
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
    let ids: Vec<&str> = out.iter().map(|v| v.doc.ref_id.as_str()).collect();
    assert!(ids.contains(&"axum"));
    assert!(ids.contains(&"axum/basic"));
    // url 拼接正确
    let basic = out.iter().find(|v| v.doc.ref_id == "axum/basic").expect("found");
    assert_eq!(basic.doc.url, "/docs/axum/basic");
    // mtime 已读取（tempdir 刚写入，应 > 0）
    assert!(basic.mtime_secs > 0);
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

  #[test]
  fn filter_documents_gates_content_boards_by_module() {
    // Phase 6 板块：kind 即 module id，关闭即从搜索剔除。
    let all = vec![doc("embedded", "x"), doc("ai", "y"), doc("web3", "z")];
    let enabled = vec!["embedded".to_string(), "ai".to_string()];
    let filtered = filter_documents_by_enabled(all, &enabled);
    let kinds: Vec<&str> = filtered.iter().map(|d| d.kind.as_str()).collect();
    assert!(kinds.contains(&"embedded"));
    assert!(kinds.contains(&"ai"));
    assert!(!kinds.contains(&"web3"));
  }

  // ---- Phase 7.3.3：IndexManifest / diff_for_reindex ----

  fn versioned(kind: &str, ref_id: &str, mtime: u64) -> VersionedDoc {
    VersionedDoc {
      doc: IndexedDocument {
        kind: kind.to_string(),
        ref_id: ref_id.to_string(),
        title: ref_id.to_string(),
        body: String::new(),
        url: format!("/{}/{}", kind, ref_id),
        created_at: String::new(),
      },
      mtime_secs: mtime,
    }
  }

  fn manifest(files: &[(&str, u64)], dyn_uids: &[&str]) -> IndexManifest {
    IndexManifest {
      version: 1,
      files: files.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
      dyn_uids: dyn_uids.iter().map(|s| s.to_string()).collect(),
    }
  }

  #[test]
  fn manifest_round_trip_through_disk() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut files = BTreeMap::new();
    files.insert("blog:a".to_string(), 100u64);
    files.insert("doc:b/c".to_string(), 200u64);
    let mut dyn_uids = BTreeSet::new();
    dyn_uids.insert("case:demo".to_string());
    let m = IndexManifest { version: 1, files, dyn_uids };
    m.save(tmp.path()).expect("save");
    let loaded = IndexManifest::load(tmp.path());
    assert_eq!(loaded, m);
  }

  #[test]
  fn manifest_load_missing_returns_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let loaded = IndexManifest::load(tmp.path());
    assert_eq!(loaded, IndexManifest::default());
  }

  #[test]
  fn manifest_load_corrupt_returns_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(IndexManifest::FILE_NAME), "not json {").unwrap();
    let loaded = IndexManifest::load(tmp.path());
    assert_eq!(loaded, IndexManifest::default());
  }

  #[test]
  fn diff_detects_added_file() {
    let prev = IndexManifest::default();
    let current = vec![versioned("blog", "a", 100)];
    let d = diff_for_reindex(&prev, current, vec![]);
    assert_eq!(d.upserts.len(), 1);
    assert_eq!(d.upserts[0].ref_id, "a");
    assert!(d.deletes.is_empty());
    assert_eq!(d.next.files.get("blog:a"), Some(&100));
  }

  #[test]
  fn diff_detects_modified_file_via_mtime() {
    let prev = manifest(&[("blog:a", 100)], &[]);
    let current = vec![versioned("blog", "a", 200)];
    let d = diff_for_reindex(&prev, current, vec![]);
    assert_eq!(d.upserts.len(), 1);
    assert!(d.deletes.is_empty());
    assert_eq!(d.next.files.get("blog:a"), Some(&200));
  }

  #[test]
  fn diff_skips_unchanged_file() {
    let prev = manifest(&[("blog:a", 100)], &[]);
    let current = vec![versioned("blog", "a", 100)];
    let d = diff_for_reindex(&prev, current, vec![]);
    assert!(d.upserts.is_empty());
    assert!(d.deletes.is_empty());
  }

  #[test]
  fn diff_detects_removed_file() {
    let prev = manifest(&[("blog:a", 100), ("blog:b", 100)], &[]);
    let current = vec![versioned("blog", "a", 100)];
    let d = diff_for_reindex(&prev, current, vec![]);
    assert!(d.upserts.is_empty());
    assert_eq!(d.deletes, vec!["blog:b".to_string()]);
    assert!(!d.next.files.contains_key("blog:b"));
  }

  #[test]
  fn diff_dyn_treats_all_current_as_upserts() {
    // 动态来源：即便上次已见过同一 uid，也照样 upsert（无可靠 version key）。
    let prev = manifest(&[], &["case:demo"]);
    let current_dyn = vec![doc("case", "demo")];
    let d = diff_for_reindex(&prev, vec![], current_dyn);
    assert_eq!(d.upserts.len(), 1);
    assert_eq!(d.upserts[0].ref_id, "demo");
    assert!(d.deletes.is_empty());
    assert!(d.next.dyn_uids.contains("case:demo"));
  }

  #[test]
  fn diff_dyn_detects_removed_dyn_uid() {
    let prev = manifest(&[], &["case:a", "case:b"]);
    let current_dyn = vec![doc("case", "a")];
    let d = diff_for_reindex(&prev, vec![], current_dyn);
    assert_eq!(d.upserts.len(), 1); // a re-upserted
    assert_eq!(d.deletes, vec!["case:b".to_string()]);
  }

  #[test]
  fn diff_mixes_file_and_dyn_sources() {
    let prev = manifest(&[("blog:keep", 50), ("blog:remove", 50)], &["case:gone"]);
    let current_files = vec![versioned("blog", "keep", 50), versioned("blog", "new", 99)];
    let current_dyn = vec![doc("case", "kept")];
    let d = diff_for_reindex(&prev, current_files, current_dyn);

    // upserts：新文件 + 动态来源（每次都 upsert）
    let upsert_ids: Vec<&str> = d.upserts.iter().map(|u| u.ref_id.as_str()).collect();
    assert!(upsert_ids.contains(&"new"));
    assert!(upsert_ids.contains(&"kept"));
    assert!(!upsert_ids.contains(&"keep")); // unchanged → skip

    // deletes：消失的文件 + 消失的动态 uid
    let mut deletes_sorted = d.deletes.clone();
    deletes_sorted.sort();
    assert_eq!(deletes_sorted, vec!["blog:remove".to_string(), "case:gone".to_string()]);
  }

  #[test]
  fn filter_versioned_by_enabled_respects_module_switches() {
    let docs =
      vec![versioned("blog", "a", 1), versioned("embedded", "b", 1), versioned("ai", "c", 1)];
    let enabled = vec!["blog".to_string(), "embedded".to_string()];
    let filtered = filter_versioned_by_enabled(docs, &enabled);
    let kinds: Vec<&str> = filtered.iter().map(|v| v.doc.kind.as_str()).collect();
    assert!(kinds.contains(&"blog"));
    assert!(kinds.contains(&"embedded"));
    assert!(!kinds.contains(&"ai"));
  }
}
