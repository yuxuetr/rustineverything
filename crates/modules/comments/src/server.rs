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
        use rustineverything_core::db::get_or_init_pool;
        use rustineverything_core::entities::{comment, user};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let db = get_or_init_pool()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

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
                author: u
                    .as_ref()
                    .map(|u| u.nickname.clone())
                    .unwrap_or_else(|| "已注销".to_string()),
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
pub async fn post_comment(
    blog_id: String,
    content: String,
) -> Result<Vec<Comment>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use rustineverything_core::db::get_or_init_pool;
        use rustineverything_core::entities::comment;
        use rustineverything_core::session::current_session_user;
        use sea_orm::{EntityTrait, Set};

        let session_user = current_session_user()
            .ok_or_else(|| ServerFnError::new("请先登录后再发表评论"))?;

        let db = get_or_init_pool()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let new_comment = comment::ActiveModel {
            blog_id: Set(blog_id.clone()),
            user_id: Set(session_user.id),
            content: Set(content),
            created_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        };
        comment::Entity::insert(new_comment)
            .exec(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        get_comments(blog_id).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (blog_id, content);
        Ok(vec![])
    }
}
