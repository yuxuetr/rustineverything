//! Phase 7.1：数据库 schema 迁移。
//!
//! ## 角色
//! 取代根目录 `init.sql`，由 SeaORM 的 [`MigratorTrait`] 在应用启动时
//! 自动跑：`crates/app/src/main.rs::init_pool` 之后调
//! [`Migrator::up`]`(&db, None)` 即可。
//!
//! ## 历史
//! `m20260527_000001_initial_schema` 把 init.sql 的全部 7 张表 + 索引
//! 落地到 SeaORM 的 schema builder：
//! - `users` / `comments` / `user_identities`
//! - `course_progress` / `annotations`
//! - `topics` / `topic_replies`
//!
//! 后续 schema 变更（增列 / 改类型 / 增表）都按
//! `m<YYYYMMDD>_<seq>_<slug>.rs` 命名追加。SeaORM 会在
//! `seaql_migrations` 表中记录已应用迁移，重复运行幂等。
//!
//! ## 测试
//! - 单测：[`tests::migrations_have_expected_names`] 确保 [`Migrator`] 与
//!   预期的迁移列表对齐。
//! - 集成测试（需 DATABASE_URL）：`cargo test -p rustineverything-migration -- --ignored`

pub use sea_orm_migration::prelude::*;

mod m20260527_000001_initial_schema;
mod m20260530_000002_moderation_queue;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(m20260527_000001_initial_schema::Migration),
      Box::new(m20260530_000002_moderation_queue::Migration),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn migrations_have_expected_names() {
    let names: Vec<String> = Migrator::migrations()
      .iter()
      .map(|m| m.name().to_string())
      .collect();
    assert_eq!(
      names,
      vec![
        "m20260527_000001_initial_schema".to_string(),
        "m20260530_000002_moderation_queue".to_string(),
      ]
    );
  }

  #[test]
  fn migrator_can_be_constructed() {
    // 单测仅验证类型可正常构造、API 表面稳定；真实数据库 round-trip
    // 留给集成测试（需配置 DATABASE_URL）。
    let _ = Migrator;
  }
}
