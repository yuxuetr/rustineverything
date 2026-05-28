use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::path::{Path, PathBuf};

// =============================================================
// Public types (shared between server and client)
// =============================================================

/// 话题关联的源资源（博客/文档/课程/课节/案例）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopicRef {
  pub kind: String, // "blog" | "doc" | "course" | "lesson" | "case"
  pub path: String, // 资源叶子路径
  pub title: String,
}

/// 话题摘要（列表项，不含正文）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopicSummary {
  pub id: i32,
  pub title: String,
  pub tag: String,
  pub author: String,
  pub author_avatar: Option<String>,
  pub user_id: i32,
  pub reply_count: i32,
  pub last_reply_at: Option<String>,
  pub created_at: String,
  pub reference: Option<TopicRef>,
}

/// 单条回复
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reply {
  pub id: i32,
  pub content: String,
  pub author: String,
  pub author_avatar: Option<String>,
  pub user_id: i32,
  pub created_at: String,
}

/// 话题完整详情 + 回复列表
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopicDetail {
  pub id: i32,
  pub title: String,
  pub tag: String,
  pub content: String,
  pub author: String,
  pub author_avatar: Option<String>,
  pub user_id: i32,
  pub created_at: String,
  pub updated_at: String,
  pub reference: Option<TopicRef>,
  pub replies: Vec<Reply>,
}

/// Tag 聚合摘要
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagSummary {
  pub tag: String,
  pub topic_count: i64,
}

/// 创建话题输入
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NewTopicInput {
  pub title: String,
  pub tag: String,
  pub content: String,
  #[serde(default)]
  pub ref_kind: Option<String>,
  #[serde(default)]
  pub ref_path: Option<String>,
}

// =============================================================
// Pure validation / normalization helpers (testable without DB)
// =============================================================

pub const MAX_TITLE_LEN: usize = 255;
pub const MAX_TAG_LEN: usize = 64;
pub const MAX_CONTENT_LEN: usize = 64 * 1024;
pub const MAX_REPLY_LEN: usize = 32 * 1024;

/// 规范化 tag：trim、转小写、限制字符集 [a-z0-9_-]，截断到 64
pub fn normalize_tag(raw: &str) -> String {
  let trimmed = raw.trim().to_lowercase();
  let cleaned: String = trimmed
    .chars()
    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
    .take(MAX_TAG_LEN)
    .collect();
  cleaned
}

/// 校验话题输入；返回错误描述（中文）
pub fn validate_new_topic(input: &NewTopicInput) -> Result<(), String> {
  let title = input.title.trim();
  if title.is_empty() {
    return Err("标题不能为空".to_string());
  }
  if title.chars().count() > MAX_TITLE_LEN {
    return Err(format!("标题不能超过 {} 个字符", MAX_TITLE_LEN));
  }

  let tag = normalize_tag(&input.tag);
  if tag.is_empty() {
    return Err("标签不能为空，且只能包含字母、数字、下划线、连字符".to_string());
  }

  if input.content.trim().is_empty() {
    return Err("正文不能为空".to_string());
  }
  if input.content.len() > MAX_CONTENT_LEN {
    return Err(format!("正文长度不能超过 {} 字节", MAX_CONTENT_LEN));
  }

  // 引用必须 kind 与 path 同时存在或同时不存在
  match (&input.ref_kind, &input.ref_path) {
    (Some(k), Some(p)) => {
      if !matches!(k.as_str(), "blog" | "doc" | "course" | "lesson" | "case") {
        return Err(format!("不支持的引用类型: {}", k));
      }
      if p.trim().is_empty() {
        return Err("引用路径不能为空".to_string());
      }
    }
    (None, None) => {}
    _ => return Err("引用类型与路径必须同时提供".to_string()),
  }

  Ok(())
}

/// 校验回复内容
pub fn validate_new_reply(content: &str) -> Result<(), String> {
  if content.trim().is_empty() {
    return Err("回复内容不能为空".to_string());
  }
  if content.len() > MAX_REPLY_LEN {
    return Err(format!("回复长度不能超过 {} 字节", MAX_REPLY_LEN));
  }
  Ok(())
}

// =============================================================
// Server-only helpers
// =============================================================

#[allow(dead_code)]
#[cfg(feature = "server")]
fn get_asset_root() -> PathBuf {
  let p = PathBuf::from("assets");
  if p.exists() {
    p
  } else {
    PathBuf::from("../../assets")
  }
}

