//! Phase 4.5：审核队列表。
//!
//! 每条 Flag 决定入队 → admin 复核 → approved / rejected。
//! 详见 `crates/core/src/entities/moderation_queue.rs` 与 docs/MODERATION_SPEC.md §3.9。

use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260530_000002_moderation_queue"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(ModerationQueue::Table)
          .if_not_exists()
          .col(big_integer(ModerationQueue::Id).not_null().auto_increment().primary_key())
          .col(string_len(ModerationQueue::Kind, 32).not_null())
          .col(big_integer_null(ModerationQueue::RefId))
          .col(text(ModerationQueue::RefPath).not_null())
          .col(integer_null(ModerationQueue::UserId))
          .col(text(ModerationQueue::Content).not_null())
          .col(text_null(ModerationQueue::Images))
          .col(float(ModerationQueue::Score).not_null().default(0.0))
          .col(string_len(ModerationQueue::Label, 16).not_null())
          .col(text(ModerationQueue::Reason).not_null().default("".to_string()))
          .col(string_len(ModerationQueue::Status, 16).not_null().default("pending".to_string()))
          .col(integer_null(ModerationQueue::ReviewerUserId))
          .col(text_null(ModerationQueue::ReviewerNote))
          .col(
            timestamp_with_time_zone(ModerationQueue::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .col(timestamp_with_time_zone_null(ModerationQueue::ReviewedAt))
          // 用户删除时把 user_id / reviewer_user_id 置 NULL，保留审核记录
          .foreign_key(
            ForeignKey::create()
              .name("fk_moderation_queue_user")
              .from(ModerationQueue::Table, ModerationQueue::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::SetNull),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_moderation_queue_reviewer")
              .from(ModerationQueue::Table, ModerationQueue::ReviewerUserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::SetNull),
          )
          .to_owned(),
      )
      .await?;

    // 列表查询主路径：pending 状态按时间倒序
    manager
      .create_index(
        Index::create()
          .name("idx_moderation_queue_status")
          .table(ModerationQueue::Table)
          .col(ModerationQueue::Status)
          .col((ModerationQueue::CreatedAt, IndexOrder::Desc))
          .to_owned(),
      )
      .await?;
    // 按业务类型过滤（admin 切 Tab）
    manager
      .create_index(
        Index::create()
          .name("idx_moderation_queue_kind")
          .table(ModerationQueue::Table)
          .col(ModerationQueue::Kind)
          .to_owned(),
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.drop_table(Table::drop().table(ModerationQueue::Table).if_exists().to_owned()).await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum ModerationQueue {
  Table,
  Id,
  Kind,
  RefId,
  RefPath,
  UserId,
  Content,
  Images,
  Score,
  Label,
  Reason,
  Status,
  ReviewerUserId,
  ReviewerNote,
  CreatedAt,
  ReviewedAt,
}

#[derive(DeriveIden)]
enum Users {
  Table,
  Id,
}
