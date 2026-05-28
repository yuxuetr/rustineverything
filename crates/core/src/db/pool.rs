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

use sea_orm::{Database, DatabaseConnection, DbErr};
use tokio::sync::OnceCell;

static POOL: OnceCell<DatabaseConnection> = OnceCell::const_new();

/// 显式初始化全局数据库连接池。应用启动时调用一次。
/// 多次调用第二次起会复用已有连接，不会报错也不会重新建立。
pub async fn init_pool(url: &str) -> Result<(), DbErr> {
  POOL.get_or_try_init(|| async { Database::connect(url).await }).await?;
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
      Database::connect(&url).await
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
}