/// 从 frontmatter / 课程 YAML / 一级标题中尽力提取标题
#[cfg(feature = "server")]
fn extract_title_line(content: &str) -> Option<String> {
  // 尝试 frontmatter `title: xxx`
  if content.starts_with("---") {
    if let Some(rest) = content.splitn(3, "---").nth(1) {
      for line in rest.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("title:") {
          let t = value.trim().trim_matches(|c| c == '"' || c == '\'');
          if !t.is_empty() {
            return Some(t.to_string());
          }
        }
      }
    }
  }
  // 退而求其次：第一个 # 标题
  for line in content.lines() {
    let trimmed = line.trim();
    if let Some(t) = trimmed.strip_prefix("# ") {
      return Some(t.trim().to_string());
    }
  }
  None
}

#[cfg(feature = "server")]
fn read_index_title(dir: &Path) -> Option<String> {
  for name in ["index.md", "index.mdx"] {
    let p = dir.join(name);
    if let Ok(content) = std::fs::read_to_string(&p) {
      if let Some(t) = extract_title_line(&content) {
        return Some(t);
      }
    }
  }
  None
}

/// 按 ref_kind/ref_path 反查标题；找不到则用 path 兜底
#[cfg(feature = "server")]
fn resolve_ref_title(kind: &str, path: &str) -> String {
  let root = get_asset_root();
  let fallback = || path.to_string();
  match kind {
    "blog" => read_index_title(&root.join("posts").join(path)).unwrap_or_else(fallback),
    "doc" => read_index_title(&root.join("docs").join(path)).unwrap_or_else(fallback),
    "course" => {
      let yaml = root.join("courses").join(path).join("course.yaml");
      if let Ok(content) = std::fs::read_to_string(&yaml) {
        for line in content.lines() {
          let line = line.trim();
          if let Some(value) = line.strip_prefix("title:") {
            let t = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if !t.is_empty() {
              return t.to_string();
            }
          }
        }
      }
      fallback()
    }
    "lesson" => {
      let parts: Vec<&str> = path.split('/').collect();
      if parts.len() == 3 {
        let lesson_dir = root.join("courses").join(parts[0]).join(parts[1]).join(parts[2]);
        if let Some(t) = read_index_title(&lesson_dir) {
          return t;
        }
      }
      fallback()
    }
    "case" => {
      let yaml = root.join("cases").join(path).join("case.yaml");
      if let Ok(content) = std::fs::read_to_string(&yaml) {
        for line in content.lines() {
          let line = line.trim();
          if let Some(value) = line.strip_prefix("name:") {
            let t = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if !t.is_empty() {
              return t.to_string();
            }
          }
        }
      }
      fallback()
    }
    _ => fallback(),
  }
}

#[cfg(feature = "server")]
fn build_topic_ref(kind: Option<String>, path: Option<String>) -> Option<TopicRef> {
  match (kind, path) {
    (Some(k), Some(p)) if !k.is_empty() && !p.is_empty() => {
      let title = resolve_ref_title(&k, &p);
      Some(TopicRef { kind: k, path: p, title })
    }
    _ => None,
  }
}

#[cfg(feature = "server")]
fn current_session_user() -> Option<rustineverything_core::session::SessionUser> {
  use dioxus::fullstack::FullstackContext;
  use rustineverything_core::session::parse_session_from_cookie_header;

  let ctx = FullstackContext::current()?;
  let parts = ctx.parts_mut();
  let cookie_str = parts.headers.get("cookie").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
  drop(parts);
  parse_session_from_cookie_header(cookie_str.as_deref())
}

#[cfg(feature = "server")]
fn require_session() -> Result<rustineverything_core::session::SessionUser, ServerFnError> {
  current_session_user().ok_or_else(|| ServerFnError::new("请先登录".to_string()))
}

#[cfg(feature = "server")]
async fn open_db() -> Result<sea_orm::DatabaseConnection, ServerFnError> {
  rustineverything_core::db::get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg(feature = "server")]
fn fmt_dt(dt: chrono::DateTime<chrono::FixedOffset>) -> String {
  dt.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(feature = "server")]
