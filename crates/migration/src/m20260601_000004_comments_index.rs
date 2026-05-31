//! Phase 8.4：`comments(blog_id, created_at DESC)` 复合索引。
//!
//! 命中场景：每篇博客的评论列表（按 blog_id 过滤、按时间倒序），是 hot path。
//! init.sql 初版只在 (blog_id) 上有索引；ORDER BY 仍要做 sort，10K+ 行规模下
//! 每查约 30–50 ms 都耗在排序上。把 created_at 也带进来 → DB 直接走 index scan，
//! 节省 sort + 把 EXPLAIN 中的 `Sort` 节点拿掉。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260601_000004_comments_index"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_index(
        Index::create()
          .name("idx_comments_blog_created")
          .table(Comments::Table)
          .col(Comments::BlogId)
          .col((Comments::CreatedAt, IndexOrder::Desc))
          // 安全幂等：本仓库的 dev / staging 可能已手动建过 → IF NOT EXISTS
          .if_not_exists()
          .to_owned(),
      )
      .await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_index(
        Index::drop().name("idx_comments_blog_created").table(Comments::Table).to_owned(),
      )
      .await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Comments {
  Table,
  BlogId,
  CreatedAt,
}
