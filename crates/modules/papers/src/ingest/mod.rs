//! 采集流水线（`ingest` feature），由 `papers-ingest` 二进制调用，不在 app 进程里跑。

pub mod check;
pub mod fulltext;
pub mod generate;
pub mod hf;
pub mod store;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use llm::{LlmClient, LlmUsage, OpenAiChat};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
  ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
  QueryOrder, QuerySelect, TransactionTrait,
};

use crate::entities::{paper, paper_ingest_run, paper_summary};

/// arXiv 要求每 3 秒最多一个请求。
const ARXIV_INTERVAL: Duration = Duration::from_secs(3);
/// 单次模型调用的超时：长论文抽取实测接近 30 秒，留足余量。
const LLM_TIMEOUT: Duration = Duration::from_secs(600);
/// 连接（含 TLS 握手）的超时：总超时很长，连不上时不该等满十分钟。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 每天默认最多生成几篇（`PAPERS_DAILY_LIMIT` 覆盖）。N0 实测每篇约 0.018 美元。
const DEFAULT_DAILY_LIMIT: usize = 20;

/// 生成解读要用的东西：模型（共用 `OPENAI_LLM_*`）、抓全文的 HTTP 客户端、每天上限。
pub struct Generator {
  pub llm: Box<dyn LlmClient>,
  pub model: String,
  pub http: reqwest::Client,
  pub daily_limit: usize,
}

impl Generator {
  pub fn from_env(http: reqwest::Client) -> Result<Self, String> {
    let cfg = llm::LlmConfig::from_env();
    let (Some(url), Some(key)) = (cfg.openai_base_url, cfg.openai_api_key) else {
      return Err("OPENAI_LLM_BASE_URL / OPENAI_LLM_API_KEY 未设置".into());
    };
    let model = cfg.openai_model.unwrap_or_else(|| "deepseek-chat".into());
    // 和 crates/llm 一样不跟随重定向：这个客户端带着 API key
    let llm_http = reqwest::Client::builder()
      .connect_timeout(CONNECT_TIMEOUT)
      .timeout(LLM_TIMEOUT)
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .map_err(|e| format!("构造 HTTP 客户端失败：{e}"))?;
    let daily_limit = match std::env::var("PAPERS_DAILY_LIMIT") {
      Ok(v) => v.trim().parse().map_err(|_| format!("PAPERS_DAILY_LIMIT 不是数字：{v}"))?,
      Err(_) => DEFAULT_DAILY_LIMIT,
    };
    Ok(Self {
      llm: Box::new(OpenAiChat::new(url, key, model.clone()).with_client(llm_http)),
      model,
      http,
      daily_limit,
    })
  }
}

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

