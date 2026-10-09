use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 课程访问权益：用户 ↔ 课程的拥有关系（M4 付费地基）。
///
/// 复合主键 `(user_id, course_slug)`：一个用户对一门课至多一条权益。
/// `source` 记录来源：`purchase | membership | coupon | admin_grant`。
/// 详见 docs/SITE_REDESIGN_SPEC.md §5.3。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entitlements")]
pub struct Model {
  #[sea_orm(primary_key, auto_increment = false)]
  pub user_id: i32,
  #[sea_orm(primary_key, auto_increment = false)]
  pub course_slug: String,
  pub source: String,
  pub granted_at: DateTimeWithTimeZone,
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
