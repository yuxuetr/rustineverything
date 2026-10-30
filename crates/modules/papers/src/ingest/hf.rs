//! Hugging Face Daily Papers 列表：`/api/daily_papers?date=YYYY-MM-DD`。

use app_core::error::with_sources;
use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::Deserialize;

/// 每天最多取多少篇。实测 2026-10 每天 45–97 篇。
pub const HF_LIMIT: usize = 100;

#[derive(Deserialize)]
struct Entry {
  paper: WirePaper,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePaper {
  id: String,
  title: String,
  #[serde(default)]
  summary: String,
  #[serde(default)]
  authors: Vec<WireAuthor>,
  published_at: DateTime<FixedOffset>,
  #[serde(default)]
  upvotes: i32,
  #[serde(default)]
  github_repo: Option<String>,
}

#[derive(Deserialize)]
struct WireAuthor {
  #[serde(default)]
  name: String,
  #[serde(default)]
  hidden: bool,
}

/// 列表里的一篇，只留入库要用的字段。
#[derive(Debug, Clone, PartialEq)]
pub struct HfPaper {
  /// 不带版本号的 arXiv id
  pub arxiv_id: String,
  pub title: String,
  pub summary: String,
  pub authors: Vec<String>,
  pub published_at: DateTime<FixedOffset>,
  pub upvotes: i32,
  pub github_repo: Option<String>,
}

/// 去掉 arXiv id 末尾的版本号：`2510.12345v2` → `2510.12345`。
pub fn strip_version(id: &str) -> &str {
  match id.rfind('v') {
    Some(i) if i > 0 && id[i + 1..].chars().all(|c| c.is_ascii_digit()) && i + 1 < id.len() => {
      &id[..i]
    }
    _ => id,
  }
}

/// 解析一天的列表 JSON。同一个 id 出现两次时保留第一次。
pub fn parse_daily(json: &str) -> Result<Vec<HfPaper>, String> {
  let entries: Vec<Entry> =
    serde_json::from_str(json).map_err(|e| format!("HF 列表不是预期的 JSON：{e}"))?;
  let mut out: Vec<HfPaper> = Vec::with_capacity(entries.len());
  for Entry { paper: p } in entries {
    let arxiv_id = strip_version(p.id.trim()).to_string();
    if arxiv_id.is_empty() || out.iter().any(|x| x.arxiv_id == arxiv_id) {
      continue;
    }
    out.push(HfPaper {
      arxiv_id,
      title: squash_spaces(&p.title),
      summary: p.summary.trim().to_string(),
      authors: p
        .authors
        .into_iter()
        .filter(|a| !a.hidden && !a.name.trim().is_empty())
        .map(|a| a.name.trim().to_string())
        .collect(),
      published_at: p.published_at,
      upvotes: p.upvotes,
      github_repo: p.github_repo.filter(|r| !r.trim().is_empty()),
    });
  }
  Ok(out)
}

/// HF 的标题里常带换行和连续空格。
fn squash_spaces(s: &str) -> String {
  s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 下载某一天的列表。
pub async fn fetch_daily(
  client: &reqwest::Client,
  date: NaiveDate,
) -> Result<Vec<HfPaper>, String> {
  let url = format!("https://huggingface.co/api/daily_papers?date={date}&limit={HF_LIMIT}");
  let resp =
    client.get(&url).send().await.map_err(|e| format!("HF 请求失败：{}", with_sources(&e)))?;
  let status = resp.status();
  let body = resp.text().await.map_err(|e| format!("HF 响应读取失败：{}", with_sources(&e)))?;
  if !status.is_success() {
    return Err(format!("HF 返回 {status}"));
  }
  let papers = parse_daily(&body)?;
  if papers.len() >= HF_LIMIT {
    tracing::warn!(%date, n = papers.len(), "papers: HF 列表达到上限，可能没取全");
  }
  Ok(papers)
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = r#"[
    {"paper": {"id": "2610.18773", "title": "Fast\n  Things", "summary": " abs ",
      "authors": [{"name": "A B", "hidden": false}, {"name": "Hidden", "hidden": true}],
      "publishedAt": "2026-10-13T00:00:00.000Z", "upvotes": 459,
      "githubRepo": "https://github.com/x/y"}, "title": "Fast Things"},
    {"paper": {"id": "2610.18773v2", "title": "dup", "publishedAt": "2026-10-13T00:00:00.000Z"}},
    {"paper": {"id": "2511.00042v3", "title": "Old", "publishedAt": "2026-10-12T00:00:00.000Z",
      "githubRepo": ""}}
  ]"#;

  #[test]
  fn parses_dedupes_and_strips_versions() {
    let papers = parse_daily(SAMPLE).unwrap();
    let ids: Vec<&str> = papers.iter().map(|p| p.arxiv_id.as_str()).collect();
    assert_eq!(ids, ["2610.18773", "2511.00042"]);
    assert_eq!(papers[0].title, "Fast Things");
    assert_eq!(papers[0].summary, "abs");
    assert_eq!(papers[0].authors, ["A B"]);
    assert_eq!(papers[0].upvotes, 459);
    assert_eq!(papers[1].upvotes, 0);
    assert_eq!(papers[1].github_repo, None);
  }

  #[test]
  fn strip_version_only_removes_numeric_suffix() {
    assert_eq!(strip_version("2510.12345v12"), "2510.12345");
    assert_eq!(strip_version("2510.12345"), "2510.12345");
    assert_eq!(strip_version("2510.12345v"), "2510.12345v");
    assert_eq!(strip_version("hep-th/9901001v1"), "hep-th/9901001");
  }
}
