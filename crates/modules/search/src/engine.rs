//! 基于 tantivy 的内嵌全文检索引擎（MmapDirectory 持久化 + 中文 jieba 分词）。
//!
//! 设计要点：
//! - 全局单例，首次查询时 lazy 加载或构建索引；`rebuild()` 强制全量重建。
//! - 使用 `MmapDirectory` 持久化到磁盘（Phase 7.3.1）：路径由
//!   `SEARCH_INDEX_DIR` 环境变量控制，默认 `data/search-index`。
//!   重启进程后索引文件仍在，无需重新扫描磁盘 / DB。
//! - schema 不匹配（例如新增字段后旧索引仍在）时，自动清空目录并重建。
//! - 多字段 schema（kind / ref_id / title / body / url / created_at）。
//! - 中文使用 jieba，英文/数字走 lowercase，默认 OR 查询。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, TextFieldIndexing, TextOptions, FAST, STORED, STRING};
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, TantivyError, Term};

use crate::indexer::{
  collect_dyn_documents, collect_file_documents, diff_for_reindex, filter_versioned_by_enabled,
  IndexManifest, IndexedDocument,
};
use crate::text::truncate_chars;

pub const TOKENIZER_NAME: &str = "jieba";

/// 默认持久化目录（相对工作目录）。可通过 `SEARCH_INDEX_DIR` 覆盖。
pub const DEFAULT_INDEX_DIR: &str = "data/search-index";

/// 命中结果（与 server 层对齐）。
#[derive(Debug, Clone)]
pub struct EngineHit {
  pub kind: String,
  pub ref_id: String,
  pub title: String,
  pub snippet: String,
  pub url: String,
  pub created_at: String,
  pub score: f32,
}

/// schema 字段引用集合。
#[derive(Clone)]
pub struct SearchFields {
  /// 文档唯一标识 `{kind}:{ref_id}`，用于增量 upsert / delete（Phase 7.3.2）。
  pub doc_uid: Field,
  pub kind: Field,
  pub ref_id: Field,
  pub title: Field,
  pub body: Field,
  pub url: Field,
  pub created_at: Field,
}

pub fn build_schema() -> (Schema, SearchFields) {
  let mut builder = Schema::builder();

  let text_indexing = TextFieldIndexing::default()
    .set_tokenizer(TOKENIZER_NAME)
    .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);
  let text_options = TextOptions::default().set_indexing_options(text_indexing).set_stored();

  // doc_uid 用 STRING（单 token，整串精确匹配）+ STORED，方便 `delete_term` 命中。
  let doc_uid = builder.add_text_field("doc_uid", STRING | STORED);
  let kind = builder.add_text_field("kind", STRING | STORED | FAST);
  let ref_id = builder.add_text_field("ref_id", STRING | STORED);
  let title = builder.add_text_field("title", text_options.clone());
  let body = builder.add_text_field("body", text_options);
  let url = builder.add_text_field("url", STRING | STORED);
  let created_at = builder.add_text_field("created_at", STRING | STORED);

  let schema = builder.build();
  let fields = SearchFields { doc_uid, kind, ref_id, title, body, url, created_at };
  (schema, fields)
}

/// 文档 uid 拼接：`{kind}:{ref_id}`。供 indexer / engine 共用。
pub fn doc_uid(kind: &str, ref_id: &str) -> String {
  format!("{}:{}", kind, ref_id)
}

