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

#[post("/api/comments/post")]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use chrono::Utc;
    use app_core::db::get_or_init_pool;
    use app_core::entities::comment;
    use app_core::session::current_session_user;
    use module_moderation::{
      absolutize_image_url, enqueue_if_flagged, evaluate_submission, extract_image_urls,
      ModerationLabel,
    };
    use sdk::{ImageRef, ModerationSubmission};
    use sea_orm::{ActiveModelTrait, Set};

    let session_user =
      current_session_user().ok_or_else(|| ServerFnError::new("请先登录后再发表评论"))?;

    // ── 审核（默认 disabled → pipeline 为空 → 直接 Allow，零开销） ──
    let base_url = std::env::var("BASE_URL").unwrap_or_default();
    let raw_image_urls: Vec<String> = extract_image_urls(&content)
      .into_iter()
      .map(|u| absolutize_image_url(&u, &base_url))
      .collect();
    let image_refs: Vec<ImageRef> = raw_image_urls.iter().cloned().map(ImageRef::url).collect();
    let ref_path = format!("blog:{}", blog_id);
    let submission = ModerationSubmission::new(&content)
      .with_kind("comment")
      .with_ref_path(&ref_path)
      .with_images(image_refs);
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

    let new_comment = comment::ActiveModel {
      blog_id: Set(blog_id.clone()),
      user_id: Set(session_user.id),
      content: Set(content.clone()),
      created_at: Set(Utc::now().fixed_offset()),
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
      &raw_image_urls,
    )
    .await;

    get_comments(blog_id).await
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (blog_id, content);
    Ok(vec![])
  }
}
