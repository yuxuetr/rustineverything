//! 基于 tantivy 的内嵌全文检索引擎(RAM 索引 + 中文 jieba 分词)。
//!
//! 设计要点:
//! - 全局单例,首次查询时 lazy 构建索引;`reindex()` 强制重建。
//! - 使用 RAMDirectory,简化部署(零文件路径管理),进程重启重新索引。
//! - 多字段 schema(kind / ref_id / title / body / url / created_at)。
//! - 中文使用 jieba,英文/数字走 lowercase,默认 OR 查询。

use std::sync::{Arc, Mutex, OnceLock};

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, TextFieldIndexing, TextOptions, FAST, STORED, STRING};
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{doc, Index, IndexReader, ReloadPolicy, TantivyDocument};

use crate::indexer::{collect_documents, IndexedDocument};
use crate::text::truncate_chars;

pub const TOKENIZER_NAME: &str = "jieba";

/// 命中结果 (与 server 层对齐)。
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

    // jieba 索引选项:用于 title/body
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer(TOKENIZER_NAME)
        .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();

    // STRING 选项: 不分词、用于 kind 过滤
    let kind = builder.add_text_field("kind", STRING | STORED | FAST);
    let ref_id = builder.add_text_field("ref_id", STRING | STORED);
    let title = builder.add_text_field("title", text_options.clone());
    let body = builder.add_text_field("body", text_options);
    let url = builder.add_text_field("url", STRING | STORED);
    let created_at = builder.add_text_field("created_at", STRING | STORED);

    let schema = builder.build();
    let fields = SearchFields {
        kind,
        ref_id,
        title,
        body,
        url,
        created_at,
    };
    (schema, fields)
}

/// 引擎实例:持有 Index + Reader + 字段引用。
pub struct SearchEngine {
    pub index: Index,
    pub reader: IndexReader,
    pub fields: SearchFields,
}

impl SearchEngine {
    fn build_with_documents(docs: Vec<IndexedDocument>) -> tantivy::Result<Self> {
        let (schema, fields) = build_schema();
        let index = Index::create_in_ram(schema);
        // 注册 jieba 分词器
        let analyzer: TextAnalyzer =
            tantivy_jieba::JiebaTokenizer::default().into();
        index.tokenizers().register(TOKENIZER_NAME, analyzer);

        // 写入文档
        let mut writer = index.writer(50_000_000)?;
        for d in docs {
            writer.add_document(doc!(
                fields.kind => d.kind,
                fields.ref_id => d.ref_id,
                fields.title => d.title,
                fields.body => d.body,
                fields.url => d.url,
                fields.created_at => d.created_at,
            ))?;
        }
        writer.commit()?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(Self {
            index,
            reader,
            fields,
        })
    }

    /// 查询:返回按 BM25 分数排序的结果。
    /// `kind_filter`:Some("blog") 仅命中博客,None 不过滤。
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
        let mut parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.title, self.fields.body],
        );
        // title 加权,默认 OR
        parser.set_field_boost(self.fields.title, 3.0);

        // 拼接 kind 过滤
        let final_query = match kind_filter {
            Some(k) if !k.is_empty() => format!("({}) AND kind:{}", escape_query(q), k),
            _ => q.to_string(),
        };

        let query = parser
            .parse_query(&final_query)
            .or_else(|_| parser.parse_query(&parser_safe(q)))?;
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

        // 简化的片段生成:在 body 里找第一个查询关键字位置,截 200 字符上下文
        let snippet = make_snippet(&body, query_text, 200);
        EngineHit {
            kind,
            ref_id,
            title,
            snippet,
            url,
            created_at,
            score,
        }
    }
}

/// 防御性查询转义:保留 ASCII 字母数字、空格、引号、中文等。
fn escape_query(q: &str) -> String {
    q.chars()
        .map(|c| match c {
            '+' | '-' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '~' | '*' | '?'
            | ':' | '\\' | '/' => ' ',
            other => other,
        })
        .collect()
}

/// 进一步降级:如果 parse 仍失败,只保留 alphanumeric + 中文,避免崩溃。
fn parser_safe(q: &str) -> String {
    q.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || (*c as u32) > 0x7F)
        .collect()
}