/// 解析持久化目录：优先 `SEARCH_INDEX_DIR`，否则用 [`DEFAULT_INDEX_DIR`]。
pub fn resolve_index_dir() -> PathBuf {
  std::env::var("SEARCH_INDEX_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|_| PathBuf::from(DEFAULT_INDEX_DIR))
}

/// 引擎实例：持有 Index + Reader + 字段引用 + 持久化路径。
pub struct SearchEngine {
  pub index: Index,
  pub reader: IndexReader,
  pub fields: SearchFields,
  pub dir: PathBuf,
}

impl SearchEngine {
  /// 在 `dir` 上打开或创建一个 `MmapDirectory` 支持的索引。
  ///
  /// 行为：
  /// - 目录不存在 → `create_dir_all` 创建。
  /// - 已存在索引且 schema 匹配 → 直接打开（不动数据）。
  /// - 已存在索引但 schema 不匹配 → 清空目录后重新创建（schema 迁移）。
  /// - 目录为空（首次启动）→ 创建空索引，由调用方决定是否填充。
  ///
  /// 返回 `(engine, was_freshly_created)`：第二个布尔表示是否是「目录为空 →
  /// 新建空索引」，调用方可据此判断是否需要全量填充。
  pub fn open_or_create(dir: &Path) -> Result<(Self, bool), String> {
    std::fs::create_dir_all(dir)
      .map_err(|e| format!("search: create_dir_all {} failed: {}", dir.display(), e))?;

    let (schema, fields) = build_schema();
    let mmap = MmapDirectory::open(dir)
      .map_err(|e| format!("search: open MmapDirectory {} failed: {}", dir.display(), e))?;

    let index = match Index::open_or_create(mmap, schema.clone()) {
      Ok(idx) => idx,
      Err(TantivyError::SchemaError(msg)) => {
        tracing::warn!(
          dir = %dir.display(),
          error = %msg,
          "search: schema mismatch on existing index, wiping and recreating"
        );
        wipe_index_dir(dir)?;
        let mmap = MmapDirectory::open(dir)
          .map_err(|e| format!("search: reopen MmapDirectory after wipe failed: {}", e))?;
        Index::open_or_create(mmap, schema)
          .map_err(|e| format!("search: recreate index after wipe failed: {}", e))?
      }
      Err(e) => return Err(format!("search: open_or_create failed: {}", e)),
    };

    // 注册 jieba 分词器（运行时状态，每次 open 都要重新注册）。
    let analyzer: TextAnalyzer = tantivy_jieba::JiebaTokenizer::default().into();
    index.tokenizers().register(TOKENIZER_NAME, analyzer);

    let reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::Manual)
      .try_into()
      .map_err(|e: TantivyError| format!("search: build reader failed: {}", e))?;

    let was_freshly_created = reader.searcher().num_docs() == 0;

    Ok((Self { index, reader, fields, dir: dir.to_path_buf() }, was_freshly_created))
  }

  /// 用给定文档全量替换当前索引内容（清空 → 写入 → commit → reload reader）。
  pub fn replace_all(&self, docs: Vec<IndexedDocument>) -> Result<usize, String> {
    let mut writer = self.new_writer()?;
    writer
      .delete_all_documents()
      .map_err(|e| format!("search: delete_all_documents failed: {}", e))?;

    let count = docs.len();
    for d in docs {
      self.add_one(&mut writer, &d)?;
    }
    writer.commit().map_err(|e| format!("search: commit failed: {}", e))?;
    self.reader.reload().map_err(|e| format!("search: reader reload failed: {}", e))?;
    Ok(count)
  }

  /// 增量 upsert 单篇文档（先按 `doc_uid` 删除旧版，再插入新版）。
  ///
  /// 适合单文档热更新；如需批量请用 [`Self::upsert_documents`]，单次 writer +
  /// 单次 commit 显著更高效。
  pub fn upsert_document(&self, doc: &IndexedDocument) -> Result<(), String> {
    self.upsert_documents(std::slice::from_ref(doc)).map(|_| ())
  }

  /// 增量 upsert 一组文档：单 writer / 单 commit。返回写入的文档数。
  ///
  /// 语义：对每篇 `(kind, ref_id)` 已存在的旧文档 → 删除；然后写入新版。
  /// 调用方需保证每个 `IndexedDocument` 的 `(kind, ref_id)` 在批内唯一，
  /// 否则同 uid 的最后一篇会胜出。
  pub fn upsert_documents(&self, docs: &[IndexedDocument]) -> Result<usize, String> {
    if docs.is_empty() {
      return Ok(0);
    }
    let mut writer = self.new_writer()?;
    for d in docs {
      let term = Term::from_field_text(self.fields.doc_uid, &doc_uid(&d.kind, &d.ref_id));
      writer.delete_term(term);
      self.add_one(&mut writer, d)?;
    }
    writer.commit().map_err(|e| format!("search: commit failed: {}", e))?;
    self.reader.reload().map_err(|e| format!("search: reader reload failed: {}", e))?;
    Ok(docs.len())
  }

  /// 增量删除一组文档（按 `doc_uid`）。不存在的 uid 静默忽略。
  pub fn delete_documents(&self, uids: &[String]) -> Result<usize, String> {
    if uids.is_empty() {
      return Ok(0);
    }
    let mut writer = self.new_writer()?;
    for uid in uids {
      let term = Term::from_field_text(self.fields.doc_uid, uid);
      writer.delete_term(term);
    }
    writer.commit().map_err(|e| format!("search: commit failed: {}", e))?;
    self.reader.reload().map_err(|e| format!("search: reader reload failed: {}", e))?;
    Ok(uids.len())
  }

  fn new_writer(&self) -> Result<IndexWriter, String> {
    self.index.writer(50_000_000).map_err(|e| format!("search: writer init failed: {}", e))
  }

  fn add_one(&self, writer: &mut IndexWriter, d: &IndexedDocument) -> Result<(), String> {
    let uid = doc_uid(&d.kind, &d.ref_id);
    writer
      .add_document(doc!(
          self.fields.doc_uid => uid,
          self.fields.kind => d.kind.clone(),
          self.fields.ref_id => d.ref_id.clone(),
          self.fields.title => d.title.clone(),
          self.fields.body => d.body.clone(),
          self.fields.url => d.url.clone(),
          self.fields.created_at => d.created_at.clone(),
      ))
      .map(|_| ())
      .map_err(|e| format!("search: add_document failed: {}", e))
  }

  /// 查询：返回按 BM25 分数排序的结果。
  /// `kind_filter`：Some("blog") 仅命中博客，None 不过滤。
  pub fn query(
    &self,
    text: &str,
    kind_filter: Option<&str>,
    limit: usize,
  ) -> tantivy::Result<Vec<EngineHit>> {
    let q = text.trim();
    if q.is_empty() {
      return Ok(vec![]);
    }

    let searcher = self.reader.searcher();
    let mut parser = QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.body]);
    parser.set_field_boost(self.fields.title, 3.0);

    let final_query = match kind_filter {
      Some(k) if !k.is_empty() => format!("({}) AND kind:{}", escape_query(q), k),
      _ => q.to_string(),
    };

    let query =
      parser.parse_query(&final_query).or_else(|_| parser.parse_query(&parser_safe(q)))?;
    let top = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;

    let mut hits = Vec::with_capacity(top.len());
    for (score, address) in top {
      let retrieved: TantivyDocument = searcher.doc(address)?;
      hits.push(self.doc_to_hit(&retrieved, score, q));
    }
    Ok(hits)
  }

  fn doc_to_hit(&self, doc: &TantivyDocument, score: f32, query_text: &str) -> EngineHit {
    let kind = first_text(doc, self.fields.kind);
    let ref_id = first_text(doc, self.fields.ref_id);
    let title = first_text(doc, self.fields.title);
    let body = first_text(doc, self.fields.body);
    let url = first_text(doc, self.fields.url);
    let created_at = first_text(doc, self.fields.created_at);

    let snippet = make_snippet(&body, query_text, 200);
    EngineHit { kind, ref_id, title, snippet, url, created_at, score }
  }
}

