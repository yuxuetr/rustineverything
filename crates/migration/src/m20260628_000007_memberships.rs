//! M6：Pro 订阅会员表 `memberships`。
//!
//! 详见 `crates/core/src/entities/membership.rs` 与 docs/SITE_REDESIGN_SPEC.md §5.6 D。

use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260628_000007_memberships"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Memberships::Table)
          .if_not_exists()
          .col(integer(Memberships::UserId).not_null().primary_key())
          .col(string_len(Memberships::Tier, 16).not_null().default("pro".to_string()))
          .col(timestamp_with_time_zone(Memberships::ExpiresAt).not_null())
          .col(string_len(Memberships::Source, 32).not_null().default("admin_grant".to_string()))
          .col(
            timestamp_with_time_zone(Memberships::UpdatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_memberships_user")
              .from(Memberships::Table, Memberships::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.drop_table(Table::drop().table(Memberships::Table).if_exists().to_owned()).await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Memberships {
  Table,
  UserId,
  Tier,
  ExpiresAt,
  Source,
  UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
  Table,
  Id,
}
