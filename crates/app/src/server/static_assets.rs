//! S7（风险 R8）：静态资源目录服务。从 `main.rs` 拆出，行为不变。

use axum::http::{header, HeaderValue};
use axum::middleware;
use axum::response::Response;
use axum::Router;
#[cfg(feature = "course")]
use axum::{extract::Request, http::StatusCode, middleware::Next, response::IntoResponse};
use tower_http::services::ServeDir;

/// 挂载静态资源目录。`assets_root` 为资产根目录（启动期解析）。
pub fn mount(router: Router, assets_root: &str) -> Router {
  // 文件名不带哈希：浏览器缓存一段时间，过期后靠 Last-Modified 复验（304）。
  // 文章图片 / 音频改动很少，一天；上传文件可能被审核撤下，一小时；字体基本不变，30 天。
  let router = router
    .nest("/images", cached(ServeDir::new(format!("{}/images", assets_root)), DAY))
    .nest("/posts", cached(ServeDir::new(app_core::utils::content_dir("posts")), DAY))
    // 文档卷里的图（卷首页的架构图等）。`/docs` 本身是页面路由，文件走单独的前缀。
    .nest("/docs-files", cached(ServeDir::new(app_core::utils::content_dir("docs")), DAY))
    // 只剩 annotations / mermaid 两个无哈希脚本：每次使用前向服务端确认（未变时 304），改版后立即生效
    .nest("/js", cached(ServeDir::new(format!("{}/js", assets_root)), "no-cache"))
    .nest(
      "/uploads",
      cached(ServeDir::new(format!("{}/uploads", assets_root)), "public, max-age=3600"),
    )
    .nest("/audio", cached(ServeDir::new(format!("{}/audio", assets_root)), DAY))
    .nest("/podcasts", cached(ServeDir::new(format!("{}/podcasts", assets_root)), DAY))
    .nest("/cases", cached(ServeDir::new(format!("{}/cases", assets_root)), DAY))
    .nest(
      "/assets/font",
      cached(ServeDir::new(format!("{}/font", assets_root)), "public, max-age=2592000"),
    );
  // 课时文件只在 course 层提供（未编译课程时整个目录不对外）。
  #[cfg(feature = "course")]
  let router = router.nest(
    "/courses",
    Router::new()
      .fallback_service(ServeDir::new(app_core::utils::content_dir("courses")))
      .layer(middleware::from_fn(guard_course_file))
      // 按用户鉴权的内容：共享缓存（CDN / 反代）不得存，浏览器每次复验
      .layer(middleware::map_response(cache_control("private, no-cache"))),
  );
  router
}

const DAY: &str = "public, max-age=86400";

/// 目录服务 + 固定的 `Cache-Control`
fn cached(dir: ServeDir, value: &'static str) -> Router {
  Router::new().fallback_service(dir).layer(middleware::map_response(cache_control(value)))
}

/// 给响应设置固定的 `Cache-Control`
fn cache_control(value: &'static str) -> impl Fn(Response) -> std::future::Ready<Response> + Clone {
  move |mut res: Response| {
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    std::future::ready(res)
  }
}

#[cfg(feature = "course")]
/// SEC-02：课时目录里的正文 / 音视频 / 代码 / 附件与 `get_lesson` 同权限，
/// 无权时按不存在处理，不暴露付费课时的文件名。
async fn guard_course_file(req: Request, next: Next) -> Response {
  let joined: String = req
    .headers()
    .get_all(header::COOKIE)
    .iter()
    .filter_map(|v| v.to_str().ok())
    .collect::<Vec<_>>()
    .join("; ");
  let cookie = (!joined.is_empty()).then_some(joined.as_str());
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

  /// 文章图片等无哈希文件给一天缓存，过期后复验；改为长缓存前要先给文件名加哈希。
  #[tokio::test]
  async fn images_cached_for_a_day() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("images")).unwrap();
    std::fs::write(tmp.path().join("images/a.png"), "1").unwrap();
    let res = get(tmp.path().to_str().unwrap(), "/images/a.png").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(cache_control(&res), Some("public, max-age=86400"));
    assert!(res.headers().contains_key(header::LAST_MODIFIED));
  }

  /// 课时文件按用户鉴权，共享缓存（CDN / 反代）不得存，否则会把付费内容发给未购用户。
  #[cfg(feature = "course")]
  #[tokio::test]
  async fn course_files_are_private() {
    let tmp = tempfile::TempDir::new().unwrap();
    let res = get(tmp.path().to_str().unwrap(), "/courses/nope/01-ch/01-le/index.md").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(cache_control(&res), Some("private, no-cache"));
  }
}
