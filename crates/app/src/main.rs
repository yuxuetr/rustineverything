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

/// 全局一次性引导脚本：初始化 mermaid，对**整个文档**开 MutationObserver，
/// 一旦 DOM 中出现未处理的 `<code class="language-*">` 或 `.mermaid` 块就
/// 自动跑 Prism / mermaid。SPA 导航换入新 Markdown 内容时同样能触发。
///
/// 设计要点：
/// - **只挂载一次**（在 App 根的 document::Script 里），避免 mdx 组件级别
///   的 Script 引起 dioxus-document props-change 警告且无法重跑。
/// - 等 `window.Prism` / `window.mermaid` 就绪后再启动 observer（脚本加载顺序
///   不可控，prism / mermaid 的 `<script src>` 可能晚于本脚本到达）。
/// - 用 `Prism.highlightAllUnder` + 自加 `data-prism-done` 标记，防止重复处理。
/// - mermaid 用其自身的 `data-processed` 已有标记（mermaid 内部维护）。
const GLOBAL_REHIGHLIGHT_BOOT_SCRIPT: &str = r#"
(function(){
  function tryHighlight(root){
    if(!window.Prism) return;
    var nodes = (root || document).querySelectorAll('code[class*="language-"]:not([data-prism-done])');
    for(var i = 0; i < nodes.length; i++){
      try { window.Prism.highlightElement(nodes[i]); } catch(e) {}
      nodes[i].setAttribute('data-prism-done', '1');
    }
  }
  function tryMermaid(root){
    if(!window.mermaid || typeof window.mermaid.run !== 'function') return;
    var hits = (root || document).querySelectorAll('.mermaid:not([data-processed])');
    if(hits.length === 0) return;
    try { window.mermaid.run({nodes: hits}); } catch(e) {}
  }
  function bootMermaid(){
    if(window.mermaid && typeof window.mermaid.initialize === 'function' && !window.__rie_mermaid_init){
      window.__rie_mermaid_init = true;
      try { window.mermaid.initialize({startOnLoad: false, theme: 'default'}); } catch(e) {}
    }
  }
  function sweep(){
    bootMermaid();
    tryHighlight(document);
    tryMermaid(document);
  }
  // 初始扫一遍（处理首屏 / hydration 后已存在的内容）
  if(document.readyState === 'loading'){
    document.addEventListener('DOMContentLoaded', sweep);
  } else {
    sweep();
  }
  // 观察后续 DOM 插入（SPA 路由切换换入新 markdown 时触发）
  if(typeof MutationObserver === 'function' && !window.__rie_obs){
    window.__rie_obs = new MutationObserver(function(){
      // 节流：合并同一 tick 的多次插入
      if(window.__rie_obs_q) return;
      window.__rie_obs_q = true;
      setTimeout(function(){
        window.__rie_obs_q = false;
        sweep();
      }, 30);
    });
    window.__rie_obs.observe(document.body || document.documentElement, {childList: true, subtree: true});
  }
})();
"#;

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
    // 1) JWT_SECRET 必须配置且非 placeholder（panic on missing / 命中模板片段）
    let _ = app_core::session::get_jwt_secret();
    // 2) BASE_URL 必须配置为可访问的公网 / 内网地址，且不能仍是 .env.example 占位
    let base_url =
      std::env::var("BASE_URL").expect("BASE_URL 未配置，请在环境变量或 .env 中设置 BASE_URL");
    app_core::session::assert_not_placeholder("BASE_URL", &base_url);
    let cookie_is_secure = base_url.starts_with("https://");

    // 3) 提前初始化数据库连接池，后续 server fn 都走共享连接。
    //    DATABASE_URL 也按 placeholder 校验，避免「忘了改 docker-compose 密码」
    //    导致 sqlx 拒绝认证的难以排查的失败。
    //    连接失败仅在日志提示，不阻塞启动，以保证静态页面仍可访问。
    //    成功连上数据库后再跑 sea-orm-migration（Phase 7.1）。
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
      app_core::session::assert_not_placeholder("DATABASE_URL", &db_url);
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

    // Phase 8.8：用 OnceLock 取代 Box::leak，避免 `dx serve` 反复重启的开发态泄漏
    // 累积。`OnceLock<&'static str>` 在首次写入时把 String leak 成 &'static，
    // 后续运行直接读 cell（无重复 leak、无可观察的语义差异）。
    static ASSETS_ROOT: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    let assets_root: &'static str = ASSETS_ROOT.get_or_init(|| {
      Box::leak(
        app_core::utils::get_asset_root()
          .to_string_lossy()
          .into_owned()
          .into_boxed_str(),
      )
    });

    // Phase 8.8：插件 hot reload 会留下 `assets/plugins/<name>.wasm.bak`
    // 备份。超过 7 天的旧备份在启动时清理，避免长跑站点磁盘占用持续累加。
    let plugin_dir = app_core::utils::get_asset_root().join("plugins");
    if let Ok(entries) = std::fs::read_dir(&plugin_dir) {
      let week = std::time::Duration::from_secs(7 * 24 * 3600);
      let now = std::time::SystemTime::now();
      for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("bak") {
          continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        if now.duration_since(modified).map(|age| age > week).unwrap_or(false) {
          match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(path = %path.display(), "startup: pruned stale .bak (>7d)"),
            Err(e) => tracing::warn!(error = %e, path = %path.display(), "startup: failed to prune .bak"),
          }
        }
      }
    }

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
            // Phase 8.5：sitemap 1 小时 Cache-Control，让爬虫与 CDN 不至于
            // 每次请求都打满 list_* hot path。`public` 表示中间代理也能缓存。
            axum::response::Response::builder()
              .header("content-type", "application/xml; charset=utf-8")
              .header("cache-control", "public, max-age=3600")
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
            // Phase 8.5：同 sitemap，feed 1 小时 Cache-Control 缓解 RSS reader
            // 高频轮询带来的 server fn 调用压力。
            axum::response::Response::builder()
              .header("content-type", "application/atom+xml; charset=utf-8")
              .header("cache-control", "public, max-age=3600")
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
            // robots.txt 几乎不变，6h 缓存。
            axum::response::Response::builder()
              .header("content-type", "text/plain; charset=utf-8")
              .header("cache-control", "public, max-age=21600")
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
  // Phase 8.8：项目当前是 web-first via dioxus_fullstack（SSR + WASM hydration）。
  // 原措辞「保留 desktop / mobile 等跨平台能力」与现状不符；其他 16 处
  // `dioxus::document::eval` 在 widgets crate 中保留是为了避免大规模重写
  // （见 `docs/PLUGIN_DEV.md`）。
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

      // 从 WASM 插件聚合出的主题 CSS。
      // 注意：**不能**用 `document::Style`：theme_css_value 会随用户切换
      // 主题变化，而 `document::Style/Script` 只支持一次性挂载（变更 props
      // 会触发 dioxus-document "Changing the props … is not supported" 警告
      // 且不更新 DOM）。改用普通 `style` 元素 + `dangerous_inner_html`：
      // CSS 落在 <body> 仍全局生效，且 vdom diff 正常更新内容。
      if !theme_css_value.is_empty() {
          style {
              id: "wasm-theme-style",
              dangerous_inner_html: "{theme_css_value}",
          }
      }

      // mdx 内容的全局静态样式（math / 评论 prose 缩排）。原本住在
      // widgets/src/mdx.rs 的 document::Style，但 mdx 组件每次路由切换都
      // 重渲，会触发 props-change 警告 → 上提到 App 根，挂载一次。
      document::Style { "
        math {{ font-size: 1.1em; }}
        .math-display math {{ font-size: 1.4em; }}
        .prose code::before, .prose code::after {{ content: none !important; }}
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

      // 全局 rehighlight / mermaid 触发器。**只在 App 根挂载一次**，靠
      // MutationObserver 自动捕获后续插入到 DOM 的代码块 / mermaid 块（包括
      // SPA 路由切换换入的新 Markdown 内容）。
      //
      // 为什么不放在 widgets/mdx.rs 里：那个组件每次路由切换都重渲，会触发
      // dioxus-document "Changing the props of Script is not supported" 警告，
      // 且更换后的脚本不会再次执行 → SPA 导航后 Prism / Mermaid 不重跑。
      // MutationObserver 一次挂载 → 长期生效，幂等且零警告。
      document::Script { {GLOBAL_REHIGHLIGHT_BOOT_SCRIPT} }

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