/// 清空索引目录内的所有条目（保留目录本身），用于 schema 迁移。
fn wipe_index_dir(dir: &Path) -> Result<(), String> {
  let entries = std::fs::read_dir(dir)
    .map_err(|e| format!("search: read_dir {} failed: {}", dir.display(), e))?;
  for entry in entries.flatten() {
    let path = entry.path();
    let result = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    result.map_err(|e| format!("search: wipe {} failed: {}", path.display(), e))?;
  }
  Ok(())
}

/// 防御性查询转义：保留 ASCII 字母数字、空格、引号、中文等。
fn escape_query(q: &str) -> String {
  q.chars()
    .map(|c| match c {
      '+' | '-' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '~' | '*' | '?' | ':' | '\\'
      | '/' => ' ',
      other => other,
    })
    .collect()
}

/// 进一步降级：如果 parse 仍失败，只保留 alphanumeric + 中文，避免崩溃。
fn parser_safe(q: &str) -> String {
  q.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || (*c as u32) > 0x7F).collect()
}

fn first_text(doc: &TantivyDocument, field: Field) -> String {
  use tantivy::schema::Value;
  doc.get_first(field).and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default()
}

/// 简单 snippet：命中位置前后取 max_chars/2 的窗口，过短则取开头。
fn make_snippet(body: &str, query: &str, max_chars: usize) -> String {
  let trimmed_body = body.trim();
  if trimmed_body.is_empty() {
    return String::new();
  }
  let token = query.split_whitespace().max_by_key(|t| t.chars().count()).unwrap_or(query);
  let lower_body = trimmed_body.to_lowercase();
  let lower_token = token.to_lowercase();
  let half = max_chars / 2;

  if let Some(byte_pos) = lower_body.find(&lower_token) {
    let char_pos = trimmed_body[..byte_pos.min(trimmed_body.len())].chars().count();
    let start_char = char_pos.saturating_sub(half);
    let mut chars_iter = trimmed_body.char_indices();
    let mut start_byte = 0usize;
    for (i, (b, _)) in chars_iter.by_ref().enumerate() {
      if i == start_char {
        start_byte = b;
        break;
      }
    }
    if start_char == 0 {
      start_byte = 0;
    }
    let snippet = &trimmed_body[start_byte..];
    let prefix = if start_char > 0 { "…" } else { "" };
    let truncated = truncate_chars(snippet, max_chars);
    format!("{}{}", prefix, truncated)
  } else {
    truncate_chars(trimmed_body, max_chars)
  }
}

