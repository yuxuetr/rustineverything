//! Live DB test：等审核的内容在 admin 通过时公开（`publish_held`）。
//!
//! 默认 `#[ignore]`：要求 `DATABASE_URL` 可用。
//! ```sh
//! cargo test -p module-moderation --test pending_review_live \
//!   -- --ignored --nocapture --test-threads=1
//! ```

use std::path::PathBuf;

use app_core::entities::{comment, topic, topic_reply, user};
use module_moderation::publish_held;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| PathBuf::from("."))
}

async fn setup_db() -> Option<sea_orm::DatabaseConnection> {
  let _ = dotenvy::from_path(workspace_root().join(".env"));
  let url = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty())?;
  let db = sea_orm::Database::connect(&url).await.ok()?;
  use migration::MigratorTrait;
  migration::Migrator::up(&db, None).await.ok()?;
  Some(db)
}

#[tokio::test]
#[ignore = "Requires DATABASE_URL"]
async fn approving_held_content_publishes_it_and_counts_reply_once() {
  let Some(db) = setup_db().await else {
    eprintln!("跳过：DATABASE_URL 不可用");
    return;
  };
  let now = chrono::Utc::now().fixed_offset();

  let u = user::ActiveModel {
    nickname: Set("pending-review-test".into()),
    role: Set("member".into()),
    created_at: Set(now),
    updated_at: Set(now),
    ..Default::default()
  }
  .insert(&db)
  .await
  .unwrap();
  let t = topic::ActiveModel {
    title: Set("held topic".into()),
    tag: Set("test".into()),
    content: Set("x".into()),
    user_id: Set(u.id),
    reply_count: Set(0),
    created_at: Set(now),
    updated_at: Set(now),
    pending_review: Set(true),
    ..Default::default()
  }
  .insert(&db)
  .await
  .unwrap();
  let r = topic_reply::ActiveModel {
    topic_id: Set(t.id),
    user_id: Set(u.id),
    content: Set("held reply".into()),
    created_at: Set(now),
    pending_review: Set(true),
    ..Default::default()
  }
  .insert(&db)
  .await
  .unwrap();
  let c = comment::ActiveModel {
    blog_id: Set("pending-review-test".into()),
    user_id: Set(u.id),
    content: Set("held comment".into()),
    created_at: Set(now),
    pending_review: Set(true),
    ..Default::default()
  }
  .insert(&db)
  .await
  .unwrap();

  publish_held(&db, "topic", t.id as i64).await.unwrap();
  publish_held(&db, "comment", c.id as i64).await.unwrap();
  // 通过两次（单条 + 批量重叠）：回复数只加一次
  publish_held(&db, "reply", r.id as i64).await.unwrap();
  publish_held(&db, "reply", r.id as i64).await.unwrap();

  let t2 = topic::Entity::find_by_id(t.id).one(&db).await.unwrap().unwrap();
  let r2 = topic_reply::Entity::find_by_id(r.id).one(&db).await.unwrap().unwrap();
  let c2 = comment::Entity::find_by_id(c.id).one(&db).await.unwrap().unwrap();

  // 先清理再断言，断言失败也不留测试行
  topic_reply::Entity::delete_by_id(r.id).exec(&db).await.unwrap();
  topic::Entity::delete_by_id(t.id).exec(&db).await.unwrap();
  comment::Entity::delete_by_id(c.id).exec(&db).await.unwrap();
  user::Entity::delete_by_id(u.id).exec(&db).await.unwrap();

  assert!(!t2.pending_review && !r2.pending_review && !c2.pending_review);
  assert_eq!(t2.reply_count, 1);
  assert!(t2.last_reply_at.is_some());
}
