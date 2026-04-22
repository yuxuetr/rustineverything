use dioxus::prelude::*;
use dioxus::router::Router;

mod components;
mod i18n;
mod routes;
mod server;

use crate::i18n::init_i18n;
use crate::routes::Route;
use crate::server::get_aggregated_theme_css;

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
      use axum::response::Redirect;

      // 加载 .env 环境变量
      dotenvy::dotenv().ok();

      // Detect the assets root (same logic as server/mod.rs)
      let assets_root = if std::path::Path::new("assets").exists() {
          "assets"
      } else {
          "../../assets"
      };

      let router = dioxus::server::router(App)
          // 1. 处理登录跳转：GET /api/auth/login/github -> Redirect to GitHub
          .route("/api/auth/login/{provider}", get(|Path(provider): Path<String>| async move {
              if let Ok(url) = crate::server::get_login_url(provider).await {
                  Redirect::temporary(&url)
              } else {
                  Redirect::temporary("/")
              }
          }))
          // 2. 处理回调：GET /api/auth/callback/github?code=...
          .route("/api/auth/callback/{provider}", get(|Path(provider): Path<String>, Query(params): Query<std::collections::HashMap<String, String>>| async move {
              let code = params.get("code").cloned().unwrap_or_default();
              if let Ok(msg) = crate::server::auth_callback(code, provider).await {
                  // 登录成功后跳回首页或显示成功信息
                  Redirect::temporary(&format!("/?message={}", msg))
              } else {
                  Redirect::temporary("/?error=auth_failed")
              }
          }))
          .nest_service("/images", ServeDir::new(format!("{}/images", assets_root)))
          .nest_service("/posts", ServeDir::new(format!("{}/posts", assets_root)))
          .nest_service("/js", ServeDir::new(format!("{}/js", assets_root)))
          .nest_service("/uploads", ServeDir::new(format!("{}/uploads", assets_root)))
          .nest_service("/audio", ServeDir::new(format!("{}/audio", assets_root)));

      Ok(router)
  });

  // Client: standard launch
  #[cfg(not(feature = "server"))]
  dioxus::launch(App);
}

#[component]
fn App() -> Element {
  init_i18n();

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

      // Main router entry
      Router::<Route> {}
  }
}
