//! S7（风险 R8）：静态资源目录服务。从 `main.rs` 拆出，行为不变。

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
    .nest_service("/courses", ServeDir::new(format!("{}/courses", assets_root)))
    .nest_service("/cases", ServeDir::new(format!("{}/cases", assets_root)))
    .nest_service("/assets/font", ServeDir::new(format!("{}/font", assets_root)))
}
