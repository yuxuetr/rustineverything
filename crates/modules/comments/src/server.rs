use dioxus::fullstack::{post, ServerFnError};
#[allow(unused_imports)]
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
  pub id: String,
  pub blog_id: String,
  pub content: String,
  pub author: String,
  pub author_avatar: Option<String>,
  pub user_id: Option<i32>,
  pub date: String,
}

/// 发评论的结果：最新的公开评论列表，以及刚发的这条是不是在等人工审核（等审的不在列表里）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostedComment {
  pub comments: Vec<Comment>,
  pub pending_review: bool,
}

/// 评论长度上限，与论坛回复一致（Y3）。
pub const MAX_COMMENT_LEN: usize = 32 * 1024;

/// 校验评论内容
pub fn validate_comment(content: &str) -> Result<(), String> {
  if content.trim().is_empty() {
    return Err("评论内容不能为空".to_string());
  }
  if content.len() > MAX_COMMENT_LEN {
    return Err(format!("评论长度不能超过 {} 字节", MAX_COMMENT_LEN));
  }
  Ok(())
}

#[post("/api/comments/list")]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::db::get_or_init_pool;
    use app_core::entities::{comment, user};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let db = get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let results = comment::Entity::find()
      .filter(comment::Column::BlogId.eq(&blog_id))
      .filter(comment::Column::PendingReview.eq(false))
      .find_also_related(user::Entity)
      .order_by_desc(comment::Column::CreatedAt)
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    let comments = results
      .into_iter()
      .map(|(c, u)| Comment {
        id: c.id.to_string(),
        blog_id: c.blog_id,
        content: c.content,
        author: u.as_ref().map(|u| u.nickname.clone()).unwrap_or_else(|| "已注销".to_string()),
        author_avatar: u.as_ref().and_then(|u| u.avatar_url.clone()),
        user_id: Some(c.user_id),
        date: c.created_at.format("%Y-%m-%d %H:%M").to_string(),
      })
      .collect();

    Ok(comments)
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = blog_id;
    Ok(vec![])
  }
}

/// Phase 7.5 debug 习惯（详见 `docs/DEVELOPER.md §3.5`）：用
/// `#[tracing::instrument]` 自动记录入口 + 退出 + elapsed；`content` 走
/// skip 避免评论原文泄漏到日志，仅留 `content_len` 字段供联调；`blog_id`
/// 是资源 id 不敏感，直接入字段。
#[cfg_attr(
  feature = "server",
  tracing::instrument(
    name = "server::post_comment",
    level = "info",
    skip(content),
    fields(blog_id = %blog_id, content_len = content.len()),
    err
  )
)]
#[post("/api/comments/post")]
pub async fn post_comment(
  blog_id: String,
  content: String,
) -> Result<PostedComment, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::db::get_or_init_pool;
    use app_core::entities::comment;
    use app_core::session::require_session_verified;
    use chrono::Utc;
    use module_moderation::{
      enqueue_if_flagged, evaluate_submission, external_image_urls, ModerationLabel,
    };
    use sdk::{ImageRef, ModerationSubmission};
    use sea_orm::{ActiveModelTrait, Set};

    // S4：写路径用 verified 会话（DB 回查 token_version，支持即时吊销）
    let session_user = require_session_verified().await?;
    validate_comment(&content).map_err(ServerFnError::new)?;

    // ── 审核（默认 disabled → pipeline 为空 → 直接 Allow，零开销） ──
    let base_url = std::env::var("BASE_URL").unwrap_or_default();
    let image_urls = external_image_urls(&content, &base_url).map_err(ServerFnError::new)?;
    let ref_path = format!("blog:{}", blog_id);
    let submission = ModerationSubmission::new(&content)
      .with_kind("comment")
      .with_ref_path(&ref_path)
      .with_images(image_urls.iter().cloned().map(ImageRef::url));
    let verdict = evaluate_submission(submission).await;
    if verdict.label == ModerationLabel::Block {
      tracing::warn!(
          user = %session_user.nickname,
          blog_id = %blog_id,
          score = verdict.score,
          reason = %verdict.reason,
          "moderation: comment BLOCKED"
      );
      return Err(ServerFnError::new(format!(
        "评论被审核拒绝：{}",
        if verdict.reason.is_empty() {
          "未通过内容审核".to_string()
        } else {
          verdict.reason
        }
      )));
    }

    let db = get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Flag：先存下但不公开，等 admin 在审核队列里通过
    let pending_review = verdict.label == ModerationLabel::Flag;
    let new_comment = comment::ActiveModel {
      blog_id: Set(blog_id.clone()),
      user_id: Set(session_user.id),
      content: Set(content.clone()),
      created_at: Set(Utc::now().fixed_offset()),
      pending_review: Set(pending_review),
      ..Default::default()
    };
    // `insert(...)` 返回新行 ActiveModel；用 id 入审核队列
    let inserted = new_comment.insert(&db).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Flag → 入审核队列（Allow / Block 都是 no-op）
    enqueue_if_flagged(
      &db,
      &verdict,
      "comment",
      Some(inserted.id as i64),
      &ref_path,
      Some(session_user.id),
      &content,
      &image_urls,
    )
    .await;

    Ok(PostedComment { comments: get_comments(blog_id).await?, pending_review })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (blog_id, content);
    Ok(PostedComment { comments: vec![], pending_review: false })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn validate_comment_rejects_empty_and_oversized() {
    assert!(validate_comment("好文").is_ok());
    assert!(validate_comment("  \n ").is_err());
    assert!(validate_comment(&"a".repeat(MAX_COMMENT_LEN)).is_ok());
    assert!(validate_comment(&"a".repeat(MAX_COMMENT_LEN + 1)).is_err());
  }
}
