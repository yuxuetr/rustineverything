//! 页面用的 server fn。只读已经生成好解读（`status = done`）的论文。

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// 列表页每次最多取多少篇。
#[cfg(feature = "server")]
const LIST_LIMIT: u64 = 200;

/// 列表里的一篇。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaperCard {
  pub arxiv_id: String,
  pub title: String,
  /// HF 日期，`YYYY-MM-DD`
  pub date: String,
  pub hf_upvotes: i32,
}

/// 最近的论文，按 HF 日期倒序、同一天按 upvotes 倒序。
#[post("/api/papers/list")]
pub async fn list_papers() -> Result<Vec<PaperCard>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use crate::entities::paper;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
    let db =
      app_core::db::get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))?;
    let rows = paper::Entity::find()
      .filter(paper::Column::Status.eq("done"))
      .order_by_desc(paper::Column::FirstSeenOn)
      .order_by_desc(paper::Column::HfUpvotes)
      .limit(LIST_LIMIT)
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(
      rows
        .into_iter()
        .map(|p| PaperCard {
          arxiv_id: p.arxiv_id,
          title: p.title,
          date: p.first_seen_on.to_string(),
          hf_upvotes: p.hf_upvotes,
        })
        .collect(),
    )
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}
