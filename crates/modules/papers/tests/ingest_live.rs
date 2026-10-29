//! Live DB test：用存下来的 HF 列表导入，按 arXiv id 去重，重复导入不增行。
//!
//! 需要一个空的临时库（会清空 papers 相关表）和 HF 响应目录（`<date>.json`）：
//! ```sh
//! PAPERS_TEST_URL=postgres://.../scratch PAPERS_HF_DIR=/path/to/hf \
//! PAPERS_EXPECT=739 cargo test -p module-papers --features ingest --test ingest_live -- --ignored
//! ```

#![cfg(feature = "ingest")]

use std::path::PathBuf;

use migration::{Migrator, MigratorTrait};
use module_papers::entities::{paper, paper_ingest_run};
use module_papers::ingest::{hf, run_day, store, HfSource};
use sea_orm::{ConnectionTrait, EntityTrait, PaginatorTrait, QueryOrder};

#[tokio::test]
#[ignore = "Requires PAPERS_TEST_URL (scratch database) and PAPERS_HF_DIR"]
async fn importing_saved_days_dedupes_by_arxiv_id() {
  let (Ok(url), Ok(dir)) = (std::env::var("PAPERS_TEST_URL"), std::env::var("PAPERS_HF_DIR"))
  else {
    eprintln!("跳过：PAPERS_TEST_URL / PAPERS_HF_DIR 未设置");
    return;
  };
  let expect: u64 = std::env::var("PAPERS_EXPECT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
  let db = sea_orm::Database::connect(&url).await.unwrap();
  Migrator::up(&db, None).await.unwrap();
  db.execute_unprepared("TRUNCATE papers, paper_summaries, paper_ingest_runs").await.unwrap();

  let dir = PathBuf::from(dir);
  let mut dates: Vec<chrono::NaiveDate> = std::fs::read_dir(&dir)
    .unwrap()
    .filter_map(|e| {
      let name = e.ok()?.file_name().into_string().ok()?;
      chrono::NaiveDate::parse_from_str(name.strip_suffix(".json")?, "%Y-%m-%d").ok()
    })
    .collect();
  dates.sort();
  let source = HfSource::Dir(dir);

  let mut inserted = 0;
  for d in &dates {
    inserted += run_day(&db, *d, &source).await.unwrap().inserted;
  }
  let rows = paper::Entity::find().count(&db).await.unwrap();
  println!("{} days, {} rows", dates.len(), rows);
  assert_eq!(inserted as u64, rows);
  if expect > 0 {
    assert_eq!(rows, expect);
  }

  // 同一天再导一次：不增行，first_seen_on 不变
  let last = *dates.last().unwrap();
  let before = paper::Entity::find().order_by_asc(paper::Column::ArxivId).all(&db).await.unwrap();
  let again = run_day(&db, last, &source).await.unwrap();
  assert_eq!(again.inserted, 0);
  let after = paper::Entity::find().order_by_asc(paper::Column::ArxivId).all(&db).await.unwrap();
  assert_eq!(after.len(), before.len());
  for (a, b) in before.iter().zip(after.iter()) {
    assert_eq!((a.arxiv_id.as_str(), a.first_seen_on), (b.arxiv_id.as_str(), b.first_seen_on));
  }
  let runs = paper_ingest_run::Entity::find().count(&db).await.unwrap();
  assert_eq!(runs as usize, dates.len() + 1);

  // 同一篇在更晚、更早的日期再出现：last_seen_on 取最晚，first_seen_on 取最早
  let first = dates[0];
  let json = std::fs::read_to_string(
    PathBuf::from(std::env::var("PAPERS_HF_DIR").unwrap()).join(format!("{first}.json")),
  )
  .unwrap();
  let one = hf::parse_daily(&json).unwrap().into_iter().take(1).collect::<Vec<_>>();
  let id = one[0].arxiv_id.clone();
  let later = last + chrono::Duration::days(30);
  let earlier = first - chrono::Duration::days(30);
  assert_eq!(store::upsert_day(&db, later, &one).await.unwrap(), 0);
  assert_eq!(store::upsert_day(&db, first, &one).await.unwrap(), 0);
  let p = paper::Entity::find_by_id(id.clone()).one(&db).await.unwrap().unwrap();
  assert_eq!((p.first_seen_on, p.last_seen_on), (first, later));
  store::upsert_day(&db, earlier, &one).await.unwrap();
  let p = paper::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
  assert_eq!((p.first_seen_on, p.last_seen_on), (earlier, later));
}
