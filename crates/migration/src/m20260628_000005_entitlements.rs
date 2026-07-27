//! M4 付费地基：课程访问权益表 `entitlements`。
//!
//! 复合主键 `(user_id, course_slug)`；用户删除时级联清除其权益。
//! 详见 `crates/core/src/entities/entitlement.rs` 与 docs/SITE_REDESIGN_SPEC.md §5.3。

use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260628_000005_entitlements"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Entitlements::Table)
          .if_not_exists()
          .col(integer(Entitlements::UserId).not_null())
          .col(string_len(Entitlements::CourseSlug, 128).not_null())
          .col(string_len(Entitlements::Source, 32).not_null().default("admin_grant".to_string()))
          .col(
            timestamp_with_time_zone(Entitlements::GrantedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .primary_key(Index::create().col(Entitlements::UserId).col(Entitlements::CourseSlug))
          .foreign_key(
            ForeignKey::create()
              .name("fk_entitlements_user")
              .from(Entitlements::Table, Entitlements::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    // 按用户查「我拥有的课程」主路径
    manager
      .create_index(
        Index::create()
          .name("idx_entitlements_user")
          .table(Entitlements::Table)
          .col(Entitlements::UserId)
          .to_owned(),
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.drop_table(Table::drop().table(Entitlements::Table).if_exists().to_owned()).await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Entitlements {
  Table,
  UserId,
  CourseSlug,
  Source,
  GrantedAt,
}

#[derive(DeriveIden)]
enum Users {
  Table,
  Id,
}
