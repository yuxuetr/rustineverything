//! M5a：课程购买订单表 `orders`。
//!
//! 详见 `crates/core/src/entities/order.rs` 与 docs/PAYMENT_SPEC.md §2。

use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260628_000006_orders"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Orders::Table)
          .if_not_exists()
          .col(big_integer(Orders::Id).not_null().auto_increment().primary_key())
          .col(string_len(Orders::OutTradeNo, 64).not_null().unique_key())
          .col(integer(Orders::UserId).not_null())
          .col(string_len(Orders::CourseSlug, 128).not_null())
          .col(string_len(Orders::Provider, 16).not_null())
          .col(string_len(Orders::Scene, 16).not_null())
          .col(big_integer(Orders::Amount).not_null())
          .col(string_len(Orders::Currency, 8).not_null().default("CNY".to_string()))
          .col(string_len(Orders::Status, 16).not_null().default("pending".to_string()))
          .col(string_len_null(Orders::ProviderTxn, 64))
          .col(
            timestamp_with_time_zone(Orders::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .col(timestamp_with_time_zone_null(Orders::PaidAt))
          .foreign_key(
            ForeignKey::create()
              .name("fk_orders_user")
              .from(Orders::Table, Orders::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create().name("idx_orders_user").table(Orders::Table).col(Orders::UserId).to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_orders_status")
          .table(Orders::Table)
          .col(Orders::Status)
          .col((Orders::CreatedAt, IndexOrder::Desc))
          .to_owned(),
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.drop_table(Table::drop().table(Orders::Table).if_exists().to_owned()).await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Orders {
  Table,
  Id,
  OutTradeNo,
  UserId,
  CourseSlug,
  Provider,
  Scene,
  Amount,
  Currency,
  Status,
  ProviderTxn,
  CreatedAt,
  PaidAt,
}

#[derive(DeriveIden)]
enum Users {
  Table,
  Id,
}
