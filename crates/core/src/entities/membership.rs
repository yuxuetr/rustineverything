use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Pro 订阅会员（M6）。一个用户一条记录；`expires_at > now` 视为有效。
///
/// 有效会员可访问全部 `access_tier == "pro"` 的课程（`paid` 仍需单独购买）。
/// 详见 docs/SITE_REDESIGN_SPEC.md §5.6 D。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memberships")]
pub struct Model {
  #[sea_orm(primary_key, auto_increment = false)]
  pub user_id: i32,
  /// 会员层级，目前仅 `pro`。
  pub tier: String,
  pub expires_at: DateTimeWithTimeZone,
  /// 来源：purchase | admin_grant | …
  pub source: String,
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
