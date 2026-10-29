#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Live DB test：`m20261029_000011_papers` 能来回跑，外键级联生效。
//!
//! 用一个空的临时库，不要指向开发库（会回滚最后一个迁移）：
//! ```sh
//! MIGRATION_TEST_URL=postgres://.../rie_papers_tmp \
//!   cargo test -p migration --test papers_roundtrip_live -- --ignored
//! ```

use migration::{Migrator, MigratorTrait};
use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

async fn tables(db: &DatabaseConnection) -> Vec<String> {
  let rows = db
    .query_all(Statement::from_string(
      db.get_database_backend(),
      "SELECT table_name::text FROM information_schema.tables \
       WHERE table_schema = 'public' AND table_name LIKE 'paper%' ORDER BY 1",
    ))
    .await
    .unwrap();
  rows.iter().map(|r| r.try_get_by_index::<String>(0).unwrap()).collect()
}

#[tokio::test]
#[ignore = "Requires MIGRATION_TEST_URL pointing at an empty scratch database"]
async fn papers_migration_round_trips() {
  let Ok(url) = std::env::var("MIGRATION_TEST_URL") else {
    eprintln!("跳过：MIGRATION_TEST_URL 未设置");
    return;
  };
  let db = Database::connect(&url).await.unwrap();
  let all = ["paper_ingest_runs", "paper_summaries", "papers"];

  Migrator::up(&db, None).await.unwrap();
  assert_eq!(tables(&db).await, all);

  Migrator::down(&db, Some(1)).await.unwrap();
  assert!(tables(&db).await.is_empty());

  Migrator::up(&db, None).await.unwrap();
  assert_eq!(tables(&db).await, all);

  // 删论文时解读跟着删
  for sql in [
    "INSERT INTO papers (arxiv_id, title, abstract, authors, published_at, first_seen_on, \
     last_seen_on, created_at, updated_at) VALUES ('2510.00001', 't', 'a', '[]', now(), \
     current_date, current_date, now(), now())",
    "INSERT INTO paper_summaries (arxiv_id, lang, digest, model, prompt_version, created_at) \
     VALUES ('2510.00001', 'en', '{}', 'm', 1, now())",
    "DELETE FROM papers WHERE arxiv_id = '2510.00001'",
  ] {
    db.execute_unprepared(sql).await.unwrap();
  }
  let left = db
    .query_one(Statement::from_string(
      db.get_database_backend(),
      "SELECT count(*) FROM paper_summaries",
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index::<i64>(0)
    .unwrap();
  assert_eq!(left, 0);
}
