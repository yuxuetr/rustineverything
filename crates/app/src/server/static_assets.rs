//! S7（风险 R8）：静态资源目录服务。从 `main.rs` 拆出，行为不变。

use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tower_http::services::ServeDir;

/// 挂载静态资源目录。`assets_root` 为资产根目录（启动期解析）。
pub fn mount(router: Router, assets_root: &str) -> Router {
  router
    .nest_service("/images", ServeDir::new(format!("{}/images", assets_root)))
    .nest_service("/posts", ServeDir::new(format!("{}/posts", assets_root)))
    // 脚本文件名不带哈希：每次使用前向服务端确认（未变时 304），改版后立即生效
    .nest(
      "/js",
      Router::new()
        .fallback_service(ServeDir::new(format!("{}/js", assets_root)))
        .layer(middleware::map_response(cache_control("no-cache"))),
    )
    .nest_service("/uploads", ServeDir::new(format!("{}/uploads", assets_root)))
    .nest_service("/audio", ServeDir::new(format!("{}/audio", assets_root)))
    .nest_service("/podcasts", ServeDir::new(format!("{}/podcasts", assets_root)))
    .nest(
      "/courses",
      Router::new()
        .fallback_service(ServeDir::new(format!("{}/courses", assets_root)))
        .layer(middleware::from_fn(guard_course_file))
        // 按用户鉴权的内容：共享缓存（CDN / 反代）不得存，浏览器每次复验
        .layer(middleware::map_response(cache_control("private, no-cache"))),
    )
    .nest_service("/cases", ServeDir::new(format!("{}/cases", assets_root)))
    .nest_service("/assets/font", ServeDir::new(format!("{}/font", assets_root)))
}

/// 给响应设置固定的 `Cache-Control`
fn cache_control(value: &'static str) -> impl Fn(Response) -> std::future::Ready<Response> + Clone {
  move |mut res: Response| {
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    std::future::ready(res)
  }
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

#[cfg(test)]
mod tests {
  use axum::body::Body;
  use axum::http::{header, Request, StatusCode};
  use tower::ServiceExt;

  async fn get(assets_root: &str, uri: &str) -> axum::response::Response {
    super::mount(axum::Router::new(), assets_root)
      .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
      .await
      .unwrap()
  }

  fn cache_control(res: &axum::response::Response) -> Option<&str> {
    res.headers().get(header::CACHE_CONTROL).and_then(|v| v.to_str().ok())
  }

  /// 脚本无哈希文件名，不带缓存头时浏览器按启发式缓存，改版后可能长时间拿旧版。
  #[tokio::test]
  async fn js_must_revalidate() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("js")).unwrap();
    std::fs::write(tmp.path().join("js/a.js"), "1").unwrap();
    let res = get(tmp.path().to_str().unwrap(), "/js/a.js").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(cache_control(&res), Some("no-cache"));
  }

  /// 课时文件按用户鉴权，共享缓存（CDN / 反代）不得存，否则会把付费内容发给未购用户。
  #[tokio::test]
  async fn course_files_are_private() {
    let tmp = tempfile::TempDir::new().unwrap();
    let res = get(tmp.path().to_str().unwrap(), "/courses/nope/01-ch/01-le/index.md").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(cache_control(&res), Some("private, no-cache"));
  }
}