fn topic_to_summary(
  t: rustineverything_core::entities::topic::Model,
  author: Option<&rustineverything_core::entities::user::Model>,
) -> TopicSummary {
  let reference = build_topic_ref(t.ref_kind.clone(), t.ref_path.clone());
  TopicSummary {
    id: t.id,
    title: t.title,
    tag: t.tag,
    author: author.map(|u| u.nickname.clone()).unwrap_or_else(|| "已注销".to_string()),
    author_avatar: author.and_then(|u| u.avatar_url.clone()),
    user_id: t.user_id,
    reply_count: t.reply_count,
    last_reply_at: t.last_reply_at.map(fmt_dt),
    created_at: fmt_dt(t.created_at),
    reference,
  }
}

// =============================================================
// Server functions
// =============================================================

const PAGE_SIZE: u64 = 20;

#[post("/api/topics/list")]
pub async fn list_topics(
  tag: Option<String>,
  page: Option<u32>,
) -> Result<Vec<TopicSummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{topic, user as user_entity};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
    let db = open_db().await?;
    let page = page.unwrap_or(0) as u64;

    let mut q = topic::Entity::find();
    if let Some(t) = tag.as_ref() {
      if !t.is_empty() {
        q = q.filter(topic::Column::Tag.eq(t));
      }
    }
    let q = q.order_by_desc(topic::Column::LastReplyAt).order_by_desc(topic::Column::CreatedAt);

    let rows: Vec<topic::Model> = q
      .paginate(&db, PAGE_SIZE)
      .fetch_page(page)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 收集 user_id 一次性查 nickname/avatar
    let mut user_ids: Vec<i32> = rows.iter().map(|r| r.user_id).collect();
    user_ids.sort();
    user_ids.dedup();

    let users = if user_ids.is_empty() {
      vec![]
    } else {
      user_entity::Entity::find()
        .filter(user_entity::Column::Id.is_in(user_ids))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let user_map: std::collections::HashMap<i32, rustineverything_core::entities::user::Model> =
      users.into_iter().map(|u| (u.id, u)).collect();

    Ok(
      rows
        .into_iter()
        .map(|t| {
          let author = user_map.get(&t.user_id);
          topic_to_summary(t, author)
        })
        .collect(),
    )
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (tag, page);
    Ok(vec![])
  }
}

#[post("/api/topics/list-by-ref")]
pub async fn list_topics_by_ref(
  kind: String,
  path: String,
) -> Result<Vec<TopicSummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{topic, user as user_entity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    if kind.is_empty() || path.is_empty() {
      return Ok(vec![]);
    }
    let db = open_db().await?;
    let rows: Vec<topic::Model> = topic::Entity::find()
      .filter(topic::Column::RefKind.eq(&kind))
      .filter(topic::Column::RefPath.eq(&path))
      .order_by_desc(topic::Column::LastReplyAt)
      .order_by_desc(topic::Column::CreatedAt)
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut user_ids: Vec<i32> = rows.iter().map(|r| r.user_id).collect();
    user_ids.sort();
    user_ids.dedup();
    let users = if user_ids.is_empty() {
      vec![]
    } else {
      user_entity::Entity::find()
        .filter(user_entity::Column::Id.is_in(user_ids))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let user_map: std::collections::HashMap<i32, rustineverything_core::entities::user::Model> =
      users.into_iter().map(|u| (u.id, u)).collect();

    Ok(
      rows
        .into_iter()
        .map(|t| {
          let author = user_map.get(&t.user_id).cloned();
          topic_to_summary(t, author.as_ref())
        })
        .collect(),
    )
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (kind, path);
    Ok(vec![])
  }
}

#[post("/api/topics/tags")]
pub async fn list_tags() -> Result<Vec<TagSummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::topic;
    use sea_orm::{ColumnTrait, EntityTrait, FromQueryResult, Order, QueryOrder, QuerySelect};

    #[derive(FromQueryResult)]
    struct TagRow {
      tag: String,
      cnt: i64,
    }

    let db = open_db().await?;
    let rows = topic::Entity::find()
      .select_only()
      .column(topic::Column::Tag)
      .column_as(topic::Column::Id.count(), "cnt")
      .group_by(topic::Column::Tag)
      .order_by(topic::Column::Id.count(), Order::Desc)
      .into_model::<TagRow>()
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| TagSummary { tag: r.tag, topic_count: r.cnt }).collect())
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