// =============================================================
// 全局单例 + 入口
// =============================================================

static ENGINE: OnceLock<Mutex<Option<Arc<SearchEngine>>>> = OnceLock::new();

fn engine_slot() -> &'static Mutex<Option<Arc<SearchEngine>>> {
  ENGINE.get_or_init(|| Mutex::new(None))
}

/// 获取当前引擎；若未初始化则在默认目录上 [`init_or_load`]。
pub async fn get_or_build() -> Result<Arc<SearchEngine>, String> {
  {
    let guard = engine_slot().lock().map_err(|e| format!("search lock poisoned: {}", e))?;
    if let Some(e) = guard.as_ref() {
      return Ok(e.clone());
    }
  }
  init_or_load(&resolve_index_dir()).await
}

/// 启动期入口：在 `dir` 上打开或创建索引。
///
/// - 目录已存在索引（非空）→ 直接复用，**不**重新扫描磁盘。
/// - 目录为空 / 首次启动 → 全量扫描 + 写 [`IndexManifest`]，为后续增量索引备好基线。
/// - schema 不匹配 → 自动清空重建（[`SearchEngine::open_or_create`]）后填充。
pub async fn init_or_load(dir: &Path) -> Result<Arc<SearchEngine>, String> {
  let (engine, was_freshly_created) = SearchEngine::open_or_create(dir)?;
  let engine = Arc::new(engine);

  if was_freshly_created {
    let count = full_populate_with_manifest(&engine).await?;
    tracing::info!(
      dir = %dir.display(),
      documents = count,
      "search: built initial index from disk + DB"
    );
  } else {
    let existing = engine.reader.searcher().num_docs();
    tracing::info!(
      dir = %dir.display(),
      documents = existing,
      "search: loaded existing index"
    );
  }

  let mut guard = engine_slot().lock().map_err(|e| format!("search lock poisoned: {}", e))?;
  *guard = Some(engine.clone());
  Ok(engine)
}