fn first_text(doc: &TantivyDocument, field: Field) -> String {
    use tantivy::schema::Value;
    doc.get_first(field)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

/// 简单 snippet:命中位置前后取 max_chars/2 的窗口,过短则取开头。
fn make_snippet(body: &str, query: &str, max_chars: usize) -> String {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return String::new();
    }
    // 先尝试最长查询 token
    let token = query
        .split_whitespace()
        .max_by_key(|t| t.chars().count())
        .unwrap_or(query);
    let lower_body = trimmed_body.to_lowercase();
    let lower_token = token.to_lowercase();
    let half = max_chars / 2;

    if let Some(byte_pos) = lower_body.find(&lower_token) {
        // 在原 body 中,定位到字符位置
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

/// 获取当前引擎,若不存在则构建。
pub async fn get_or_build() -> Result<Arc<SearchEngine>, String> {
    {
        let guard = engine_slot()
            .lock()
            .map_err(|e| format!("search lock poisoned: {}", e))?;
        if let Some(e) = guard.as_ref() {
            return Ok(e.clone());
        }
    }
    rebuild().await
}

/// 强制重建引擎。
pub async fn rebuild() -> Result<Arc<SearchEngine>, String> {
    let docs = collect_documents().await?;
    let count = docs.len();
    let engine = SearchEngine::build_with_documents(docs).map_err(|e| e.to_string())?;
    let arc = Arc::new(engine);
    {
        let mut guard = engine_slot()
            .lock()
            .map_err(|e| format!("search lock poisoned: {}", e))?;
        *guard = Some(arc.clone());
    }
    println!("[Search] index rebuilt with {} documents", count);
    Ok(arc)
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
        // 应该包含 Rust
        assert!(snippet.contains("Rust"));
    }

    #[test]
    fn snippet_no_match_returns_prefix() {
        let body = "no match here, just some content about cooking";
        let snippet = make_snippet(body, "Rust", 30);
        // 没命中,返回开头
        assert!(snippet.starts_with("no match"));
    }

    #[test]
    fn snippet_empty_body() {
        assert_eq!(make_snippet("", "Rust", 50), "");
    }

    #[test]
    fn build_schema_has_six_fields() {
        let (schema, fields) = build_schema();
        // 每个字段都能从 schema 取到
        for f in [
            fields.kind,
            fields.ref_id,
            fields.title,
            fields.body,
            fields.url,
            fields.created_at,
        ] {
            assert!(schema.get_field_name(f).len() > 0);
        }
    }

    #[test]
    fn engine_query_basic() {
        let docs = vec![
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
        ];
        let eng = SearchEngine::build_with_documents(docs).expect("build");
        let hits = eng.query("Rust", None, 10).expect("query");
        assert!(!hits.is_empty());
        // title 加权,Hello Rust 应在前
        assert_eq!(hits[0].kind, "blog");
        assert!(hits[0].score >= hits.last().unwrap().score);
    }

    #[test]
    fn engine_query_kind_filter() {
        let docs = vec![
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
        ];
        let eng = SearchEngine::build_with_documents(docs).expect("build");
        let hits = eng.query("rust", Some("blog"), 10).expect("query");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "blog");
    }

    #[test]
    fn engine_query_chinese() {
        let docs = vec![IndexedDocument {
            kind: "blog".to_string(),
            ref_id: "x".to_string(),
            title: "你好 Rust".to_string(),
            body: "这是一篇关于 Rust 编程语言的中文博客文章".to_string(),
            url: "/blog/x".to_string(),
            created_at: String::new(),
        }];
        let eng = SearchEngine::build_with_documents(docs).expect("build");
        let hits = eng.query("中文", None, 10).expect("query");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn engine_query_empty_returns_empty() {
        let docs = vec![IndexedDocument {
            kind: "blog".to_string(),
            ref_id: "x".to_string(),
            title: "T".to_string(),
            body: "B".to_string(),
            url: "/x".to_string(),
            created_at: String::new(),
        }];
        let eng = SearchEngine::build_with_documents(docs).expect("build");
        let hits = eng.query("   ", None, 10).expect("query");
        assert!(hits.is_empty());
    }

    #[test]
    fn engine_handles_special_chars_gracefully() {
        let docs = vec![IndexedDocument {
            kind: "blog".to_string(),
            ref_id: "x".to_string(),
            title: "Rust".to_string(),
            body: "content".to_string(),
            url: "/x".to_string(),
            created_at: String::new(),
        }];
        let eng = SearchEngine::build_with_documents(docs).expect("build");
        // 含 tantivy 特殊字符的查询不能 panic
        let r = eng.query("rust!@#?:", None, 10);
        assert!(r.is_ok());
    }
}