#[post("/api/topics/get")]
pub async fn get_topic(id: i32) -> Result<Option<TopicDetail>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{topic, topic_reply, user as user_entity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let db = open_db().await?;
    let Some(t) = topic::Entity::find_by_id(id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
    else {
      return Ok(None);
    };

    // 加载作者
    let author = user_entity::Entity::find_by_id(t.user_id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 加载回复
    let reply_rows = topic_reply::Entity::find()
      .filter(topic_reply::Column::TopicId.eq(id))
      .order_by_asc(topic_reply::Column::CreatedAt)
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 批量取回复者
    let mut reply_user_ids: Vec<i32> = reply_rows.iter().map(|r| r.user_id).collect();
    reply_user_ids.sort();
    reply_user_ids.dedup();
    let reply_users = if reply_user_ids.is_empty() {
      vec![]
    } else {
      user_entity::Entity::find()
        .filter(user_entity::Column::Id.is_in(reply_user_ids))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let reply_user_map: std::collections::HashMap<
      i32,
      rustineverything_core::entities::user::Model,
    > = reply_users.into_iter().map(|u| (u.id, u)).collect();

    let replies: Vec<Reply> = reply_rows
      .into_iter()
      .map(|r| {
        let u = reply_user_map.get(&r.user_id);
        Reply {
          id: r.id,
          content: r.content,
          author: u.map(|u| u.nickname.clone()).unwrap_or_else(|| "已注销".to_string()),
          author_avatar: u.and_then(|u| u.avatar_url.clone()),
          user_id: r.user_id,
          created_at: fmt_dt(r.created_at),
        }
      })
      .collect();

    let reference = build_topic_ref(t.ref_kind.clone(), t.ref_path.clone());

    Ok(Some(TopicDetail {
      id: t.id,
      title: t.title,
      tag: t.tag,
      content: t.content,
      author: author.as_ref().map(|u| u.nickname.clone()).unwrap_or_else(|| "已注销".to_string()),
      author_avatar: author.as_ref().and_then(|u| u.avatar_url.clone()),
      user_id: t.user_id,
      created_at: fmt_dt(t.created_at),
      updated_at: fmt_dt(t.updated_at),
      reference,
      replies,
    }))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Ok(None)
  }
}

#[post("/api/topics/create")]
pub async fn create_topic(input: NewTopicInput) -> Result<TopicSummary, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use chrono::Utc;
    use rustineverything_core::entities::topic;
    use sea_orm::{ActiveValue::Set, EntityTrait};

    validate_new_topic(&input).map_err(ServerFnError::new)?;
    let user = require_session()?;

    // ── 审核：标题 + 正文一起评估 ──
    let combined = format!("标题：{}\n\n{}", input.title.trim(), input.content);
    let ref_path = format!("topic-new:{}", input.tag);
    let outcome = moderate_or_reject(&combined, "topic", &ref_path).await?;

    let db = open_db().await?;
    let now = Utc::now().fixed_offset();
    let normalized_tag = normalize_tag(&input.tag);
    let title = input.title.trim().to_string();

    let am = topic::ActiveModel {
      title: Set(title),
      tag: Set(normalized_tag),
      content: Set(input.content),
      user_id: Set(user.id),
      reply_count: Set(0),
      last_reply_at: Set(None),
      ref_kind: Set(input.ref_kind.clone()),
      ref_path: Set(input.ref_path.clone()),
      created_at: Set(now),
      updated_at: Set(now),
      ..Default::default()
    };
    let inserted = topic::Entity::insert(am)
      .exec_with_returning(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Flag → 入审核队列
    enqueue_after_insert(&db, &outcome, "topic", inserted.id as i64, &ref_path, user.id, &combined)
      .await;

    let author = rustineverything_core::entities::user::Entity::find_by_id(user.id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(topic_to_summary(inserted, author.as_ref()))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = input;
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[post("/api/topics/reply")]
pub async fn post_reply(topic_id: i32, content: String) -> Result<TopicDetail, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use chrono::Utc;
    use rustineverything_core::entities::{topic, topic_reply};
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

    validate_new_reply(&content).map_err(ServerFnError::new)?;
    let user = require_session()?;

    // ── 审核 ──
    let ref_path = format!("topic:{}", topic_id);
    let outcome = moderate_or_reject(&content, "reply", &ref_path).await?;

    let db = open_db().await?;

    // 必须存在该话题
    let _ = topic::Entity::find_by_id(topic_id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
      .ok_or_else(|| ServerFnError::new("话题不存在".to_string()))?;

    let now = Utc::now().fixed_offset();
    let txn = db.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let reply_am = topic_reply::ActiveModel {
      topic_id: Set(topic_id),
      user_id: Set(user.id),
      content: Set(content.clone()),
      created_at: Set(now),
      ..Default::default()
    };
    let inserted_reply = topic_reply::Entity::insert(reply_am)
      .exec_with_returning(&txn)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 更新 topic 计数与最近回复时间
    use sea_orm::sea_query::Expr;
    topic::Entity::update_many()
      .col_expr(topic::Column::ReplyCount, Expr::col(topic::Column::ReplyCount).add(1))
      .col_expr(topic::Column::LastReplyAt, Expr::value(now))
      .col_expr(topic::Column::UpdatedAt, Expr::value(now))
      .filter(topic::Column::Id.eq(topic_id))
      .exec(&txn)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    txn.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // 事务提交后再入队（队列写失败不应回滚业务）
    enqueue_after_insert(
      &db,
      &outcome,
      "reply",
      inserted_reply.id as i64,
      &ref_path,
      user.id,
      &content,
    )
    .await;

    // 返回最新详情
    get_topic(topic_id).await?.ok_or_else(|| ServerFnError::new("话题不存在".to_string()))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (topic_id, content);
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[post("/api/topics/mine")]
pub async fn list_my_topics() -> Result<Vec<TopicSummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{topic, user as user_entity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let user = match current_session_user() {
      Some(u) => u,
      None => return Ok(vec![]),
    };
    let db = open_db().await?;
    let rows = topic::Entity::find()
      .filter(topic::Column::UserId.eq(user.id))
      .order_by_desc(topic::Column::CreatedAt)
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    let me = user_entity::Entity::find_by_id(user.id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|t| topic_to_summary(t, me.as_ref())).collect())
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

// =============================================================
// Tests
// =============================================================

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;

  fn ok_input() -> NewTopicInput {
    NewTopicInput {
      title: "Hello forum".to_string(),
      tag: "general".to_string(),
      content: "讨论 Rust 与 Dioxus 的最佳实践。".to_string(),
      ref_kind: None,
      ref_path: None,
    }
  }

  #[test]
  fn normalize_tag_basic() {
    assert_eq!(normalize_tag("  Rust  "), "rust");
    assert_eq!(normalize_tag("rust-lang"), "rust-lang");
    assert_eq!(normalize_tag("rust_lang"), "rust_lang");
    assert_eq!(normalize_tag("Hello, World!"), "helloworld");
  }

  #[test]
  fn normalize_tag_truncates_to_max_len() {
    let raw = "a".repeat(MAX_TAG_LEN + 50);
    let out = normalize_tag(&raw);
    assert_eq!(out.len(), MAX_TAG_LEN);
  }

  #[test]
  fn normalize_tag_drops_non_ascii() {
    // 非 ASCII 字符（中文）应被剥除
    assert_eq!(normalize_tag("讨论"), "");
    assert_eq!(normalize_tag("rust讨论"), "rust");
  }

  #[test]
  fn validate_new_topic_ok() {
    assert!(validate_new_topic(&ok_input()).is_ok());
  }

  #[test]
  fn validate_new_topic_empty_title() {
    let mut i = ok_input();
    i.title = "   ".to_string();
    assert!(validate_new_topic(&i).is_err());
  }

  #[test]
  fn validate_new_topic_title_too_long() {
    let mut i = ok_input();
    i.title = "x".repeat(MAX_TITLE_LEN + 1);
    let err = validate_new_topic(&i).unwrap_err();
    assert!(err.contains("标题"));
  }

  #[test]
  fn validate_new_topic_empty_tag() {
    let mut i = ok_input();
    i.tag = "  ".to_string();
    assert!(validate_new_topic(&i).is_err());
  }

  #[test]
  fn validate_new_topic_tag_only_invalid_chars_rejected() {
    let mut i = ok_input();
    i.tag = "中文标签".to_string();
    // 规整后为空 → 校验失败
    assert!(validate_new_topic(&i).is_err());
  }

  #[test]
  fn validate_new_topic_tag_with_mixed_chars_passes() {
    let mut i = ok_input();
    i.tag = "Rust 入门".to_string();
    // 规整保留 "rust"
    assert!(validate_new_topic(&i).is_ok());
  }

  #[test]
  fn validate_new_topic_blank_content() {
    let mut i = ok_input();
    i.content = "\n\t   \n".to_string();
    assert!(validate_new_topic(&i).is_err());
  }

  #[test]
  fn validate_new_topic_invalid_ref_kind() {
    let mut i = ok_input();
    i.ref_kind = Some("twitter".to_string());
    i.ref_path = Some("foo".to_string());
    assert!(validate_new_topic(&i).is_err());
  }

  #[test]
  fn validate_new_topic_partial_ref_rejected() {
    let mut i = ok_input();
    i.ref_kind = Some("blog".to_string());
    i.ref_path = None;
    assert!(validate_new_topic(&i).is_err());
  }

  #[test]
  fn validate_new_topic_full_ref_ok() {
    let mut i = ok_input();
    i.ref_kind = Some("blog".to_string());
    i.ref_path = Some("hello-rust".to_string());
    assert!(validate_new_topic(&i).is_ok());
  }

  #[test]
  fn validate_new_reply_basic() {
    assert!(validate_new_reply("good point").is_ok());
    assert!(validate_new_reply("   ").is_err());
    assert!(validate_new_reply("").is_err());
  }

  #[test]
  fn validate_new_reply_too_long() {
    let big = "x".repeat(MAX_REPLY_LEN + 1);
    assert!(validate_new_reply(&big).is_err());
  }

  #[test]
  fn extract_title_from_frontmatter() {
    let raw = "---\ntitle: Hello\n---\n\n# body\n";
    assert_eq!(extract_title_line(raw).as_deref(), Some("Hello"));
  }

  #[test]
  fn extract_title_falls_back_to_h1() {
    let raw = "no fm\n\n# Heading 1\nrest";
    assert_eq!(extract_title_line(raw).as_deref(), Some("Heading 1"));
  }

  #[test]
  fn extract_title_quoted_value() {
    let raw = "---\ntitle: \"Quoted Title\"\n---\nbody";
    assert_eq!(extract_title_line(raw).as_deref(), Some("Quoted Title"));
  }

  #[test]
  fn extract_title_none() {
    assert!(extract_title_line("just plain text").is_none());
  }
}

// =============================================================
// 审核 helper（仅 server feature）
// =============================================================

/// 给 `create_topic` / `post_reply` 复用的审核结果，便于复核入队。
#[cfg(feature = "server")]
struct ModerationOutcome {
  verdict: rustineverything_core::engines::moderation::Verdict,
  image_urls: Vec<String>,
}

/// 在写库前评估内容：Block → ServerFnError，Allow / Flag → 返回 outcome 让
/// 调用方在业务行落库后调 [`enqueue_after_insert`] 把 Flag 入审核队列。
#[cfg(feature = "server")]
async fn moderate_or_reject(
  content: &str,
  kind: &str,
  ref_path: &str,
) -> Result<ModerationOutcome, ServerFnError> {
  use rustineverything_module_moderation::{
    absolutize_image_url, evaluate_submission, extract_image_urls, ModerationLabel,
  };
  use rustineverything_sdk::{ImageRef, ModerationSubmission};

  let base_url = std::env::var("BASE_URL").unwrap_or_default();
  let image_urls: Vec<String> =
    extract_image_urls(content).into_iter().map(|u| absolutize_image_url(&u, &base_url)).collect();
  let image_refs: Vec<ImageRef> = image_urls.iter().cloned().map(ImageRef::url).collect();
  let submission = ModerationSubmission::new(content)
    .with_kind(kind)
    .with_ref_path(ref_path)
    .with_images(image_refs);
  let verdict = evaluate_submission(submission).await;
  match verdict.label {
    ModerationLabel::Block => {
      tracing::warn!(
          kind = %kind,
          ref_path = %ref_path,
          score = verdict.score,
          reason = %verdict.reason,
          "moderation: forum submission BLOCKED"
      );
      Err(ServerFnError::new(format!(
        "提交被审核拒绝：{}",
        if verdict.reason.is_empty() {
          "未通过内容审核".to_string()
        } else {
          verdict.reason
        }
      )))
    }
    _ => Ok(ModerationOutcome { verdict, image_urls }),
  }
}

/// 业务行落库后调用：Flag 入队，Allow no-op。
#[cfg(feature = "server")]
async fn enqueue_after_insert(
  db: &sea_orm::DatabaseConnection,
  outcome: &ModerationOutcome,
  kind: &str,
  ref_id: i64,
  ref_path: &str,
  user_id: i32,
  content: &str,
) {
  rustineverything_module_moderation::enqueue_if_flagged(
    db,
    &outcome.verdict,
    kind,
    Some(ref_id),
    ref_path,
    Some(user_id),
    content,
    &outcome.image_urls,
  )
  .await;
}
