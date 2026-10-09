//! 搜索 server function:供前端调用。

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// 搜索命中结果(前后端共享)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchHit {
  pub kind: String,
  pub ref_id: String,
  pub title: String,
  pub snippet: String,
  pub url: String,
  pub created_at: String,
  pub score: f32,
}

/// 搜索响应(命中 + 总数 + 查询用时毫秒)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SearchResponse {
  pub hits: Vec<SearchHit>,
  pub total: usize,
  pub elapsed_ms: u64,
}

pub const MAX_LIMIT: u32 = 50;
pub const DEFAULT_LIMIT: u32 = 20;

/// 校验/规整 limit 参数。
pub fn clamp_limit(limit: Option<u32>) -> u32 {
  match limit {
    Some(0) => DEFAULT_LIMIT,
    Some(n) if n <= MAX_LIMIT => n,
    Some(_) => MAX_LIMIT,
    None => DEFAULT_LIMIT,
  }
}

/// 校验/规整 kind 参数:仅接受 "blog" / "doc" / "topic" / "case",其他返回 None。
pub fn normalize_kind(raw: Option<String>) -> Option<String> {
  let raw = raw?;
  let trimmed = raw.trim();
  match trimmed {
    "blog" | "doc" | "topic" | "case" => Some(trimmed.to_string()),
    _ => None,
  }
}

/// 全站搜索接口。
///
/// Phase 7.5 debug 习惯（详见 `docs/DEVELOPER.md §3.5`）：用
/// `#[tracing::instrument]` 自动记录入口 + 退出 + elapsed；`q` 用 `q_len`
/// 字段避免把整段查询文本进日志（含搜索词的请求 PII 风险）；
/// `kind` / `limit` 是元数据，直接进字段；返回 hit 数通过 `ret` 自动拿到。
#[cfg_attr(
  feature = "server",
  tracing::instrument(
    name = "server::search_query",
    level = "info",
    skip(q),
    fields(q_len = q.len(), kind = ?kind, limit = ?limit, hits = tracing::field::Empty),
    err
  )
)]
#[post("/api/search/query")]
pub async fn search_query(
  q: String,
  kind: Option<String>,
  limit: Option<u32>,
) -> Result<SearchResponse, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use std::time::Instant;
    let started = Instant::now();
    let limit = clamp_limit(limit) as usize;
    let kind = normalize_kind(kind);
    let engine = crate::engine::get_or_build().await.map_err(ServerFnError::new)?;
    let results =
      engine.query(&q, kind.as_deref(), limit).map_err(|e| ServerFnError::new(e.to_string()))?;
    let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let hits: Vec<SearchHit> = results
      .into_iter()
      .map(|h| SearchHit {
        kind: h.kind,
        ref_id: h.ref_id,
        title: h.title,
        snippet: h.snippet,
        url: h.url,
        created_at: h.created_at,
        score: h.score,
      })
      .collect();
    tracing::Span::current().record("hits", hits.len());
    Ok(SearchResponse { total: hits.len(), hits, elapsed_ms })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (q, kind, limit);
    Ok(SearchResponse::default())
  }
}

/// Reindex 结果（管理员调用 `search_reindex` 后的结构化报告）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReindexReport {
  /// `"incremental"` 或 `"full"`。
  pub mode: String,
  pub upserts: usize,
  pub deletes: usize,
  pub elapsed_ms: u64,
}

/// 重建索引（管理员）。Phase 7.3.3：默认走增量；`mode=full` 强制全量。
///
/// - `mode = None` 或 `"incremental"`：按 mtime + 动态来源差分增量 upsert/delete。
/// - `mode = "full"`：清空索引 + 重新写入所有文档（与旧行为一致）。
#[post("/api/search/reindex")]
pub async fn search_reindex(mode: Option<String>) -> Result<ReindexReport, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::session::require_admin;
    let _ = require_admin().await?;
    let mode_str = mode.as_deref().map(|s| s.trim()).unwrap_or("incremental");
    match mode_str {
      "full" => {
        use std::time::Instant;
        let started = Instant::now();
        let engine = crate::engine::rebuild().await.map_err(ServerFnError::new)?;
        let total = engine.reader.searcher().num_docs() as usize;
        let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        Ok(ReindexReport { mode: "full".to_string(), upserts: total, deletes: 0, elapsed_ms })
      }
      _ => {
        let r = crate::engine::reindex_incremental().await.map_err(ServerFnError::new)?;
        Ok(ReindexReport {
          mode: "incremental".to_string(),
          upserts: r.upserts,
          deletes: r.deletes,
          elapsed_ms: r.elapsed_ms,
        })
      }
    }
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = mode;
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clamp_limit_defaults_when_missing_or_zero() {
    assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
    assert_eq!(clamp_limit(Some(0)), DEFAULT_LIMIT);
  }

  #[test]
  fn clamp_limit_passes_through() {
    assert_eq!(clamp_limit(Some(5)), 5);
    assert_eq!(clamp_limit(Some(MAX_LIMIT)), MAX_LIMIT);
  }

  #[test]
  fn clamp_limit_caps_oversize() {
    assert_eq!(clamp_limit(Some(MAX_LIMIT + 1)), MAX_LIMIT);
    assert_eq!(clamp_limit(Some(u32::MAX)), MAX_LIMIT);
  }

  #[test]
  fn normalize_kind_accepts_known() {
    assert_eq!(normalize_kind(Some("blog".to_string())).as_deref(), Some("blog"));
    assert_eq!(normalize_kind(Some(" doc ".to_string())).as_deref(), Some("doc"));
    assert_eq!(normalize_kind(Some("topic".to_string())).as_deref(), Some("topic"));
    assert_eq!(normalize_kind(Some("case".to_string())).as_deref(), Some("case"));
  }

  #[test]
  fn normalize_kind_rejects_unknown() {
    assert!(normalize_kind(Some("user".to_string())).is_none());
    assert!(normalize_kind(Some("".to_string())).is_none());
    assert!(normalize_kind(None).is_none());
  }
}
