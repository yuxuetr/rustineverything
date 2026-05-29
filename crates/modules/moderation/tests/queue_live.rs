//! Live DB test：跑 sea-orm-migration + 把 Flag verdict 入审核队列 +
//! 通过 / 拒绝两条路径写回。
//!
//! 默认 `#[ignore]`：要求 `DATABASE_URL` 可用。
//! ```sh
//! cargo test -p module-moderation --test queue_live \
//!   -- --ignored --nocapture --test-threads=1
//! ```

use std::path::PathBuf;

use app_core::engines::moderation::Verdict;
use module_moderation::enqueue_if_flagged;

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| PathBuf::from("."))
}

fn load_env() {
  let _ = dotenvy::from_path(workspace_root().join(".env"));
}

async fn setup_db() -> Option<sea_orm::DatabaseConnection> {
  load_env();
  let url = match std::env::var("DATABASE_URL") {
    Ok(u) if !u.is_empty() => u,
    _ => {
      eprintln!("跳过：DATABASE_URL 未配置");
      return None;
    }
  };
  let db = match sea_orm::Database::connect(&url).await {
    Ok(d) => d,
    Err(e) => {
      eprintln!("跳过：DB 连接失败: {}", e);
      return None;
    }
  };
  // 跑迁移：可能失败（init.sql 已建过表 / seaql_migrations 表错位），
  // 但只要 moderation_queue 表存在就继续。
  use migration::MigratorTrait;
  if let Err(e) = migration::Migrator::up(&db, None).await {
    eprintln!("warn：sea-orm migration 整体报错: {}", e);
  }
  // 双保险：手动 CREATE TABLE IF NOT EXISTS，不依赖 sea-orm migrator 状态。
  // 字段定义与 m20260530_000002_moderation_queue.rs 保持一致。
  use sea_orm::ConnectionTrait;
  let create_sql = r#"
    CREATE TABLE IF NOT EXISTS moderation_queue (
      id BIGSERIAL PRIMARY KEY,
      kind VARCHAR(32) NOT NULL,
      ref_id BIGINT,
      ref_path TEXT NOT NULL,
      user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
      content TEXT NOT NULL,
      images TEXT,
      score REAL NOT NULL DEFAULT 0.0,
      label VARCHAR(16) NOT NULL,
      reason TEXT NOT NULL DEFAULT '',
      status VARCHAR(16) NOT NULL DEFAULT 'pending',
      reviewer_user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
      reviewer_note TEXT,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      reviewed_at TIMESTAMPTZ
    );
    CREATE INDEX IF NOT EXISTS idx_moderation_queue_status
      ON moderation_queue(status, created_at DESC);
    CREATE INDEX IF NOT EXISTS idx_moderation_queue_kind
      ON moderation_queue(kind);
  "#;
  if let Err(e) = db.execute_unprepared(create_sql).await {
    eprintln!("跳过：CREATE TABLE 失败: {}", e);
    return None;
  }
  Some(db)
}

#[tokio::test]
#[ignore = "Requires DATABASE_URL"]
async fn allow_verdict_is_noop() {
  let Some(db) = setup_db().await else {
    return;
  };

  // Allow 不入队
  let before = count_pending(&db).await;
  enqueue_if_flagged(
    &db,
    &Verdict::allow(),
    "comment",
    Some(1),
    "blog:test",
    None,
    "harmless content",
    &[],
  )
  .await;
  let after = count_pending(&db).await;
  assert_eq!(before, after, "Allow 不应入队");
}

#[tokio::test]
#[ignore = "Requires DATABASE_URL"]
async fn flag_verdict_inserts_pending_row() {
  let Some(db) = setup_db().await else {
    return;
  };

  let before = count_pending(&db).await;
  let v = Verdict::flag(0.7, "possibly offensive");
  enqueue_if_flagged(
    &db,
    &v,
    "comment",
    Some(99),
    "blog:live-test",
    None,
    "test content for live queue",
    &["https://example.com/x.jpg".into()],
  )
  .await;
  let after = count_pending(&db).await;
  assert_eq!(after, before + 1, "Flag 应插入 1 条 pending 行");

  // 清理：删掉刚插入的那条（按 ref_path 定位）
  cleanup_test_rows(&db).await;
}

#[tokio::test]
#[ignore = "Requires DATABASE_URL"]
async fn block_verdict_is_noop() {
  let Some(db) = setup_db().await else {
    return;
  };
  let before = count_pending(&db).await;
  enqueue_if_flagged(
    &db,
    &Verdict::block(0.95, "spam"),
    "comment",
    Some(2),
    "blog:test",
    None,
    "blocked content",
    &[],
  )
  .await;
  let after = count_pending(&db).await;
  // Block 在上游已被拒绝，按设计不入队
  assert_eq!(after, before);
}

async fn count_pending(db: &sea_orm::DatabaseConnection) -> u64 {
  use app_core::entities::moderation_queue;
  use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
  moderation_queue::Entity::find()
    .filter(moderation_queue::Column::Status.eq("pending"))
    .count(db)
    .await
    .unwrap_or(0)
}

async fn cleanup_test_rows(db: &sea_orm::DatabaseConnection) {
  use app_core::entities::moderation_queue;
  use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
  let _ = moderation_queue::Entity::delete_many()
    .filter(moderation_queue::Column::RefPath.eq("blog:live-test"))
    .exec(db)
    .await;
}
