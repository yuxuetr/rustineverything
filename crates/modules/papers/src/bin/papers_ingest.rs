//! 论文采集：取 HF Daily Papers 列表入库，再给当天 upvotes 最高的几篇生成解读。
//! 服务器 cron 每天跑一次。
//!
//! ```sh
//! papers-ingest                        # 昨天（UTC）
//! papers-ingest --date 2026-10-28      # 指定日期，可重复
//! papers-ingest --hf-dir saved/        # 从存下来的 <date>.json 导入（不带 --date 时导入目录里全部）
//! papers-ingest --list-only            # 只入库列表，不调模型
//! ```
//! 读 `DATABASE_URL`、`OPENAI_LLM_*`、`PAPERS_DAILY_LIMIT`（默认 20）；表由 app 启动时的迁移建好。

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::NaiveDate;
use module_papers::ingest::{run_day, Generator, HfSource};

struct Args {
  dates: Vec<NaiveDate>,
  hf_dir: Option<PathBuf>,
  list_only: bool,
}

fn parse_args() -> Result<Args, String> {
  let mut dates = Vec::new();
  let mut hf_dir = None;
  let mut list_only = false;
  let mut it = std::env::args().skip(1);
  while let Some(a) = it.next() {
    match a.as_str() {
      "--date" => {
        let v = it.next().ok_or("--date 缺少值")?;
        dates
          .push(NaiveDate::parse_from_str(&v, "%Y-%m-%d").map_err(|e| format!("--date {v}: {e}"))?);
      }
      "--hf-dir" => hf_dir = Some(PathBuf::from(it.next().ok_or("--hf-dir 缺少值")?)),
      "--list-only" => list_only = true,
      other => return Err(format!("未知参数：{other}")),
    }
  }
  if dates.is_empty() {
    dates = match &hf_dir {
      Some(dir) => dates_in_dir(dir)?,
      None => vec![(chrono::Utc::now() - chrono::Duration::days(1)).date_naive()],
    };
  }
  Ok(Args { dates, hf_dir, list_only })
}

/// 目录里 `<YYYY-MM-DD>.json` 的日期，升序。
fn dates_in_dir(dir: &PathBuf) -> Result<Vec<NaiveDate>, String> {
  let entries = std::fs::read_dir(dir).map_err(|e| format!("读取 {} 失败：{e}", dir.display()))?;
  let mut dates: Vec<NaiveDate> = entries
    .filter_map(|e| e.ok())
    .filter_map(|e| {
      let name = e.file_name().into_string().ok()?;
      NaiveDate::parse_from_str(name.strip_suffix(".json")?, "%Y-%m-%d").ok()
    })
    .collect();
  dates.sort();
  Ok(dates)
}

#[tokio::main]
async fn main() -> ExitCode {
  let _ = dotenvy::dotenv();
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
    )
    .init();

  let args = match parse_args() {
    Ok(a) => a,
    Err(e) => {
      eprintln!("{e}");
      return ExitCode::from(2);
    }
  };
  let Ok(url) = std::env::var("DATABASE_URL") else {
    eprintln!("DATABASE_URL 未设置");
    return ExitCode::from(2);
  };
  let db = match sea_orm::Database::connect(&url).await {
    Ok(db) => db,
    Err(e) => {
      eprintln!("连接数据库失败：{e}");
      return ExitCode::FAILURE;
    }
  };
  // arXiv 与 HF 都建议带上能联系到人的 User-Agent
  let http = match reqwest::Client::builder()
    .user_agent("rustineverything-papers/0.1 (+https://rustineverything.app)")
    .timeout(std::time::Duration::from_secs(60))
    .build()
  {
    Ok(c) => c,
    Err(e) => {
      eprintln!("构造 HTTP 客户端失败：{e}");
      return ExitCode::FAILURE;
    }
  };
  let generator = if args.list_only {
    None
  } else {
    match Generator::from_env(http.clone()) {
      Ok(g) => Some(g),
      Err(e) => {
        eprintln!("{e}（只入库列表请加 --list-only）");
        return ExitCode::from(2);
      }
    }
  };
  let source = match args.hf_dir {
    Some(dir) => HfSource::Dir(dir),
    None => HfSource::Api(http),
  };

  let mut ok = true;
  for date in args.dates {
    match run_day(&db, date, &source, generator.as_ref()).await {
      Ok(s) => tracing::info!(
        %date,
        fetched = s.fetched,
        inserted = s.inserted,
        generated = s.generated,
        failed = s.failed,
        input_tokens = s.input_tokens,
        output_tokens = s.output_tokens,
        "papers: day done"
      ),
      Err(e) => {
        ok = false;
        tracing::error!(%date, error = %e, "papers: day failed");
      }
    }
  }
  if ok {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}
