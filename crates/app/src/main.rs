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

/// Static assets used by the application.
// Dioxus 0.7 默认在 crate root 的 assets 目录下寻找
const FAVICON: Asset = asset!("/assets/images/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/css/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const MATH_CSS: Asset = asset!("/assets/css/math.css");
const PRISM_CSS: Asset = asset!("/assets/css/prism-tomorrow.min.css");

fn main() {
  // Server: customize the Axum router to serve blog post static assets
  #[cfg(feature = "server")]
  dioxus::serve(|| async move {
      use tower_http::services::ServeDir;
      use axum::routing::get;
      use axum::extract::{Path, Query};
      use axum::response::{IntoResponse, Redirect};

      // 加载 .env 环境变量
      dotenvy::dotenv().ok();

      // Detect the assets root (same logic as server/mod.rs)
      let assets_root = if std::path::Path::new("assets").exists() {
          "assets"
      } else {
          "../../assets"
      };

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
          .route("/api/auth/callback/{provider}", get(|Path(provider): Path<String>, Query(params): Query<std::collections::HashMap<String, String>>| async move {
              let code = params.get("code").cloned().unwrap_or_default();
              let state = params.get("state").cloned();
              match crate::server::auth_callback_internal(code, provider, state).await {
                  Ok((_message, jwt_token)) => {
                      let cookie = format!(
                          "session={}; HttpOnly; Path=/; Max-Age=604800; SameSite=Lax",
                          jwt_token
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
          .route("/api/auth/logout", get(|| async {
              let mut response = Redirect::temporary("/").into_response();
              if let Ok(cookie_val) = "session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax".parse() {
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

  // 使用 eval 动态注入和更新样式
  use_effect(move || {
      if let Some(css) = theme_css.read().as_ref() {
          let js = format!(
              r#"
              console.log("[Frontend] Injecting CSS into #wasm-theme-style");
              let styleTag = document.getElementById('wasm-theme-style');
              if (!styleTag) {{
                  styleTag = document.createElement('style');
                  styleTag.id = 'wasm-theme-style';
                  document.head.appendChild(styleTag);
              }}
              styleTag.innerHTML = `{}`;
              "#,
              css
          );
          dioxus::document::eval(&js);
      }
  });

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

      // Main router entry
      Router::<Route> {}

      // Auth modal (rendered at root level to avoid stacking context issues)
      crate::components::auth_modal::AuthModal { show: show_auth }
  }
}
