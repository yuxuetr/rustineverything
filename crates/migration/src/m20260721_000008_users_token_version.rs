//! S4（风险 R1）：`users.token_version` — JWT 撤销基础。
//!
//! 背景：JWT 7 天有效且无撤销机制，用户被降级 / 封禁 / 删除后，旧 cookie
//! 里的 JWT 仍会被接受最长 7 天。引入每用户单调递增的 `token_version`：
//!
//! - `create_jwt` 把当前版本写入 claims（`tv` 字段）。
//! - 会话校验（`require_session_verified` / `require_admin`）回查 DB 版本，
//!   不一致即拒绝——bump 版本 = 立刻吊销该用户所有已签发 JWT。
//! - admin 修改用户角色时自动 bump（见 `admin_set_user_role`）。
//!
//! 向后兼容：默认 0；旧 JWT 无 `tv` 字段时 serde default 也是 0，二者相等
//! → 存量登录用户不受影响，直到其版本第一次被 bump。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260721_000008_users_token_version"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Users::Table)
          .add_column(ColumnDef::new(Users::TokenVersion).integer().not_null().default(0))
          .to_owned(),
      )
      .await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table(Users::Table).drop_column(Users::TokenVersion).to_owned())
      .await?;
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Users {
  Table,
  TokenVersion,
}
