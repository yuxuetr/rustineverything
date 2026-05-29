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
    Ok(SearchResponse { total: hits.len(), hits, elapsed_ms })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (q, kind, limit);
    Ok(SearchResponse::default())
  }
}

/// 强制重建索引(管理员)。
#[post("/api/search/reindex")]
pub async fn search_reindex() -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::session::require_admin;
    let _ = require_admin()?;
    crate::engine::rebuild().await.map_err(ServerFnError::new)?;
    Ok("索引已重建".to_string())
  }
  #[cfg(not(feature = "server"))]
  {
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
