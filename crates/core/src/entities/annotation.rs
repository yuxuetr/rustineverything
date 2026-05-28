use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 用户对正文片段的标注
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "annotations")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i64,
  pub user_id: i32,
  /// 'course' | 'doc' | 'blog'
  pub resource_kind: String,
  /// 叶子页路径，如 'rust-basics/01-fundamentals/01-what-is-rust'
  pub resource_path: String,
  /// Markdown 顶层块 id，如 'b1'
  pub block_id: String,
  pub start_offset: i32,
  pub end_offset: i32,
  #[sea_orm(column_type = "Text")]
  pub exact_text: String,
  #[sea_orm(column_type = "Text", nullable)]
  pub prefix_text: Option<String>,
  #[sea_orm(column_type = "Text", nullable)]
  pub suffix_text: Option<String>,
  /// yellow|green|blue|pink|purple|underline|wavy|strikethrough
  pub style: String,
  #[sea_orm(column_type = "Text", nullable)]
  pub note: Option<String>,
  /// 'private' | 'course-public' | 'doc-public' | 'public'（v1 仅 private）
  pub visibility: String,
  pub created_at: DateTimeWithTimeZone,
  pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
  #[sea_orm(
    belongs_to = "super::user::Entity",
    from = "Column::UserId",
    to = "super::user::Column::Id"
  )]
  User,
}

impl Related<super::user::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::User.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
