//! 页面用的 server fn，以及 app 组装 sitemap / 搜索时调用的服务端函数。
//! 只读已经生成好解读（`status = done`）的论文。

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::digest::PaperDigest;

/// 列表页每次最多取多少篇。
#[cfg(feature = "server")]
const LIST_LIMIT: u64 = 200;

/// 列表里的一篇。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaperCard {
  pub arxiv_id: String,
  pub title: String,
  /// HF 日期，`YYYY-MM-DD`
  pub date: String,
  pub hf_upvotes: i32,
  /// 界面语言的一句话总结，没有就用英文
  pub tldr: String,
}

/// 单篇解读页的数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaperDetail {
  pub arxiv_id: String,
  pub title: String,
  pub authors: Vec<String>,
  pub date: String,
  pub hf_upvotes: i32,
  pub github_repo: Option<String>,
  /// false：没有 HTML 全文，只依据摘要生成
  pub has_html: bool,
  pub arxiv_version: Option<i32>,
  /// 实际显示的语言（要的语言没有时退回 `en`）
  pub lang: String,
  pub model: String,
  pub digest: PaperDigest,
  /// 原文里没找到的数字
  pub unverified_numbers: Vec<String>,
}

/// 最近的论文，按 HF 日期倒序、同一天按 upvotes 倒序。
#[post("/api/papers/list")]
pub async fn list_papers(lang: String) -> Result<Vec<PaperCard>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use crate::entities::{paper, paper_summary};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
    let db = open_db().await?;
    let rows = paper::Entity::find()
      .filter(paper::Column::Status.eq("done"))
      .order_by_desc(paper::Column::FirstSeenOn)
      .order_by_desc(paper::Column::HfUpvotes)
      .limit(LIST_LIMIT)
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    let ids: Vec<String> = rows.iter().map(|p| p.arxiv_id.clone()).collect();
    let summaries = paper_summary::Entity::find()
      .filter(paper_summary::Column::ArxivId.is_in(ids))
      .filter(paper_summary::Column::Lang.is_in([lang.as_str(), "en"]))
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(
      rows
        .into_iter()
        .map(|p| {
          let tldr = pick_summary(&summaries, &p.arxiv_id, &lang)
            .and_then(|s| serde_json::from_str::<PaperDigest>(&s.digest).ok())
            .map(|d| d.tldr)
            .unwrap_or_default();
          PaperCard {
            arxiv_id: p.arxiv_id,
            title: p.title,
            date: p.first_seen_on.to_string(),
            hf_upvotes: p.hf_upvotes,
            tldr,
          }
        })
        .collect(),
    )
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = lang;
    Ok(vec![])
  }
}

/// 单篇解读。论文不存在或还没生成时返回 `None`。
#[post("/api/papers/get")]
pub async fn get_paper(
  arxiv_id: String,
  lang: String,
) -> Result<Option<PaperDetail>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use crate::entities::{paper, paper_summary};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let db = open_db().await?;
    let Some(p) = paper::Entity::find_by_id(arxiv_id.clone())
      .filter(paper::Column::Status.eq("done"))
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
    else {
      return Ok(None);
    };
    let summaries = paper_summary::Entity::find()
      .filter(paper_summary::Column::ArxivId.eq(arxiv_id))
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    let Some(s) = pick_summary(&summaries, &p.arxiv_id, &lang) else {
      return Ok(None);
    };
    let digest: PaperDigest =
      serde_json::from_str(&s.digest).map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(Some(PaperDetail {
      arxiv_id: p.arxiv_id,
      title: p.title,
      authors: serde_json::from_str(&p.authors).unwrap_or_default(),
      date: p.first_seen_on.to_string(),
      hf_upvotes: p.hf_upvotes,
      github_repo: p.github_repo,
      has_html: p.has_html.unwrap_or(false),
      arxiv_version: p.arxiv_version,
      lang: s.lang.clone(),
      model: s.model.clone(),
      digest,
      unverified_numbers: serde_json::from_str(&s.unverified_numbers).unwrap_or_default(),
    }))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (arxiv_id, lang);
    Ok(None)
  }
}

/// 要的语言，没有就英文。
#[cfg(feature = "server")]
fn pick_summary<'a>(
  summaries: &'a [crate::entities::paper_summary::Model],
  arxiv_id: &str,
  lang: &str,
) -> Option<&'a crate::entities::paper_summary::Model> {
  let find = |l: &str| summaries.iter().find(|s| s.arxiv_id == arxiv_id && s.lang == l);
  find(lang).or_else(|| find("en"))
}

#[cfg(feature = "server")]
async fn open_db() -> Result<sea_orm::DatabaseConnection, ServerFnError> {
  app_core::db::get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))
}

/// 全部已生成论文的 `/papers/<id>`，给 sitemap。数据库不可用时为空。
#[cfg(feature = "server")]
pub async fn sitemap_paths() -> Vec<String> {
  use crate::entities::paper;
  use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
  let Ok(db) = app_core::db::get_or_init_pool().await else {
    return Vec::new();
  };
  let ids: Vec<String> = paper::Entity::find()
    .select_only()
    .column(paper::Column::ArxivId)
    .filter(paper::Column::Status.eq("done"))
    .into_tuple()
    .all(&db)
    .await
    .unwrap_or_default();
  ids.into_iter().map(|id| format!("/papers/{id}")).collect()
}

/// 搜索索引用的文档（kind = `paper`）：标题、摘要、各语言的一句话总结与要点。
#[cfg(feature = "server")]
pub async fn search_docs() -> Vec<app_core::engines::doc_source::ExternalIndexedDoc> {
  use crate::entities::{paper, paper_summary};
  use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
  let Ok(db) = app_core::db::get_or_init_pool().await else {
    return Vec::new();
  };
  let papers = match paper::Entity::find()
    .filter(paper::Column::Status.eq("done"))
    .find_with_related(paper_summary::Entity)
    .all(&db)
    .await
  {
    Ok(rows) => rows,
    Err(e) => {
      tracing::warn!(error = %e, "papers: search source unavailable");
      return Vec::new();
    }
  };
  papers
    .into_iter()
    .map(|(p, summaries)| {
      let mut body = p.r#abstract.clone();
      for s in summaries {
        if let Ok(d) = serde_json::from_str::<PaperDigest>(&s.digest) {
          body.push(' ');
          body.push_str(&d.tldr);
          for k in d.key_points {
            body.push(' ');
            body.push_str(&k.text);
          }
        }
      }
      app_core::engines::doc_source::ExternalIndexedDoc {
        kind: "paper".to_string(),
        url: format!("/papers/{}", p.arxiv_id),
        ref_id: p.arxiv_id,
        title: p.title,
        body,
        created_at: p.first_seen_on.to_string(),
      }
    })
    .collect()
}
