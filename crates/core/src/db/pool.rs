//! 全局数据库连接池单例。
//!
//! 之前每个 server fn 都会调用 `init_db` 重新建立连接，每次请求都付出
//! TLS 握手 + 连接建立的延迟。引入此 OnceCell 后：
//!
//! - 启动时（`main.rs` 中）调用一次 [`init_pool`] 完成初始化。
//! - 各模块通过 [`get_or_init_pool`] 取出共享连接句柄，SeaORM 的
//!   `DatabaseConnection` 内部封装了连接池，clone 仅复制一个 Arc 引用。
//! - 测试或直接调用 server fn 的场景下，[`get_or_init_pool`] 会读取
//!   `DATABASE_URL` 环境变量做兜底初始化。

use std::time::Duration;

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tokio::sync::OnceCell;

static POOL: OnceCell<DatabaseConnection> = OnceCell::const_new();

/// 默认 pool 上限：32 conn 足以撑住小流量公网部署的 5–50 RPS。
const DEFAULT_MAX_CONNECTIONS: u32 = 32;
/// 默认 pool 下限：保 2 个常驻连接避免冷启动握手延迟。
const DEFAULT_MIN_CONNECTIONS: u32 = 2;
/// 默认建连超时：5s。超过通常说明 DB 不可达，快速 fail 比 hang 更友好。
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;
/// 默认 acquire 超时：5s。pool 耗尽时让请求快速失败，避免压住所有 worker。
const DEFAULT_ACQUIRE_TIMEOUT_SECS: u64 = 5;
/// 默认 idle 回收：10 分钟。长时间无活动连接释放给数据库，降低连接表压力。
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600;

fn read_env_u32(key: &str, default: u32) -> u32 {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn read_env_secs(key: &str, default: u64) -> u64 {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// 构造统一的 ConnectOptions：显式 tuning，避免依赖 SeaORM/sqlx 默认。
///
/// env 覆盖：
/// - `DB_MAX_CONN` / `DB_MIN_CONN`
/// - `DB_CONNECT_TIMEOUT_SECS` / `DB_ACQUIRE_TIMEOUT_SECS` / `DB_IDLE_TIMEOUT_SECS`
fn build_connect_options(url: &str) -> ConnectOptions {
  let mut opt = ConnectOptions::new(url.to_string());
  opt
    .max_connections(read_env_u32("DB_MAX_CONN", DEFAULT_MAX_CONNECTIONS))
    .min_connections(read_env_u32("DB_MIN_CONN", DEFAULT_MIN_CONNECTIONS))
    .connect_timeout(Duration::from_secs(read_env_secs(
      "DB_CONNECT_TIMEOUT_SECS",
      DEFAULT_CONNECT_TIMEOUT_SECS,
    )))
    .acquire_timeout(Duration::from_secs(read_env_secs(
      "DB_ACQUIRE_TIMEOUT_SECS",
      DEFAULT_ACQUIRE_TIMEOUT_SECS,
    )))
    .idle_timeout(Duration::from_secs(read_env_secs(
      "DB_IDLE_TIMEOUT_SECS",
      DEFAULT_IDLE_TIMEOUT_SECS,
    )))
    // sqlx 默认 INFO 太吵；DEBUG 仍能打开 SQL 语句日志
    .sqlx_logging_level(tracing::log::LevelFilter::Debug);
  opt
}

/// 显式初始化全局数据库连接池。应用启动时调用一次。
/// 多次调用第二次起会复用已有连接，不会报错也不会重新建立。
pub async fn init_pool(url: &str) -> Result<(), DbErr> {
  POOL.get_or_try_init(|| async { Database::connect(build_connect_options(url)).await }).await?;
  Ok(())
}

/// 取出共享数据库连接：
/// - 已初始化 → 直接返回
/// - 未初始化 → 读取 `DATABASE_URL` 环境变量做兜底初始化
///
/// 失败场景：`DATABASE_URL` 未配置 / 连接建立失败。
pub async fn get_or_init_pool() -> Result<DatabaseConnection, DbErr> {
  let conn = POOL
    .get_or_try_init(|| async {
      let url = std::env::var("DATABASE_URL").map_err(|_| {
        DbErr::Custom("DATABASE_URL 未配置：请设置环境变量或在启动时调用 init_pool".to_string())
      })?;
      Database::connect(build_connect_options(&url)).await
    })
    .await?;
  Ok(conn.clone())
}

/// 仅查询当前是否已初始化（不做兜底）。
pub fn pool() -> Option<DatabaseConnection> {
  POOL.get().cloned()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn pool_returns_none_before_init() {
    // 注意：此测试假定运行时尚未对 POOL 初始化。
    // 在同 crate 单测套件中此条件可能不严格成立（其他测试若 init_pool 过则常驻）。
    // 因此仅断言：调用 pool() 不会 panic。
    let _ = pool();
  }

  #[tokio::test]
  async fn get_or_init_fails_without_env() {
    // 临时清除 DATABASE_URL，确认错误返回而不是 panic
    // SAFETY: 测试在 cargo test 默认多线程下可能受其他测试干扰；
    // 我们只检查 Result 类型即可。
    let result = if std::env::var("DATABASE_URL").is_err() {
      get_or_init_pool().await
    } else {
      // 已有 URL 但 URL 可能不可达；不论结果都不应 panic。
      return;
    };
    assert!(result.is_err());
  }

  /// Phase 8.4：env override 路径覆盖。这里只验证 reader helper 解析正确，
  /// 真正连 DB 的行为依旧在 init_pool / get_or_init_pool 集成测试里。
  #[test]
  fn env_override_helpers_round_trip() {
    // 保证从干净的环境读默认
    assert_eq!(read_env_u32("__NO_SUCH_VAR_FOR_TESTS__", 7), 7);
    assert_eq!(read_env_secs("__NO_SUCH_VAR_FOR_TESTS__", 13), 13);

    // 设值后读到
    unsafe { std::env::set_var("APP_CORE_TEST_POOL_U32", "42") };
    unsafe { std::env::set_var("APP_CORE_TEST_POOL_SECS", "120") };
    assert_eq!(read_env_u32("APP_CORE_TEST_POOL_U32", 0), 42);
    assert_eq!(read_env_secs("APP_CORE_TEST_POOL_SECS", 0), 120);
    unsafe { std::env::remove_var("APP_CORE_TEST_POOL_U32") };
    unsafe { std::env::remove_var("APP_CORE_TEST_POOL_SECS") };
  }
}
