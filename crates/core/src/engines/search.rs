//! [`SearchEngine`] 骨架（Phase 1C.4）。
//!
//! 完整实现（Tantivy 索引 / MmapDirectory / 增量更新）已在 `modules/search/`
//! 中存在；本文件仅提供「源注册」抽象，让模块可以自注册搜索源以替代当前
//! `indexer.rs` 中的硬编码 4 源。Phase 3.4 / 6 会迁移现有 4 源到这套接口。
//!
//! ## 接口
//! - [`SearchDocument`]：纯数据类型，与 `modules/search/src/indexer.rs::IndexedDocument`
//!   语义一致；保留在 core/engines 让所有模块都能依赖。
//! - [`SearchSource`] trait：模块实现该 trait 自注册。`collect()` 同步返回 docs
//!   （足够覆盖 blog / docs / cases 文件系统源）；后续如有 DB 源可改为 async。
//! - [`SearchEngine`]：保存所有源，提供 [`SearchEngine::collect_all`] 一次性
//!   收集（按 ModuleEngine.enabled_ids 过滤是上层调用的责任）。

use serde::{Deserialize, Serialize};

use super::{Engine, EngineContext};
use crate::error::AppResult;

/// 与 Tantivy 索引 schema 一一对应的纯数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchDocument {
    pub kind: String,
    pub ref_id: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub created_at: String,
}

/// 单个搜索数据源。每个模块（blog / docs / cases / forum / ...）实现一个
/// `SearchSource`，返回自家全部可索引文档。
pub trait SearchSource: Send + Sync {
    /// 唯一名（与 `ModuleSpec.id` 对齐，方便 ModuleEngine 过滤）。
    fn name(&self) -> &'static str;
    /// 收集本源的全部 documents。同步接口，足以覆盖文件系统源；DB 源
    /// 可在实现内 block_on 内部 async 调用，或后续重构为 async-trait。
    fn collect(&self) -> Vec<SearchDocument>;
}

/// SearchEngine：搜索源注册中心。
pub struct SearchEngine {
    sources: Vec<Box<dyn SearchSource>>,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn register<S: SearchSource + 'static>(&mut self, source: S) {
        self.sources.push(Box::new(source));
    }

    pub fn source_names(&self) -> Vec<&'static str> {
        self.sources.iter().map(|s| s.name()).collect()
    }

    /// 收集所有源的 documents。
    pub fn collect_all(&self) -> Vec<SearchDocument> {
        self.sources.iter().flat_map(|s| s.collect()).collect()
    }

    /// 仅采集名称在 `enabled` 列表中的源。常与 `ModuleEngine::enabled_ids`
    /// 配合使用，关闭的模块不会出现在搜索结果中。
    pub fn collect_filtered(&self, enabled: &[String]) -> Vec<SearchDocument> {
        self.sources
            .iter()
            .filter(|s| enabled.iter().any(|e| e == s.name()))
            .flat_map(|s| s.collect())
            .collect()
    }
}

impl Engine for SearchEngine {
    fn name(&self) -> &'static str {
        "search"
    }

    fn init(&mut self, _ctx: &EngineContext) -> AppResult<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(kind: &str, id: &str) -> SearchDocument {
        SearchDocument {
            kind: kind.to_string(),
            ref_id: id.to_string(),
            title: format!("{} - {}", kind, id),
            body: String::new(),
            url: format!("/{}/{}", kind, id),
            created_at: String::new(),
        }
    }

    struct BlogSource;
    impl SearchSource for BlogSource {
        fn name(&self) -> &'static str {
            "blog"
        }
        fn collect(&self) -> Vec<SearchDocument> {
            vec![doc("blog", "hello"), doc("blog", "world")]
        }
    }

    struct ForumSource;
    impl SearchSource for ForumSource {
        fn name(&self) -> &'static str {
            "forum"
        }
        fn collect(&self) -> Vec<SearchDocument> {
            vec![doc("topic", "1")]
        }
    }

    #[test]
    fn engine_name_is_search() {
        let e = SearchEngine::new();
        assert_eq!(<SearchEngine as Engine>::name(&e), "search");
    }

    #[test]
    fn collect_all_unions_sources() {
        let mut e = SearchEngine::new();
        e.register(BlogSource);
        e.register(ForumSource);
        let docs = e.collect_all();
        assert_eq!(docs.len(), 3);
        assert_eq!(e.source_names(), vec!["blog", "forum"]);
    }

    #[test]
    fn collect_filtered_skips_disabled() {
        let mut e = SearchEngine::new();
        e.register(BlogSource);
        e.register(ForumSource);
        let docs = e.collect_filtered(&["blog".to_string()]);
        assert_eq!(docs.len(), 2);
        assert!(docs.iter().all(|d| d.kind == "blog"));
    }

    #[test]
    fn collect_filtered_empty_returns_nothing() {
        let mut e = SearchEngine::new();
        e.register(BlogSource);
        let docs = e.collect_filtered(&[]);
        assert!(docs.is_empty());
    }
}