/// 强制全量重建当前引擎的索引（保持持久化目录），并同步刷新 manifest。
pub async fn rebuild() -> Result<Arc<SearchEngine>, String> {
  let engine = match engine_slot().lock() {
    Ok(g) => g.as_ref().cloned(),
    Err(e) => return Err(format!("search lock poisoned: {}", e)),
  };
  let engine = match engine {
    Some(e) => e,
    None => return init_or_load(&resolve_index_dir()).await,
  };

  let count = full_populate_with_manifest(&engine).await?;
  tracing::info!(documents = count, dir = %engine.dir.display(), "search: index rebuilt (full)");
  Ok(engine)
}

/// Phase 7.3.3：mtime 差分 + 动态来源 diff 的增量 reindex 报告。
#[derive(Debug, Clone, Default)]
pub struct IncrementalReport {
  pub upserts: usize,
  pub deletes: usize,
  pub elapsed_ms: u64,
}

/// 增量重建：基于上一版 [`IndexManifest`] 与当前磁盘 / DB 状态做差分。
///
/// 文件来源按 mtime 跳过未变文档；动态来源（cases / topics）全量 upsert，
/// 但通过 manifest 检测删除。比 [`rebuild`] 显著更快，适合频繁调用。
pub async fn reindex_incremental() -> Result<IncrementalReport, String> {
  use std::time::Instant;

  let engine = get_or_build().await?;
  let started = Instant::now();
  let prev = IndexManifest::load(&engine.dir);

  let file_docs = collect_file_documents();
  let dyn_docs = collect_dyn_documents().await;
  let enabled = app_core::engines::module::default_module_engine().enabled_ids();
  let file_docs = filter_versioned_by_enabled(file_docs, &enabled);
  let dyn_docs = crate::indexer::filter_documents_by_enabled(dyn_docs, &enabled);

  let diff = diff_for_reindex(&prev, file_docs, dyn_docs);
  let upserts = diff.upserts.len();
  let deletes = diff.deletes.len();
  engine.upsert_documents(&diff.upserts)?;
  engine.delete_documents(&diff.deletes)?;
  diff.next.save(&engine.dir)?;

  let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
  tracing::info!(
    upserts,
    deletes,
    elapsed_ms,
    dir = %engine.dir.display(),
    "search: incremental reindex applied"
  );
  Ok(IncrementalReport { upserts, deletes, elapsed_ms })
}

