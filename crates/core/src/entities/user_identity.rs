use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_identities")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i32,
  pub user_id: i32,
  pub provider: String,     // "github", "google", "wechat", "qq"
  pub provider_uid: String, // The UID from the social platform
  // Phase 8.2：原 `access_token: Option<String>` 已删（migration 0003）。
  // 当时存的是 AES-GCM 加密的 OAuth token，但宿主从未解密 / 转发它 ——
  // dead-stored 状态。未来真要做 token 转发请走单独临时表，不要复用本表。
  pub refresh_token: Option<String>,
  pub created_at: DateTimeWithTimeZone,
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
