use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i32,
  pub nickname: String,
  pub avatar_url: Option<String>,
  pub role: String, // "admin", "member", "guest"
  /// S4（风险 R1）：JWT 撤销版本。bump = 吊销该用户全部已签发 JWT。
  #[serde(default)]
  pub token_version: i32,
  pub created_at: DateTimeWithTimeZone,
  pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
  #[sea_orm(has_many = "super::user_identity::Entity")]
  UserIdentities,
}

impl Related<super::user_identity::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::UserIdentities.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
