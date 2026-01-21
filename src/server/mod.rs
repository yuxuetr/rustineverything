use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
  pub id: String,
  pub blog_id: String,
  pub content: String,
  pub author: String,
  pub date: String,
}

#[post("/api/comments")]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError> {
  let path = "public/data/comments.json";
  if !tokio::fs::try_exists(path).await.unwrap_or(false) {
    return Ok(Vec::new());
  }

  let content = tokio::fs::read_to_string(path)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to read comments: {}", e)))?;

  let comments: Vec<Comment> = serde_json::from_str(&content).unwrap_or_default();

  // Sort by date descending (newest first)
  let mut filtered: Vec<Comment> = comments
    .into_iter()
    .filter(|c| c.blog_id == blog_id)
    .collect();
  filtered.reverse();
  Ok(filtered)
}

#[post("/api/comments/post")]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError> {
    // Check if we should use a different path to avoid watcher
  let db_path = if tokio::fs::try_exists("public/data").await.unwrap_or(false) {
    "public/data/comments.json"
  } else {
    // Fallback or create if not exists
    if let Err(_) = tokio::fs::create_dir_all("public/data").await {
      // Log error
    }
    "public/data/comments.json"
  };

  let mut comments: Vec<Comment> = if tokio::fs::try_exists(db_path).await.unwrap_or(false) {
    let c = tokio::fs::read_to_string(db_path)
      .await
      .unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&c).unwrap_or_default()
  } else {
    // Try reading from old path for migration? Or just start fresh.
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
    .map_err(|e| ServerFnError::new(format!("Failed to serialize comments: {}", e)))?;

  tokio::fs::write(db_path, json)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to save comments: {}", e)))?;

  // Return updated list for this blog
  get_comments(blog_id).await
}

#[post("/api/upload")]
pub async fn upload_image(name: String, data_base64: String) -> Result<String, ServerFnError> {
  use base64::Engine as _;

  println!(
    "Server: upload_image called with name: {}, data_len: {}",
    name,
    data_base64.len()
  );
  if let Ok(cwd) = std::env::current_dir() {
    println!("Server: Current working directory: {:?}", cwd);
  }

  // Remove header if present (e.g., "data:image/png;base64,")
  let base64_str = data_base64.split(',').last().unwrap_or(&data_base64);

  let data = base64::engine::general_purpose::STANDARD
    .decode(base64_str)
    .map_err(|e| {
      println!("Server: Decode error: {}", e);
      ServerFnError::new(format!("Failed to decode base64: {}", e))
    })?;

  let filename = format!("{}_{}", chrono::Utc::now().timestamp(), name);
  // Ensure directory exists
  let dir_path = "public/uploads";
  if let Err(e) = tokio::fs::create_dir_all(dir_path).await {
    println!("Server: Failed to create directory {}: {}", dir_path, e);
    return Err(ServerFnError::new(format!(
      "Failed to create directory: {}",
      e
    )));
  }

  let path = format!("{}/{}", dir_path, filename);
  println!("Server: Writing to path: {}", path);

  if let Err(e) = tokio::fs::write(&path, &data).await {
    println!("Server: Write error: {}", e);
    return Err(ServerFnError::new(format!("Failed to write file: {}", e)));
  }

  println!(
    "Server: Successfully wrote {} bytes to {}",
    data.len(),
    path
  );
  Ok(format!("/uploads/{}", filename))
}

/// Echo the user input on the server.
///
/// This function is exposed as a POST endpoint at `/api/echo`.
/// On the client (web/desktop), calling `echo_server` will perform
/// an HTTP request to this endpoint and return the echoed value.
#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> {
  Ok(input)
}

#[post("/api/content/blog")]
pub async fn get_blog_content(id: String) -> Result<String, ServerFnError> {
  let filepath = match id.as_str() {
    "1" => "assets/content/welcome.md".to_string(),
    "2" => "assets/blog/2026-01-10-python-struct/index.mdx".to_string(),
    _ => return Err(ServerFnError::new("Blog post not found")),
  };

  tokio::fs::read_to_string(&filepath)
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to read post: {}", e)))
}
