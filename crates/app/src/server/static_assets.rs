//! S7（风险 R8）：静态资源目录服务。从 `main.rs` 拆出，行为不变。

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tower_http::services::ServeDir;

/// 挂载静态资源目录。`assets_root` 为资产根目录（启动期解析）。
pub fn mount(router: Router, assets_root: &str) -> Router {
  router
    .nest_service("/images", ServeDir::new(format!("{}/images", assets_root)))
    .nest_service("/posts", ServeDir::new(format!("{}/posts", assets_root)))
    .nest_service("/js", ServeDir::new(format!("{}/js", assets_root)))
    .nest_service("/uploads", ServeDir::new(format!("{}/uploads", assets_root)))
    .nest_service("/audio", ServeDir::new(format!("{}/audio", assets_root)))
    .nest_service("/podcasts", ServeDir::new(format!("{}/podcasts", assets_root)))
    .nest(
      "/courses",
      Router::new()
        .fallback_service(ServeDir::new(format!("{}/courses", assets_root)))
        .layer(middleware::from_fn(guard_course_file)),
    )
    .nest_service("/cases", ServeDir::new(format!("{}/cases", assets_root)))
    .nest_service("/assets/font", ServeDir::new(format!("{}/font", assets_root)))
}

/// SEC-02：课时目录里的正文 / 音视频 / 代码 / 附件与 `get_lesson` 同权限，
/// 无权时按不存在处理，不暴露付费课时的文件名。
async fn guard_course_file(req: Request, next: Next) -> Response {
  let cookie = req.headers().get(header::COOKIE).and_then(|v| v.to_str().ok());
  if module_course::server::may_serve_course_file(req.uri().path(), cookie).await {
    next.run(req).await
  } else {
    StatusCode::NOT_FOUND.into_response()
  }
}
