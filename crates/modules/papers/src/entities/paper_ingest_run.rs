use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "paper_ingest_runs")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i64,
  /// 这次采集的 HF 日期
  pub run_on: Date,
  pub fetched: i32,
  pub inserted: i32,
  pub generated: i32,
  pub failed: i32,
  pub input_tokens: i64,
  pub output_tokens: i64,
  pub duration_ms: i64,
  pub started_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
