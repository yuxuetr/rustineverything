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
use tantivy::{doc, Index, IndexReader, ReloadPolicy, TantivyDocument, TantivyError};

use crate::indexer::{collect_documents, IndexedDocument};
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

  let kind = builder.add_text_field("kind", STRING | STORED | FAST);
  let ref_id = builder.add_text_field("ref_id", STRING | STORED);
  let title = builder.add_text_field("title", text_options.clone());
  let body = builder.add_text_field("body", text_options);
  let url = builder.add_text_field("url", STRING | STORED);
  let created_at = builder.add_text_field("created_at", STRING | STORED);

  let schema = builder.build();
  let fields = SearchFields { kind, ref_id, title, body, url, created_at };
  (schema, fields)
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
    let mut writer =
      self.index.writer(50_000_000).map_err(|e| format!("search: writer init failed: {}", e))?;
    writer
      .delete_all_documents()
      .map_err(|e| format!("search: delete_all_documents failed: {}", e))?;

    let count = docs.len();
    for d in docs {
      writer
        .add_document(doc!(
            self.fields.kind => d.kind,
            self.fields.ref_id => d.ref_id,
            self.fields.title => d.title,
            self.fields.body => d.body,
            self.fields.url => d.url,
            self.fields.created_at => d.created_at,
        ))
        .map_err(|e| format!("search: add_document failed: {}", e))?;
    }
    writer.commit().map_err(|e| format!("search: commit failed: {}", e))?;
    self.reader.reload().map_err(|e| format!("search: reader reload failed: {}", e))?;
    Ok(count)
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
/// - 目录为空 / 首次启动 → 全量扫描 [`collect_documents`] 填充。
/// - schema 不匹配 → 自动清空重建（[`SearchEngine::open_or_create`]）后填充。
pub async fn init_or_load(dir: &Path) -> Result<Arc<SearchEngine>, String> {
  let (engine, was_freshly_created) = SearchEngine::open_or_create(dir)?;
  let engine = Arc::new(engine);

  if was_freshly_created {
    let docs = collect_documents().await?;
    let count = engine.replace_all(docs)?;
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

/// 强制全量重建当前引擎的索引（保持持久化目录）。
pub async fn rebuild() -> Result<Arc<SearchEngine>, String> {
  let engine = match engine_slot().lock() {
    Ok(g) => g.as_ref().cloned(),
    Err(e) => return Err(format!("search lock poisoned: {}", e)),
  };
  let engine = match engine {
    Some(e) => e,
    None => return init_or_load(&resolve_index_dir()).await,
  };

  let docs = collect_documents().await?;
  let count = engine.replace_all(docs)?;
  tracing::info!(documents = count, dir = %engine.dir.display(), "search: index rebuilt");
  Ok(engine)
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
