use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "papers")]
pub struct Model {
  /// 不带版本号的 arXiv id，例如 `2510.12345`
  #[sea_orm(primary_key, auto_increment = false)]
  pub arxiv_id: String,
  pub title: String,
  pub r#abstract: String,
  /// 作者名的 JSON 数组文本
  pub authors: String,
  pub arxiv_version: Option<i32>,
  pub published_at: DateTimeWithTimeZone,
  /// 第一次出现在 HF 列表里的日期，`/papers` 按它分组
  pub first_seen_on: Date,
  pub last_seen_on: Date,
  pub hf_upvotes: i32,
  pub github_repo: Option<String>,
  pub has_html: Option<bool>,
  /// `pending` / `done` / `failed` / `skipped`
  pub status: String,
  pub error: Option<String>,
  pub created_at: DateTimeWithTimeZone,
  pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
  #[sea_orm(has_many = "super::paper_summary::Entity")]
  Summary,
}

impl Related<super::paper_summary::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Summary.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
