use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;

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
