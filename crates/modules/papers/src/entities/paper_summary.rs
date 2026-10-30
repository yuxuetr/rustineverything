use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "paper_summaries")]
pub struct Model {
  #[sea_orm(primary_key, auto_increment = false)]
  pub arxiv_id: String,
  /// `en` / `zh`，与 `app_core::i18n::Language` 的代码一致
  #[sea_orm(primary_key, auto_increment = false)]
  pub lang: String,
  /// `PaperDigest` 的 JSON 文本
  pub digest: String,
  pub model: String,
  pub prompt_version: i32,
  /// 原文里没找到的数字，JSON 数组文本
  pub unverified_numbers: String,
  pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
  #[sea_orm(
    belongs_to = "super::paper::Entity",
    from = "Column::ArxivId",
    to = "super::paper::Column::ArxivId",
    on_delete = "Cascade"
  )]
  Paper,
}

impl Related<super::paper::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Paper.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
