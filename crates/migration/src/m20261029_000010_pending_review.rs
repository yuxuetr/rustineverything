//! 审核未决的内容先不公开：`comments` / `topics` / `topic_replies` / `annotations`
//! 加 `pending_review`。
//!
//! 审核结果为 Flag（模型拿不准，或审核调用失败）时照常写入业务行，但置 `true`；
//! 公开的读路径都过滤掉它，admin 在审核队列里通过后置回 `false`，拒绝则删除整行。
//! 默认 `false`：存量内容不受影响。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261029_000010_pending_review"
  }
}

const TABLES: [&str; 4] = ["comments", "topics", "topic_replies", "annotations"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for table in TABLES {
      manager
        .alter_table(
          Table::alter()
            .table(Alias::new(table))
            .add_column(
              ColumnDef::new(Alias::new("pending_review")).boolean().not_null().default(false),
            )
            .to_owned(),
        )
        .await?;
    }
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for table in TABLES {
      manager
        .alter_table(
          Table::alter()
            .table(Alias::new(table))
            .drop_column(Alias::new("pending_review"))
            .to_owned(),
        )
        .await?;
    }
    Ok(())
  }
}
