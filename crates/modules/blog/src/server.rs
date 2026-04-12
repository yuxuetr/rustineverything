use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
  pub id: String,
  pub blog_id: String,
  pub content: String,
  pub author: String,
  pub date: String,
}

#[server]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError> {
  let path = "assets/data/comments.json";
  if !std::path::Path::new(path).exists() {
    return Ok(Vec::new());
  }

  let content = fs::read_to_string(path)
    .map_err(|e| ServerFnError::new(e.to_string()))?;

  let comments: Vec<Comment> = serde_json::from_str(&content).unwrap_or_default();

  let mut filtered: Vec<Comment> = comments
    .into_iter()
    .filter(|c| c.blog_id == blog_id)
    .collect();
  filtered.reverse();
  Ok(filtered)
}

#[server]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError> {
  let db_dir = "assets/data";
  let db_path = "assets/data/comments.json";
  
  if !std::path::Path::new(db_dir).exists() {
      fs::create_dir_all(db_dir).map_err(|e| ServerFnError::new(e.to_string()))?;
  }

  let mut comments: Vec<Comment> = if std::path::Path::new(db_path).exists() {
    let c = fs::read_to_string(db_path).unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&c).unwrap_or_default()
  } else {
    Vec::new()
  };

  let new_comment = Comment {
    id: chrono::Utc::now().timestamp_micros().to_string(),
    blog_id: blog_id.clone(),
    content,
    author: "访客".to_string(),
    date: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
  };

  comments.push(new_comment);

  let json = serde_json::to_string_pretty(&comments)
    .map_err(|e| ServerFnError::new(e.to_string()))?;

  fs::write(db_path, json)
    .map_err(|e| ServerFnError::new(e.to_string()))?;

  get_comments(blog_id).await
}

#[server]
pub async fn get_blog_content(id: String) -> Result<String, ServerFnError> {
  let filepath = match id.as_str() {
    "1" => "assets/content/welcome.md".to_string(),
    "2" => "assets/blog/2026-01-10-python-struct/index.mdx".to_string(),
    _ => return Err(ServerFnError::new("Blog post not found")),
  };

  fs::read_to_string(&filepath)
    .map_err(|e| ServerFnError::new(e.to_string()))
}