/// 全量填充 + 同步写 manifest。`init_or_load` 和 `rebuild` 共用，保证
/// 每次全量后 manifest 与索引状态一致，后续 `reindex_incremental` 才能正确 diff。
async fn full_populate_with_manifest(engine: &SearchEngine) -> Result<usize, String> {
  let file_docs = collect_file_documents();
  let dyn_docs = collect_dyn_documents().await;
  let enabled = app_core::engines::module::default_module_engine().enabled_ids();
  let file_docs = filter_versioned_by_enabled(file_docs, &enabled);
  let dyn_docs = crate::indexer::filter_documents_by_enabled(dyn_docs, &enabled);

  // 复用纯函数算出新 manifest（prev=空 → 所有 uid 进 next）
  let diff = diff_for_reindex(&IndexManifest::default(), file_docs, dyn_docs);
  let count = engine.replace_all(diff.upserts)?;
  diff.next.save(&engine.dir)?;
  Ok(count)
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn escape_query_strips_specials() {
    let cleaned = escape_query("foo+bar!baz?");
    assert!(!cleaned.contains('+'));
    assert!(!cleaned.contains('!'));
    assert!(!cleaned.contains('?'));
    assert!(cleaned.contains("foo"));
    assert!(cleaned.contains("bar"));
  }

  #[test]
  fn parser_safe_keeps_alphanumeric_and_chinese() {
    let s = parser_safe("abc 中文 123 ?!");
    assert!(s.contains("abc"));
    assert!(s.contains("中文"));
    assert!(s.contains("123"));
    assert!(!s.contains('?'));
    assert!(!s.contains('!'));
  }

  #[test]
  fn snippet_with_match_centered() {
    let body = "this is a long body about Rust programming language with many words. ".repeat(5);
    let snippet = make_snippet(&body, "Rust", 80);
    assert!(snippet.contains("Rust"));
  }

  #[test]
  fn snippet_no_match_returns_prefix() {
    let body = "no match here, just some content about cooking";
    let snippet = make_snippet(body, "Rust", 30);
    assert!(snippet.starts_with("no match"));
  }

  #[test]
  fn snippet_empty_body() {
    assert_eq!(make_snippet("", "Rust", 50), "");
  }

  #[test]
  fn build_schema_has_six_fields() {
    let (schema, fields) = build_schema();
    for f in [fields.kind, fields.ref_id, fields.title, fields.body, fields.url, fields.created_at]
    {
      assert!(!schema.get_field_name(f).is_empty());
    }
  }

  fn sample_docs() -> Vec<IndexedDocument> {
    vec![
      IndexedDocument {
        kind: "blog".to_string(),
        ref_id: "hello-rust".to_string(),
        title: "Hello Rust".to_string(),
        body: "Learning the Rust programming language is fun".to_string(),
        url: "/blog/hello-rust".to_string(),
        created_at: "2025-01-01".to_string(),
      },
      IndexedDocument {
        kind: "doc".to_string(),
        ref_id: "intro".to_string(),
        title: "Tokio Intro".to_string(),
        body: "Async runtime for Rust".to_string(),
        url: "/docs/tokio/intro".to_string(),
        created_at: String::new(),
      },
    ]
  }

  fn open_engine(dir: &Path) -> SearchEngine {
    SearchEngine::open_or_create(dir).expect("open_or_create").0
  }

  #[test]
  fn open_or_create_creates_missing_directory_and_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("nested/search");
    let (engine, fresh) = SearchEngine::open_or_create(&dir).expect("open_or_create");
    assert!(fresh, "freshly created index should be empty");
    assert_eq!(engine.reader.searcher().num_docs(), 0);
    assert!(dir.exists(), "directory was created");
  }

  #[test]
  fn engine_query_basic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng.replace_all(sample_docs()).expect("write");
    let hits = eng.query("Rust", None, 10).expect("query");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].kind, "blog");
    assert!(hits[0].score >= hits.last().unwrap().score);
  }

  #[test]
  fn engine_query_kind_filter() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .replace_all(vec![
        IndexedDocument {
          kind: "blog".to_string(),
          ref_id: "a".to_string(),
          title: "Rust blog".to_string(),
          body: "blog about rust".to_string(),
          url: "/a".to_string(),
          created_at: String::new(),
        },
        IndexedDocument {
          kind: "doc".to_string(),
          ref_id: "b".to_string(),
          title: "Rust doc".to_string(),
          body: "doc about rust".to_string(),
          url: "/b".to_string(),
          created_at: String::new(),
        },
      ])
      .expect("write");
    let hits = eng.query("rust", Some("blog"), 10).expect("query");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, "blog");
  }

  #[test]
  fn engine_query_chinese() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .replace_all(vec![IndexedDocument {
        kind: "blog".to_string(),
        ref_id: "x".to_string(),
        title: "你好 Rust".to_string(),
        body: "这是一篇关于 Rust 编程语言的中文博客文章".to_string(),
        url: "/blog/x".to_string(),
        created_at: String::new(),
      }])
      .expect("write");
    let hits = eng.query("中文", None, 10).expect("query");
    assert_eq!(hits.len(), 1);
  }

  #[test]
  fn engine_query_empty_returns_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .replace_all(vec![IndexedDocument {
        kind: "blog".to_string(),
        ref_id: "x".to_string(),
        title: "T".to_string(),
        body: "B".to_string(),
        url: "/x".to_string(),
        created_at: String::new(),
      }])
      .expect("write");
    let hits = eng.query("   ", None, 10).expect("query");
    assert!(hits.is_empty());
  }

  #[test]
  fn engine_handles_special_chars_gracefully() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .replace_all(vec![IndexedDocument {
        kind: "blog".to_string(),
        ref_id: "x".to_string(),
        title: "Rust".to_string(),
        body: "content".to_string(),
        url: "/x".to_string(),
        created_at: String::new(),
      }])
      .expect("write");
    let r = eng.query("rust!@#?:", None, 10);
    assert!(r.is_ok());
  }

  // ---- Phase 7.3.1：持久化相关测试 ----

  #[test]
  fn index_survives_engine_drop_and_reopen() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();

    {
      let eng = open_engine(&dir);
      eng.replace_all(sample_docs()).expect("write");
      assert_eq!(eng.reader.searcher().num_docs(), 2);
    } // engine dropped here

    let (eng2, fresh) = SearchEngine::open_or_create(&dir).expect("reopen");
    assert!(!fresh, "reopened index must not be reported as freshly created");
    assert_eq!(eng2.reader.searcher().num_docs(), 2);
    let hits = eng2.query("Rust", None, 10).expect("query");
    assert!(!hits.is_empty());
  }

  #[test]
  fn schema_mismatch_wipes_and_recreates() {
    use tantivy::schema::{Schema, STORED, STRING};

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();

    // 用一个完全不同的 schema 在该目录上创建索引，模拟旧版 schema 残留。
    {
      std::fs::create_dir_all(&dir).unwrap();
      let mmap = MmapDirectory::open(&dir).expect("mmap open");
      let mut b = Schema::builder();
      b.add_text_field("only_field", STRING | STORED);
      let other_schema = b.build();
      Index::open_or_create(mmap, other_schema).expect("create alt index");
    }

    // 用当前真实 schema 打开 —— 应该触发 SchemaError 路径，自动清空 + 重建。
    let (eng, fresh) = SearchEngine::open_or_create(&dir).expect("recreate after mismatch");
    assert!(fresh, "after schema-mismatch wipe, index should be empty");
    // 验证能正常写入新 schema 的文档
    eng.replace_all(sample_docs()).expect("write after recreate");
    assert_eq!(eng.reader.searcher().num_docs(), 2);
  }

  #[test]
  fn resolve_index_dir_uses_env_when_set() {
    // SAFETY: 单测内修改进程环境变量；该值在本测试范围内独占。
    let tmp = tempfile::tempdir().expect("tempdir");
    let custom = tmp.path().join("custom-search");
    std::env::set_var("SEARCH_INDEX_DIR", &custom);
    let resolved = resolve_index_dir();
    std::env::remove_var("SEARCH_INDEX_DIR");
    assert_eq!(resolved, custom);
  }

  #[test]
  fn resolve_index_dir_defaults_when_env_missing() {
    std::env::remove_var("SEARCH_INDEX_DIR");
    let resolved = resolve_index_dir();
    assert_eq!(resolved, PathBuf::from(DEFAULT_INDEX_DIR));
  }

  // ---- Phase 7.3.2：增量 upsert / delete ----

  fn doc_with(kind: &str, ref_id: &str, title: &str, body: &str) -> IndexedDocument {
    IndexedDocument {
      kind: kind.to_string(),
      ref_id: ref_id.to_string(),
      title: title.to_string(),
      body: body.to_string(),
      url: format!("/{}/{}", kind, ref_id),
      created_at: String::new(),
    }
  }

  #[test]
  fn doc_uid_format() {
    assert_eq!(doc_uid("blog", "hello"), "blog:hello");
  }

  #[test]
  fn upsert_document_inserts_when_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng.upsert_document(&doc_with("blog", "a", "First", "body a")).expect("upsert");
    assert_eq!(eng.reader.searcher().num_docs(), 1);
    let hits = eng.query("First", None, 10).expect("query");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "First");
  }

  #[test]
  fn upsert_document_replaces_in_place_for_same_uid() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng.upsert_document(&doc_with("blog", "a", "Original", "body v1")).expect("first");
    eng.upsert_document(&doc_with("blog", "a", "Updated", "body v2")).expect("second");
    // 同 uid，无重复
    assert_eq!(eng.reader.searcher().num_docs(), 1);
    // 旧标题 / 旧 body 已不可命中
    assert!(eng.query("Original", None, 10).expect("query").is_empty());
    let hits = eng.query("Updated", None, 10).expect("query");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Updated");
  }

  #[test]
  fn upsert_documents_batches_inserts_and_updates() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .upsert_documents(&[
        doc_with("blog", "a", "Alpha", "firstversion"),
        doc_with("blog", "b", "Beta", "originalbody"),
      ])
      .expect("seed");
    assert_eq!(eng.reader.searcher().num_docs(), 2);

    // 批内一新增 (c) + 一更新 (b)。用完全不同的 body token 以保证旧文档真的消失。
    eng
      .upsert_documents(&[
        doc_with("blog", "c", "Gamma", "thirdversion"),
        doc_with("blog", "b", "Beta", "rewrittenbody"),
      ])
      .expect("incremental");
    assert_eq!(eng.reader.searcher().num_docs(), 3);

    // b 的旧 body token "originalbody" 必须查不到（旧版已删）
    assert!(eng.query("originalbody", None, 10).expect("old b").is_empty());
    // 新 body 可查
    let hits_new = eng.query("rewrittenbody", None, 10).expect("new b");
    assert_eq!(hits_new.len(), 1);
    assert_eq!(hits_new[0].ref_id, "b");
    // 新增的 c 也在
    let hits_c = eng.query("thirdversion", None, 10).expect("c");
    assert_eq!(hits_c.len(), 1);
    assert_eq!(hits_c[0].ref_id, "c");
  }

  #[test]
  fn upsert_documents_empty_input_is_noop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    let n = eng.upsert_documents(&[]).expect("empty");
    assert_eq!(n, 0);
    assert_eq!(eng.reader.searcher().num_docs(), 0);
  }

  #[test]
  fn delete_documents_removes_matching_uids_and_ignores_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .upsert_documents(&[
        doc_with("blog", "a", "A", "alpha"),
        doc_with("blog", "b", "B", "beta"),
        doc_with("doc", "x", "X", "x body"),
      ])
      .expect("seed");
    assert_eq!(eng.reader.searcher().num_docs(), 3);

    // 删一个存在 + 一个不存在；不存在的应静默忽略，不报错。
    eng
      .delete_documents(&[doc_uid("blog", "a"), doc_uid("blog", "does-not-exist")])
      .expect("delete");
    assert_eq!(eng.reader.searcher().num_docs(), 2);
    assert!(eng.query("alpha", None, 10).expect("alpha").is_empty());
    // 其余两篇仍可被命中
    let hits_beta = eng.query("beta", None, 10).expect("beta");
    assert_eq!(hits_beta.len(), 1);
  }

  #[test]
  fn delete_documents_empty_input_is_noop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng.upsert_document(&doc_with("blog", "a", "A", "alpha")).expect("seed");
    let n = eng.delete_documents(&[]).expect("noop");
    assert_eq!(n, 0);
    assert_eq!(eng.reader.searcher().num_docs(), 1);
  }

  #[test]
  fn doc_uid_distinguishes_kinds_with_same_ref_id() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng
      .upsert_documents(&[
        doc_with("blog", "shared", "Blog post", "shared body"),
        doc_with("doc", "shared", "Doc page", "shared body"),
      ])
      .expect("seed");
    assert_eq!(eng.reader.searcher().num_docs(), 2);

    // 删除 blog:shared，不应影响 doc:shared
    eng.delete_documents(&[doc_uid("blog", "shared")]).expect("delete");
    assert_eq!(eng.reader.searcher().num_docs(), 1);
    let remaining = eng.query("Doc", None, 10).expect("query");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].kind, "doc");
  }

  #[test]
  fn replace_all_overwrites_previous_contents() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eng = open_engine(tmp.path());
    eng.replace_all(sample_docs()).expect("first write");
    assert_eq!(eng.reader.searcher().num_docs(), 2);

    // 用 1 个新文档全量替换；旧 2 个应消失。
    eng
      .replace_all(vec![IndexedDocument {
        kind: "blog".to_string(),
        ref_id: "only".to_string(),
        title: "Only".to_string(),
        body: "single doc".to_string(),
        url: "/blog/only".to_string(),
        created_at: String::new(),
      }])
      .expect("replace");
    assert_eq!(eng.reader.searcher().num_docs(), 1);
    let hits = eng.query("Only", None, 10).expect("query");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].ref_id, "only");
  }
}
