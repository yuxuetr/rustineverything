//! 采集流水线（`ingest` feature），由 `papers-ingest` 二进制调用，不在 app 进程里跑。

pub mod hf;
pub mod store;

use std::path::PathBuf;
use std::time::Instant;

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};

use crate::entities::paper_ingest_run;

/// HF 列表从哪来：线上接口，或者事先存下来的 `<dir>/<date>.json`（补导与测试）。
pub enum HfSource {
  Api(reqwest::Client),
  Dir(PathBuf),
}

/// 一天的统计，最后写进 `paper_ingest_runs`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunStats {
  pub fetched: usize,
  pub inserted: usize,
  pub generated: usize,
  pub failed: usize,
  pub input_tokens: u64,
  pub output_tokens: u64,
}

/// 跑一天：取列表 → 入库，最后记一行运行记录（失败也记，便于查哪天断了）。
pub async fn run_day(
  db: &DatabaseConnection,
  date: NaiveDate,
  source: &HfSource,
) -> Result<RunStats, String> {
  let started_at = chrono::Utc::now().fixed_offset();
  let clock = Instant::now();
  let result = ingest_list(db, date, source).await;
  let stats = result.as_ref().copied().unwrap_or_default();
  let row = paper_ingest_run::ActiveModel {
    run_on: Set(date),
    fetched: Set(stats.fetched as i32),
    inserted: Set(stats.inserted as i32),
    generated: Set(stats.generated as i32),
    failed: Set(stats.failed as i32),
    input_tokens: Set(stats.input_tokens as i64),
    output_tokens: Set(stats.output_tokens as i64),
    duration_ms: Set(clock.elapsed().as_millis() as i64),
    started_at: Set(started_at),
    ..Default::default()
  };
  row.insert(db).await.map_err(|e| format!("写运行记录失败：{e}"))?;
  result
}

async fn ingest_list(
  db: &DatabaseConnection,
  date: NaiveDate,
  source: &HfSource,
) -> Result<RunStats, String> {
  let papers = match source {
    HfSource::Api(client) => hf::fetch_daily(client, date).await?,
    HfSource::Dir(dir) => {
      let path = dir.join(format!("{date}.json"));
      let json =
        std::fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
      hf::parse_daily(&json)?
    }
  };
  let inserted =
    store::upsert_day(db, date, &papers).await.map_err(|e| format!("写入 papers 失败：{e}"))?;
  Ok(RunStats { fetched: papers.len(), inserted, ..Default::default() })
}
