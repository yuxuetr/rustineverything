use dioxus::prelude::*;
use dioxus::router::Router;

mod components;
mod i18n;
mod routes;
mod server;

use crate::components::theme_picker::use_theme_version_provider;
use crate::i18n::init_i18n;
use crate::routes::Route;
use crate::server::{get_aggregated_theme_css, get_current_user};
use app_core::session::SessionUser;
use module_search::search::{use_search_open_provider, SearchModal};

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
  widgets::register_default_components();
  // 2) 各业务模块注册自身提供的组件 (PodcastCard ...)
  module_podcast::register_components();

  // Server: customize the Axum router to serve blog post static assets
  #[cfg(feature = "server")]
  dioxus::serve(|| async move {
    use axum::extract::{Path, Query};
    use axum::http::{header::SET_COOKIE, HeaderMap};
    use axum::response::{IntoResponse, Redirect};
    use axum::routing::get;
    use tower_http::services::ServeDir;

    // 加载 .env 环境变量
    dotenvy::dotenv().ok();

    // Phase 7.5：初始化 tracing 订阅。日志级别由 `RUST_LOG` 控制
    // （默认 `info`），格式为人可读 + 含 target。
    //
    // 用 `try_init`（而非 `init`）：在 `dx serve` 等开发运行时下，
    // dioxus-devtools 可能已安装全局 subscriber，`init` 会 panic
    // （"a global default trace dispatcher has already been set"）。
    // 已存在时静默跳过，生产 `dx bundle` 产物里没有冲突 subscriber，正常生效。
    let _ = tracing_subscriber::fmt()
      .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
          .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
      )
      .with_target(true)
      .try_init();

    // 安全门禁：启动时必须提供关键环境变量，避免 fallback 到不安全默认值
    // 1) JWT_SECRET必须配置（panic on missing）
    let _ = app_core::session::get_jwt_secret();
    // 2) BASE_URL 必须配置为可访问的公网 / 内网地址
    let base_url =
      std::env::var("BASE_URL").expect("BASE_URL 未配置，请在环境变量或 .env 中设置 BASE_URL");
    let cookie_is_secure = base_url.starts_with("https://");

    // 3) 提前初始化数据库连接池，后续 server fn 都走共享连接。
    //    连接失败仅在日志提示，不阻塞启动，以保证静态页面仍可访问。
    //    成功连上数据库后再跑 sea-orm-migration（Phase 7.1）。
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
      match app_core::db::init_pool(&db_url).await {
        Err(e) => {
          tracing::error!(error = %e, "startup: DB pool init failed (will retry on demand)")
        }
        Ok(()) => {
          tracing::info!("startup: DB pool initialized");
          // 自动迁移：用同一连接池跑 sea-orm-migration。
          // 失败仅日志，不退出，避免因为 schema 已存在的细节差异（例如
          // init.sql 已手动落地）阻塞启动；运维通过日志确认即可。
          match app_core::db::get_or_init_pool().await {
            Err(e) => tracing::error!(error = %e, "startup: cannot get pool for migration"),
            Ok(db) => {
              use migration::MigratorTrait;
              match migration::Migrator::up(&db, None).await {
                Ok(()) => tracing::info!("startup: schema migrations applied"),
                Err(e) => tracing::error!(error = %e, "startup: migration failed"),
              }
            }
          }
        }
      }
    } else {
      tracing::warn!("startup: DATABASE_URL not set; DB-backed features will error on first call");
    }

    // 使用 core::utils::get_asset_root 返回的 PathBuf，保证与
    // 其他 server fn 扫描资产的逻辑一致。转换为 String
    // 并 `Box::leak` 为静态生命周期字符串，方便下面 ServeDir
    // format! 调用（启动期仅泄露一次，不会被锁定。）
    let assets_root: &'static str = Box::leak(
      app_core::utils::get_asset_root()
        .to_string_lossy()
        .into_owned()
        .into_boxed_str(),
    );

    let router = dioxus::server::router(App)
      // 1. 处理登录跳转：生成 state + verifier，加密成 oauth_pkce cookie 下发，
      //    然后 302 到 OAuth 授权 URL。state / verifier 自此随浏览器走 → 支持多副本。
      .route(
        "/api/auth/login/{provider}",
        get(move |Path(provider): Path<String>| async move {
          use app_core::auth::build_pkce_set_cookie;
          match crate::server::prepare_login_for_provider(provider).await {
            Ok((url, cookie_value)) => {
              let set_cookie = build_pkce_set_cookie(&cookie_value, cookie_is_secure);
              let mut response = Redirect::temporary(&url).into_response();
              if let Ok(v) = set_cookie.parse() {
                response.headers_mut().insert(SET_COOKIE, v);
              }
              response
            }
            Err(e) => {
              tracing::error!(error = %e, "auth: prepare_login failed");
              Redirect::temporary("/?error=login_failed").into_response()
            }
          }
        }),
      )
      // 2. 处理 OAuth 回调：读 oauth_pkce cookie → 校验 + 签发 JWT Cookie + 清掉 PKCE cookie + 跳转。
      .route(
        "/api/auth/callback/{provider}",
        get(
          move |Path(provider): Path<String>,
                Query(params): Query<std::collections::HashMap<String, String>>,
                headers: HeaderMap| async move {
            use app_core::auth::{
              build_pkce_clear_cookie, extract_pkce_cookie, PkceCookiePayload,
            };
            let code = params.get("code").cloned().unwrap_or_default();
            let received_state = params.get("state").cloned().unwrap_or_default();

            // 从 Cookie 头里抽出 oauth_pkce 并解密
            let cookie_value = headers
              .get_all(axum::http::header::COOKIE)
              .iter()
              .filter_map(|v| v.to_str().ok())
              .find_map(extract_pkce_cookie);

            let clear_pkce_cookie = build_pkce_clear_cookie(cookie_is_secure);
            let attach_clear = |mut resp: axum::response::Response| {
              if let Ok(v) = clear_pkce_cookie.parse() {
                resp.headers_mut().append(SET_COOKIE, v);
              }
              resp
            };

            let pkce = match cookie_value.as_deref().map(PkceCookiePayload::decode) {
              Some(Ok(p)) => p,
              Some(Err(e)) => {
                tracing::warn!(error = %e, "auth: oauth_pkce cookie decode failed");
                return attach_clear(
                  Redirect::temporary("/?error=auth_session_invalid").into_response(),
                );
              }
              None => {
                tracing::warn!("auth: oauth_pkce cookie missing on callback");
                return attach_clear(
                  Redirect::temporary("/?error=auth_session_missing").into_response(),
                );
              }
            };

            match crate::server::auth_callback_internal(code, provider, received_state, pkce)
              .await
            {
              Ok((_message, jwt_token)) => {
                let secure_flag = if cookie_is_secure { "; Secure" } else { "" };
                let session_cookie = format!(
                  "session={}; HttpOnly; Path=/; Max-Age=604800; SameSite=Lax{}",
                  jwt_token, secure_flag
                );
                let mut response = Redirect::temporary("/").into_response();
                if let Ok(v) = session_cookie.parse() {
                  response.headers_mut().append(SET_COOKIE, v);
                }
                attach_clear(response)
              }
              Err(e) => {
                tracing::error!(error = %e, "auth callback failed");
                attach_clear(Redirect::temporary("/?error=auth_failed").into_response())
              }
            }
          },
        ),
      )
      // 3. 登出：清除 Cookie
      .route(
        "/api/auth/logout",
        get(move || async move {
          let secure_flag = if cookie_is_secure { "; Secure" } else { "" };
          let cookie_str =
            format!("session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax{}", secure_flag);
          let mut response = Redirect::temporary("/").into_response();
          if let Ok(cookie_val) = cookie_str.parse() {
            response.headers_mut().insert(axum::http::header::SET_COOKIE, cookie_val);
          }
          response
        }),
      )
      .nest_service("/images", ServeDir::new(format!("{}/images", assets_root)))
      .nest_service("/posts", ServeDir::new(format!("{}/posts", assets_root)))
      .nest_service("/js", ServeDir::new(format!("{}/js", assets_root)))
      .nest_service("/uploads", ServeDir::new(format!("{}/uploads", assets_root)))
      .nest_service("/audio", ServeDir::new(format!("{}/audio", assets_root)))
      .nest_service("/podcasts", ServeDir::new(format!("{}/podcasts", assets_root)))
      .nest_service("/courses", ServeDir::new(format!("{}/courses", assets_root)))
      .nest_service("/cases", ServeDir::new(format!("{}/cases", assets_root)))
      .nest_service("/assets/font", ServeDir::new(format!("{}/font", assets_root)));

    // Phase 2.4: 公开 SEO 路由
    // 为 sitemap / feed 构造 base_url。复用上面已读取的 base_url 变量，
    // 避免重复读 env。router 各闭包需要拥有该字符串，这里 clone
    // 三份分别交给 sitemap / feed / robots。
    let base_url_for_routes = base_url.clone();
    let sitemap_base = base_url.clone();
    let feed_base = base_url.clone();
    let robots_base = base_url_for_routes.clone();
    let _ = base_url_for_routes; // 仅为下面闭包提供变量名锁定

    let router = router
      .route(
        "/sitemap.xml",
        get(move || {
          let base = sitemap_base.clone();
          async move {
            use widgets::{build_sitemap_xml, ContentEntry};
            // Phase 3.4：按模块开关过滤静态路径与博客条目。
            let module_engine = app_core::engines::module::default_module_engine();
            let enabled = module_engine.enabled_ids();
            let is_on = |id: &str| enabled.iter().any(|s| s == id);

            // 仅在 blog 启用时枚举博客条目（避免无谓 IO）
            let mut entries: Vec<ContentEntry> = if is_on("blog") {
              let posts =
                module_blog::server::list_blog_posts().await.unwrap_or_default();
              posts
                .into_iter()
                .map(|p| ContentEntry {
                  url_path: format!("/blog/{}", p.slug),
                  title: p.title,
                  description: p.description,
                  date: p.date,
                  tags: p.tags,
                })
                .collect()
            } else {
              Vec::new()
            };

            // Phase 6：内容板块文章条目，按开关收录。
            macro_rules! collect_board {
              ($id:literal, $list:path, $route:literal) => {
                if is_on($id) {
                  for a in $list().await.unwrap_or_default() {
                    entries.push(ContentEntry {
                      url_path: format!(concat!($route, "/{}"), a.slug),
                      title: a.title,
                      description: a.description,
                      date: a.date,
                      tags: a.tags,
                    });
                  }
                }
              };
            }
            collect_board!(
              "embedded",
              module_embedded::server::list_embedded_articles,
              "/embedded"
            );
            collect_board!("ai", module_ai::server::list_ai_articles, "/ai");
            collect_board!(
              "web3",
              module_web3::server::list_web3_articles,
              "/web3"
            );
            collect_board!(
              "wasm",
              module_wasm::server::list_wasm_articles,
              "/wasm"
            );
            collect_board!("cli", module_cli::server::list_cli_articles, "/cli");

            // 静态路径：首页恒收录；其它模块按开关动态拼接。
            let mut static_paths: Vec<&'static str> = vec!["/"];
            if is_on("blog") {
              static_paths.push("/blog");
            }
            if is_on("podcast") {
              static_paths.push("/podcast");
            }
            if is_on("course") {
              static_paths.push("/course");
            }
            if is_on("cases") {
              static_paths.push("/case");
            }
            if is_on("docs") {
              static_paths.push("/docs");
            }
            if is_on("forum") {
              static_paths.push("/topics");
            }
            if is_on("embedded") {
              static_paths.push("/embedded");
            }
            if is_on("ai") {
              static_paths.push("/ai");
            }
            if is_on("web3") {
              static_paths.push("/web3");
            }
            if is_on("wasm") {
              static_paths.push("/wasm");
            }
            if is_on("cli") {
              static_paths.push("/cli");
            }

            let xml = build_sitemap_xml(&entries, &static_paths, &base);
            axum::response::Response::builder()
              .header("content-type", "application/xml; charset=utf-8")
              .body(axum::body::Body::from(xml))
              .unwrap_or_else(|_| axum::response::Response::new(axum::body::Body::empty()))
          }
        }),
      )
      .route(
        "/feed.xml",
        get(move || {
          let base = feed_base.clone();
          async move {
            use widgets::{build_atom_feed, ContentEntry};
            // Phase 3.4：blog 关闭时输出空 feed，但保留站点元信息。
            let module_engine = app_core::engines::module::default_module_engine();
            let blog_on = module_engine.is_enabled("blog");

            let is_on = |id: &str| module_engine.enabled_ids().iter().any(|s| s == id);
            let mut entries: Vec<ContentEntry> = if blog_on {
              module_blog::server::list_blog_posts()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|p| ContentEntry {
                  url_path: format!("/blog/{}", p.slug),
                  title: p.title,
                  description: p.description,
                  date: p.date,
                  tags: p.tags,
                })
                .collect()
            } else {
              Vec::new()
            };

            // Phase 6：内容板块文章也进 feed。
            macro_rules! collect_board {
              ($id:literal, $list:path, $route:literal) => {
                if is_on($id) {
                  for a in $list().await.unwrap_or_default() {
                    entries.push(ContentEntry {
                      url_path: format!(concat!($route, "/{}"), a.slug),
                      title: a.title,
                      description: a.description,
                      date: a.date,
                      tags: a.tags,
                    });
                  }
                }
              };
            }
            collect_board!(
              "embedded",
              module_embedded::server::list_embedded_articles,
              "/embedded"
            );
            collect_board!("ai", module_ai::server::list_ai_articles, "/ai");
            collect_board!(
              "web3",
              module_web3::server::list_web3_articles,
              "/web3"
            );
            collect_board!(
              "wasm",
              module_wasm::server::list_wasm_articles,
              "/wasm"
            );
            collect_board!("cli", module_cli::server::list_cli_articles, "/cli");

            // 全站按日期降序，取最近 50 篇。
            entries.sort_by(|a, b| b.date.cmp(&a.date));
            entries.truncate(50);
            // 取站点元信息：如取不到 site.json 则走默认。
            let cfg = app_core::settings::SiteConfig::from_file(
              app_core::utils::get_asset_root()
                .join("site.json")
                .to_str()
                .unwrap_or_default(),
            )
            .unwrap_or_default();
            let xml = build_atom_feed(&entries, &cfg.site_name, &cfg.site_description, &base);
            axum::response::Response::builder()
              .header("content-type", "application/atom+xml; charset=utf-8")
              .body(axum::body::Body::from(xml))
              .unwrap_or_else(|_| axum::response::Response::new(axum::body::Body::empty()))
          }
        }),
      )
      .route(
        "/robots.txt",
        get(move || {
          let base = robots_base.clone();
          async move {
            let body = widgets::build_robots_txt(&base);
            axum::response::Response::builder()
              .header("content-type", "text/plain; charset=utf-8")
              .body(axum::body::Body::from(body))
              .unwrap_or_else(|_| axum::response::Response::new(axum::body::Body::empty()))
          }
        }),
      );

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

  // Phase 3.1：ThemeVersion context。Navbar 的 ThemePicker 切换后 bump 该 Signal，
  // 下面的 theme_css use_resource 将重新拉取聚合 CSS。
  let theme_version = use_theme_version_provider();

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

  // Fetch aggregated theme CSS from WASM plugins。订阅 theme_version 以便切换重拉。
  let theme_css = use_resource(move || {
    let _ = theme_version();
    async move {
      let result = get_aggregated_theme_css().await;
      match &result {
        Ok(css) => tracing::debug!(len = css.len(), "frontend: fetched theme CSS"),
        Err(e) => tracing::warn!(error = ?e, "frontend: failed to fetch theme"),
      }
      result.unwrap_or_default()
    }
  });

  // 原生渲染：读取当前 theme CSS，由下面的 RSX 直接输出为 <style> 节点。
  // 避免 dioxus::document::eval(...) 这种依赖浏览器 DOM API 的街道，
  // 从而保留 desktop / mobile 等跨平台能力。
  let theme_css_value: String = theme_css.read().as_ref().cloned().unwrap_or_default();

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
