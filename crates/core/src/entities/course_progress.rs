use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 课程学习进度（lesson 粒度）
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "course_progress")]
pub struct Model {
  #[sea_orm(primary_key, auto_increment = false)]
  pub user_id: i32,
  #[sea_orm(primary_key, auto_increment = false)]
  pub course_slug: String,
  #[sea_orm(primary_key, auto_increment = false)]
  pub lesson_path: String,
  pub completed: bool,
  pub position_seconds: Option<i32>,
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
