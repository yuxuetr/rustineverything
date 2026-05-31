//! Phase 8.2：删除 `user_identities.access_token` 列。
//!
//! 原列在 Phase 7.4 引入用于「OAuth token 转发」预想场景，但实际从未被读取
//! （[`AuthService::sync_user_to_db`] 写入后再没有出库 / 解密路径）。继续保留它意味着：
//! - 持久化加密的第三方 token，泄露面与价值不成比例
//! - 应用代码里 dead-stored 状态难以审计
//!
//! 决策：YAGNI 删列。未来真要 token 转发时再走「单独的、用户主动授权过的临时
//! 缓存表」，避免与登录会话耦合。
//!
//! 注意：本 migration 只动 SCHEMA；entity 与 `sync_user_to_db` 的对应字段会
//! 在同 commit 中一起删除，确保上线后 ORM 不再 INSERT 该列。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260601_000003_drop_access_token"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(UserIdentities::Table)
          .drop_column(UserIdentities::AccessToken)
          .to_owned(),
      )
      .await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    // 回滚：重新加列（text null）。注意此时无法恢复历史数据。
    manager
      .alter_table(
        Table::alter()
          .table(UserIdentities::Table)
          .add_column(ColumnDef::new(UserIdentities::AccessToken).text().null())
          .to_owned(),
      )
      .await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum UserIdentities {
  Table,
  AccessToken,
}
