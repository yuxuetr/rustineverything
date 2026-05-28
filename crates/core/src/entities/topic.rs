use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "topics")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i32,
  pub title: String,
  pub tag: String,
  #[sea_orm(column_type = "Text")]
  pub content: String,
  pub user_id: i32,
  pub reply_count: i32,
  pub last_reply_at: Option<DateTimeWithTimeZone>,
  pub ref_kind: Option<String>,
  #[sea_orm(column_type = "Text", nullable)]
  pub ref_path: Option<String>,
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
  #[sea_orm(has_many = "super::topic_reply::Entity")]
  Replies,
}

impl Related<super::user::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::User.def()
  }
}

impl Related<super::topic_reply::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Replies.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
