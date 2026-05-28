use sea_orm::{Database, DatabaseConnection};

pub mod pool;
pub use pool::{get_or_init_pool, init_pool};

/// 历史调用点：每次都新建连接。仅供〈尚未迁移到连接池〉的调用者使用，
/// 新代码请一律调用 [`get_or_init_pool`]。
#[deprecated(note = "Use rustineverything_core::db::get_or_init_pool() instead")]
pub async fn init_db(url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
  Database::connect(url).await
}