/// 跑一天：取列表 → 入库 →（给了 `generator` 时）生成解读，最后记一行运行记录
/// （失败也记，便于查哪天断了）。
pub async fn run_day(
  db: &DatabaseConnection,
  date: NaiveDate,
  source: &HfSource,
  generator: Option<&Generator>,
) -> Result<RunStats, String> {
  let started_at = chrono::Utc::now().fixed_offset();
  let clock = Instant::now();
  let mut result = ingest_list(db, date, source).await;
  if let (Ok(stats), Some(g)) = (result.as_mut(), generator) {
    if let Err(e) = generate_day(db, date, g, stats).await {
      result = Err(e);
    }
  }
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

/// 当天还没生成的论文，按 upvotes 取前 `daily_limit` 篇逐篇生成。
/// 单篇失败只记在那一篇上（`status = failed`），不影响其他篇。
async fn generate_day(
  db: &DatabaseConnection,
  date: NaiveDate,
  g: &Generator,
  stats: &mut RunStats,
) -> Result<(), String> {
  let picked = paper::Entity::find()
    .filter(paper::Column::LastSeenOn.eq(date))
    .filter(paper::Column::Status.eq("pending"))
    .order_by_desc(paper::Column::HfUpvotes)
    .limit(g.daily_limit as u64)
    .all(db)
    .await
    .map_err(|e| format!("读取待生成论文失败：{e}"))?;
  for (i, p) in picked.into_iter().enumerate() {
    if i > 0 {
      tokio::time::sleep(ARXIV_INTERVAL).await;
    }
    let id = p.arxiv_id.clone();
    let mut usage = LlmUsage::default();
    let outcome = generate_one(db, g, p, &mut usage).await;
    stats.input_tokens += usage.input_tokens;
    stats.output_tokens += usage.output_tokens;
    match outcome {
      Ok(()) => {
        stats.generated += 1;
        tracing::info!(%id, input = usage.input_tokens, output = usage.output_tokens, "papers: generated");
      }
      Err(e) => {
        stats.failed += 1;
        tracing::warn!(%id, error = %e, "papers: generation failed");
        mark_failed(db, &id, &e).await?;
      }
    }
  }
  Ok(())
}

async fn generate_one(
  db: &DatabaseConnection,
  g: &Generator,
  p: paper::Model,
  usage: &mut LlmUsage,
) -> Result<(), String> {
  // 没有 HTML 版的论文只依据摘要生成，页面上会注明
  let (text, sections, has_html, version) = match fulltext::fetch(&g.http, &p.arxiv_id).await? {
    Some((ft, version)) => (ft.text, ft.sections, true, version),
    None => (format!("[§Abstract]\n{}", p.r#abstract), Vec::new(), false, None),
  };
  let (en, checked) =
    generate::digest_en(g.llm.as_ref(), &p.title, &text, &sections, usage).await?;
  let zh = generate::translate_zh(g.llm.as_ref(), &en, usage).await?;

  let now = chrono::Utc::now().fixed_offset();
  let unverified = serde_json::to_string(&checked.unverified_numbers).map_err(|e| e.to_string())?;
  let mut rows = Vec::new();
  for (lang, digest) in [("en", &en), ("zh", &zh)] {
    rows.push(paper_summary::ActiveModel {
      arxiv_id: Set(p.arxiv_id.clone()),
      lang: Set(lang.into()),
      digest: Set(serde_json::to_string(digest).map_err(|e| e.to_string())?),
      model: Set(g.model.clone()),
      prompt_version: Set(generate::PROMPT_VERSION),
      unverified_numbers: Set(unverified.clone()),
      created_at: Set(now),
    });
  }
  let txn = db.begin().await.map_err(|e| e.to_string())?;
  paper_summary::Entity::insert_many(rows)
    .on_conflict(
      OnConflict::columns([paper_summary::Column::ArxivId, paper_summary::Column::Lang])
        .update_columns([
          paper_summary::Column::Digest,
          paper_summary::Column::Model,
          paper_summary::Column::PromptVersion,
          paper_summary::Column::UnverifiedNumbers,
          paper_summary::Column::CreatedAt,
        ])
        .to_owned(),
    )
    .exec_without_returning(&txn)
    .await
    .map_err(|e| format!("写入解读失败：{e}"))?;
  paper::ActiveModel {
    arxiv_id: Set(p.arxiv_id),
    status: Set("done".into()),
    has_html: Set(Some(has_html)),
    arxiv_version: Set(version),
    error: Set(None),
    updated_at: Set(now),
    ..Default::default()
  }
  .update(&txn)
  .await
  .map_err(|e| format!("更新论文状态失败：{e}"))?;
  txn.commit().await.map_err(|e| e.to_string())
}

async fn mark_failed(db: &DatabaseConnection, arxiv_id: &str, error: &str) -> Result<(), String> {
  paper::ActiveModel {
    arxiv_id: Set(arxiv_id.to_string()),
    status: Set("failed".into()),
    error: Set(Some(error.chars().take(1000).collect())),
    updated_at: Set(chrono::Utc::now().fixed_offset()),
    ..Default::default()
  }
  .update(db)
  .await
  .map(|_| ())
  .map_err(|e| format!("记录失败状态失败：{e}"))
}
