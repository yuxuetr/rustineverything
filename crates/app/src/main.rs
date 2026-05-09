use dioxus::prelude::*;
use dioxus::router::Router;

mod components;
mod i18n;
mod routes;
mod server;

use crate::i18n::init_i18n;
use crate::routes::Route;
use crate::server::{get_aggregated_theme_css, get_current_user};
use rustineverything_core::session::SessionUser;
use rustineverything_module_search::search::{use_search_open_provider, SearchModal};

/// Static assets used by the application.
// Dioxus 0.7 默认在 crate root 的 assets 目录下寻找
const FAVICON: Asset = asset!("/assets/images/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/css/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const MATH_CSS: Asset = asset!("/assets/css/math.css");
const PRISM_CSS: Asset = asset!("/assets/css/prism-tomorrow.min.css");

fn main() {
  // Phase 2.1 / 2.2: 启动期一次性注册 MDX 嵌入组件。
  // 在 server 和 client 两边都调用，用于 SSR + hydration 双边 registry 一致。
  // 1) widgets 内置 9 个默认组件 (YouTube / Bilibili / 5 色 / Underline / Strikethrough)
  rustineverything_widgets::register_default_components();
  // 2) 各业务模块注册自身提供的组件 (PodcastCard ...)
  rustineverything_module_podcast::register_components();

  // Server: customize the Axum router to serve blog post static assets
  #[cfg(feature = "server")]
  dioxus::serve(|| async move {
      use tower_http::services::ServeDir;
      use axum::routing::get;
      use axum::extract::{Path, Query};
      use axum::response::{IntoResponse, Redirect};

      // 加载 .env 环境变量
      dotenvy::dotenv().ok();

      // 安全门禁：启动时必须提供关键环境变量，避免 fallback 到不安全默认值
      // 1) JWT_SECRET必须配置（panic on missing）
      let _ = rustineverything_core::session::get_jwt_secret();
      // 2) BASE_URL 必须配置为可访问的公网 / 内网地址
      let base_url = std::env::var("BASE_URL")
          .expect("BASE_URL 未配置，请在环境变量或 .env 中设置 BASE_URL");
      let cookie_is_secure = base_url.starts_with("https://");

      // 3) 提前初始化数据库连接池，后续 server fn 都走共享连接。
      //    连接失败仅在日志提示，不阻塞启动，以保证静态页面仍可访问。
      if let Ok(db_url) = std::env::var("DATABASE_URL") {
          if let Err(e) = rustineverything_core::db::init_pool(&db_url).await {
              eprintln!("[Startup] DB pool init failed (服务将在需要时进行连接重试): {}", e);
          } else {
              println!("[Startup] DB pool initialized");
          }
      } else {
          eprintln!("[Startup] DATABASE_URL 未配置，依赖 DB 的功能将在首次调用时出错");
      }

      // 使用 core::utils::get_asset_root 返回的 PathBuf，保证与
      // 其他 server fn 扫描资产的逻辑一致。转换为 String
      // 并 `Box::leak` 为静态生命周期字符串，方便下面 ServeDir
      // format! 调用（启动期仅泄露一次，不会被锁定。）
      let assets_root: &'static str = Box::leak(
          rustineverything_core::utils::get_asset_root()
              .to_string_lossy()
              .into_owned()
              .into_boxed_str()
      );

      let router = dioxus::server::router(App)
          // 1. 处理登录跳转
          .route("/api/auth/login/{provider}", get(|Path(provider): Path<String>| async move {
              if let Ok(url) = crate::server::get_login_url(provider).await {
                  Redirect::temporary(&url).into_response()
              } else {
                  Redirect::temporary("/").into_response()
              }
          }))
          // 2. 处理 OAuth 回调：验证 + 签发 JWT Cookie + 跳转
          .route("/api/auth/callback/{provider}", get(move |Path(provider): Path<String>, Query(params): Query<std::collections::HashMap<String, String>>| async move {
              let code = params.get("code").cloned().unwrap_or_default();
              let state = params.get("state").cloned();
              match crate::server::auth_callback_internal(code, provider, state).await {
                  Ok((_message, jwt_token)) => {
                      // 生产环境 (https) 增加 Secure 标志防止明文传输
                      let secure_flag = if cookie_is_secure { "; Secure" } else { "" };
                      let cookie = format!(
                          "session={}; HttpOnly; Path=/; Max-Age=604800; SameSite=Lax{}",
                          jwt_token,
                          secure_flag
                      );
                      let mut response = Redirect::temporary("/").into_response();
                      if let Ok(cookie_val) = cookie.parse() {
                          response.headers_mut().insert(
                              axum::http::header::SET_COOKIE,
                              cookie_val,
                          );
                      }
                      response
                  }
                  Err(e) => {
                      eprintln!("[Auth Callback] Error: {}", e);
                      Redirect::temporary("/?error=auth_failed").into_response()
                  }
              }
          }))
          // 3. 登出：清除 Cookie
          .route("/api/auth/logout", get(move || async move {
              let secure_flag = if cookie_is_secure { "; Secure" } else { "" };
              let cookie_str = format!(
                  "session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax{}",
                  secure_flag
              );
              let mut response = Redirect::temporary("/").into_response();
              if let Ok(cookie_val) = cookie_str.parse() {
                  response.headers_mut().insert(
                      axum::http::header::SET_COOKIE,
                      cookie_val,
                  );
              }
              response
          }))
          .nest_service("/images", ServeDir::new(format!("{}/images", assets_root)))
          .nest_service("/posts", ServeDir::new(format!("{}/posts", assets_root)))
          .nest_service("/js", ServeDir::new(format!("{}/js", assets_root)))
          .nest_service("/uploads", ServeDir::new(format!("{}/uploads", assets_root)))
          .nest_service("/audio", ServeDir::new(format!("{}/audio", assets_root)))
          .nest_service("/podcasts", ServeDir::new(format!("{}/podcasts", assets_root)))
          .nest_service("/courses", ServeDir::new(format!("{}/courses", assets_root)))
          .nest_service("/cases", ServeDir::new(format!("{}/cases", assets_root)))
          .nest_service("/assets/font", ServeDir::new(format!("{}/font", assets_root)));

      Ok(router)
  });

  // Client: standard launch
  #[cfg(not(feature = "server"))]
  dioxus::launch(App);
}

