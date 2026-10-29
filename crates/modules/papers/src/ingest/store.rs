//! 把 HF 列表写进 `papers`：按 arXiv id 去重，再次出现只更新出现日期与热度。

use chrono::NaiveDate;
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{
  ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
};

use super::hf::HfPaper;
use crate::entities::paper;

/// 写入某一天的列表，返回新增的篇数。
///
/// 已有的论文只改 `last_seen_on`（取较晚的日期）、`first_seen_on`（取较早的，
/// 补导更早的日期时用）、`hf_upvotes` 和 `updated_at`；解读与状态不动。
pub async fn upsert_day(
  db: &DatabaseConnection,
  date: NaiveDate,
  papers: &[HfPaper],
) -> Result<usize, DbErr> {
  if papers.is_empty() {
    return Ok(0);
  }
  let ids: Vec<&str> = papers.iter().map(|p| p.arxiv_id.as_str()).collect();
  let existing: Vec<String> = paper::Entity::find()
    .select_only()
    .column(paper::Column::ArxivId)
    .filter(paper::Column::ArxivId.is_in(ids))
    .into_tuple()
    .all(db)
    .await?;

  let now = chrono::Utc::now().fixed_offset();
  let rows = papers.iter().map(|p| paper::ActiveModel {
    arxiv_id: Set(p.arxiv_id.clone()),
    title: Set(p.title.clone()),
    r#abstract: Set(p.summary.clone()),
    authors: Set(serde_json::to_string(&p.authors).unwrap_or_else(|_| "[]".into())),
    arxiv_version: Set(None),
    published_at: Set(p.published_at),
    first_seen_on: Set(date),
    last_seen_on: Set(date),
    hf_upvotes: Set(p.upvotes),
    github_repo: Set(p.github_repo.clone()),
    has_html: Set(None),
    status: Set("pending".into()),
    error: Set(None),
    created_at: Set(now),
    updated_at: Set(now),
  });
  let on_conflict = OnConflict::column(paper::Column::ArxivId)
    .value(
      paper::Column::LastSeenOn,
      Expr::cust("GREATEST(papers.last_seen_on, EXCLUDED.last_seen_on)"),
    )
    .value(
      paper::Column::FirstSeenOn,
      Expr::cust("LEAST(papers.first_seen_on, EXCLUDED.first_seen_on)"),
    )
    .value(paper::Column::HfUpvotes, Expr::cust("GREATEST(papers.hf_upvotes, EXCLUDED.hf_upvotes)"))
    .update_column(paper::Column::UpdatedAt)
    .to_owned();
  paper::Entity::insert_many(rows).on_conflict(on_conflict).exec_without_returning(db).await?;

  Ok(papers.iter().filter(|p| !existing.contains(&p.arxiv_id)).count())
}
