//! 请求体大小上限：axum 默认 2 MiB，只给图片上传放宽。
//!
//! server fn 的请求体由 dioxus-fullstack 0.7.9 用 `Bytes::from_request(..).unwrap()`
//! 读取，超过上限时直接 panic、回 500，而不是 413。`/api/upload` 允许 5 MB 图片，
//! 经 base64 再包进 JSON 后远超 2 MiB，所以这条路径的上限要跟上传上限对齐。

use axum::extract::{DefaultBodyLimit, Request};
use axum::middleware::Next;
use axum::response::Response;
use tower::{Layer, ServiceExt};

/// 图片上传的 server fn 路径（`module_uploads::server::upload_image`）。
const UPLOAD_PATH: &str = "/api/upload";

/// 5 MB 图片经 base64（×4/3）约 6.7 MB，再加 JSON 外壳；取 8 MiB。
const UPLOAD_BODY_LIMIT: usize = 8 * 1024 * 1024;

/// 只对 `/api/upload` 放宽请求体上限，其余路径保持 axum 默认的 2 MiB。
pub async fn upload_body_limit_mw(req: Request, next: Next) -> Response {
  if req.uri().path() != UPLOAD_PATH {
    return next.run(req).await;
  }
  match DefaultBodyLimit::max(UPLOAD_BODY_LIMIT).layer(next).oneshot(req).await {
    Ok(res) => res,
    Err(never) => match never {},
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use axum::body::{Body, Bytes};
  use axum::http::StatusCode;
  use axum::routing::post;
  use axum::Router;

  fn app() -> Router {
    let echo_len = |body: Bytes| async move { body.len().to_string() };
    Router::new()
      .route(UPLOAD_PATH, post(echo_len))
      .route("/api/other", post(echo_len))
      .layer(axum::middleware::from_fn(upload_body_limit_mw))
  }

  async fn post_bytes(path: &str, len: usize) -> StatusCode {
    let req = Request::post(path).body(Body::from(vec![b'a'; len])).unwrap();
    app().oneshot(req).await.unwrap().status()
  }

  #[tokio::test]
  async fn upload_accepts_bodies_above_the_default_limit() {
    assert_eq!(post_bytes(UPLOAD_PATH, 3 * 1024 * 1024).await, StatusCode::OK);
    assert_eq!(post_bytes(UPLOAD_PATH, UPLOAD_BODY_LIMIT + 1).await, StatusCode::PAYLOAD_TOO_LARGE);
  }

  #[tokio::test]
  async fn other_paths_keep_the_default_limit() {
    assert_eq!(post_bytes("/api/other", 3 * 1024 * 1024).await, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(post_bytes("/api/other", 1024).await, StatusCode::OK);
  }
}
