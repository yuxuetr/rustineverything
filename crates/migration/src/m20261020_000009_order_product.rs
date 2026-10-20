//! 打赏（FEATURE_TIERS.md §6 DT1）：订单不再只卖课程。
//!
//! - `course_slug` 改名 `product_ref`：课程是课程 slug，打赏是打赏发生的页面路径。
//! - 新增 `product_kind`（`course` | `tip`），存量订单默认 `course`。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261020_000009_order_product"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Orders::Table)
          .rename_column(Orders::CourseSlug, Orders::ProductRef)
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(Orders::Table)
          .add_column(
            ColumnDef::new(Orders::ProductKind).string_len(16).not_null().default("course"),
          )
          .to_owned(),
      )
      .await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table(Orders::Table).drop_column(Orders::ProductKind).to_owned())
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(Orders::Table)
          .rename_column(Orders::ProductRef, Orders::CourseSlug)
          .to_owned(),
      )
      .await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Orders {
  Table,
  CourseSlug,
  ProductRef,
  ProductKind,
}