/// 全局登录模态框状态
pub fn use_auth_modal() -> Signal<bool> {
  use_context::<Signal<bool>>()
}

/// 全局用户会话状态
pub fn use_session_user() -> Signal<Option<SessionUser>> {
  use_context::<Signal<Option<SessionUser>>>()
}

#[component]
fn App() -> Element {
  init_i18n();
  let show_auth = use_signal(|| false);
  use_context_provider(|| show_auth);

  // 搜索 modal 全局状态(Cmd+K 快捷键 + 导航栏按钮共享)
  let _ = use_search_open_provider();

  // 全局用户会话
  let user: Signal<Option<SessionUser>> = use_signal(|| None);
  use_context_provider(|| user);

  // 加载当前用户
  let mut user_signal = user;
  use_effect(move || {
      spawn(async move {
          if let Ok(Some(u)) = get_current_user().await {
              user_signal.set(Some(u));
          }
      });
  });

  // Fetch aggregated theme CSS from WASM plugins
  let theme_css = use_resource(move || async move {
      let result = get_aggregated_theme_css().await;
      match &result {
          Ok(css) => println!("[Frontend] Fetched theme CSS (len: {})", css.len()),
          Err(e) => println!("[Frontend] Failed to fetch theme: {:?}", e),
      }
      result.unwrap_or_default()
  });

  // 原生渲染：读取当前 theme CSS，由下面的 RSX 直接输出为 <style> 节点。
  // 避免 dioxus::document::eval(...) 这种依赖浏览器 DOM API 的街道，
  // 从而保留 desktop / mobile 等跨平台能力。
  let theme_css_value: String = theme_css
      .read()
      .as_ref()
      .cloned()
      .unwrap_or_default();

  rsx! {
      // Head links
      document::Link { rel: "icon", href: FAVICON }
      document::Link { rel: "stylesheet", href: MAIN_CSS }
      document::Link { rel: "stylesheet", href: TAILWIND_CSS }
      
      // Global Fixed Styles (Static)
      document::Style { "
        body {{ 
          background-color: var(--color-bg, white); 
          color: var(--color-text, #0f172a);
          transition: background-color 0.3s ease, color 0.3s ease; 
        }}
        .dark body {{ 
          background-color: var(--color-bg, #020617); 
          color: var(--color-text, #f8fafc);
        }}
        .prose-comment .prose {{ font-size: 0.875rem; }}
        .prose-comment .prose h1 {{ font-size: 1.1em; margin: 0.4em 0; line-height: 1.3; }}
        .prose-comment .prose h2 {{ font-size: 1em; margin: 0.3em 0; line-height: 1.3; }}
        .prose-comment .prose h3 {{ font-size: 0.95em; margin: 0.2em 0; line-height: 1.3; }}
        .prose-comment .prose p {{ margin: 0.3em 0; line-height: 1.5; }}
        .prose-comment .prose img {{ max-height: 200px; border-radius: 0.5rem; margin: 0.5em 0; }}
      " }

      // 从 WASM 插件聊合出的主题 CSS：直接以原生 <style> 标签输出。
      // 使用 document::Style 以保证节点被插入 <head>，不依赖 JS DOM API。
      if !theme_css_value.is_empty() {
          document::Style { id: "wasm-theme-style", "{theme_css_value}" }
      }

      // pulldown-latex math fonts & styles
      document::Link { rel: "stylesheet", href: MATH_CSS }

      // PrismJS for syntax highlighting (core + language packs, served via /js)
      document::Link { rel: "stylesheet", href: PRISM_CSS }
      document::Script { src: "/js/prism.min.js" }
      document::Script { src: "/js/prism-rust.min.js" }
      document::Script { src: "/js/prism-bash.min.js" }
      document::Script { src: "/js/prism-toml.min.js" }
      document::Script { src: "/js/prism-json.min.js" }
      document::Script { src: "/js/prism-yaml.min.js" }
      document::Script { src: "/js/prism-python.min.js" }

      // Mermaid.js for diagram rendering
      document::Script { src: "/js/mermaid.min.js" }
      document::Script { "mermaid.initialize({{ startOnLoad: true, theme: 'default' }});" }

      // 运行时标注运行时（PR-D）
      document::Script { src: "/js/annotations.js" }

      // Main router entry
      Router::<Route> {}

      // Auth modal (rendered at root level to avoid stacking context issues)
      crate::components::auth_modal::AuthModal { show: show_auth }

      // 全局搜索模态框(在根挂一次,任意页面都可 Cmd+K 拉起)
      SearchModal {}
  }
}
