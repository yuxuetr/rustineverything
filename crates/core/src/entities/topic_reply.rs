use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "topic_replies")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i32,
  pub topic_id: i32,
  pub user_id: i32,
  #[sea_orm(column_type = "Text")]
  pub content: String,
  pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
  #[sea_orm(
    belongs_to = "super::topic::Entity",
    from = "Column::TopicId",
    to = "super::topic::Column::Id"
  )]
  Topic,
  #[sea_orm(
    belongs_to = "super::user::Entity",
    from = "Column::UserId",
    to = "super::user::Column::Id"
  )]
  User,
}

impl Related<super::topic::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Topic.def()
  }
}

impl Related<super::user::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::User.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
