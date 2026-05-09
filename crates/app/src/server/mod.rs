use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use rustineverything_core::settings::SiteConfig;
use rustineverything_core::session::SessionUser;
use rustineverything_core::utils::get_asset_root;

// ========== 辅助：从 FullstackContext 读取 Cookie 中的用户 ==========

/// server-only: 从当前请求上下文的 Cookie 中解析 SessionUser
#[cfg(feature = "server")]
fn current_session_user() -> Option<SessionUser> {
    use dioxus::fullstack::FullstackContext;
    use rustineverything_core::session::parse_session_from_cookie_header;

    let ctx = FullstackContext::current()?;
    let parts = ctx.parts_mut();
    let cookie_str = parts
        .headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    drop(parts);
    parse_session_from_cookie_header(cookie_str.as_deref())
}

// ========== 站点配置 ==========

#[post("/api/site/config")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
    let config_path = get_asset_root().join("site.json");
    SiteConfig::from_file(config_path.to_str().unwrap())
        .map_err(|e| ServerFnError::new(format!("配置文件加载失败: {}", e)))
}

// ========== i18n ==========

#[post("/api/i18n/translate")]
pub async fn translate_server(key: String, lang: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join("i18n_fluent_plugin.wasm");

        if !wasm_path.exists() { return Ok(key); }
        let manager = rustineverything_core::shared_plugin_manager();
        let input = serde_json::json!({ "key": key, "lang": lang }).to_string();
        manager.call_path_with_string(&wasm_path, "translate", &input)
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { let _ = (key, lang); Ok(String::new()) }
}

// ========== 主题 ==========

#[post("/api/theme/aggregated-css")]
pub async fn get_aggregated_theme_css() -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join(&config.active_theme);

        if !wasm_path.exists() { return Ok("".to_string()); }
        let manager = rustineverything_core::shared_plugin_manager();
        Ok(manager.aggregate_theme_css_paths(&[wasm_path]))
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

// ========== Auth 辅助 (server-only) ==========

#[cfg(feature = "server")]
fn build_auth_service() -> (rustineverything_core::auth::AuthService, SiteConfig) {
    use rustineverything_core::auth::{AuthService, AuthConfig};

    // BASE_URL 未配置时 panic，避免生产环境误用 localhost
    let base_url = std::env::var("BASE_URL")
        .expect("BASE_URL 未配置，请在环境变量或 .env 中设置 BASE_URL");
    let config = AuthConfig { base_url };
    let site_config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
    let auth_service = AuthService::new(config, get_asset_root().join("plugins"));
    (auth_service, site_config)
}

#[cfg(feature = "server")]
fn find_plugin_filename(site_config: &SiteConfig, provider: &str) -> Option<String> {
    site_config.auth.providers.iter()
        .find(|p| p.id == provider)
        .map(|p| p.plugin.clone())
}

// ========== Auth 端点 ==========

#[post("/api/auth/providers")]
pub async fn get_auth_providers() -> Result<Vec<rustineverything_core::AuthProviderDisplay>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let (auth_service, site_config) = build_auth_service();
        Ok(auth_service.list_available_providers(&site_config))
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

#[post("/api/auth/login-url")]
pub async fn get_login_url(provider: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let (auth_service, site_config) = build_auth_service();
        let plugin_filename = find_plugin_filename(&site_config, &provider)
            .ok_or_else(|| ServerFnError::new(format!("未在 site.json 中配置 provider: {}", provider)))?;
        auth_service.get_auth_url(&provider, &plugin_filename)
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

/// 内部 auth callback — 仅 server 端调用，返回 (welcome_message, jwt_token)
#[cfg(feature = "server")]
pub async fn auth_callback_internal(
    code: String,
    provider: String,
    state: Option<String>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    use rustineverything_core::db::get_or_init_pool;
    use rustineverything_core::session::create_jwt;

    let (auth_service, site_config) = build_auth_service();
    let plugin_filename = find_plugin_filename(&site_config, &provider)
        .ok_or_else(|| format!("未在 site.json 中配置 provider: {}", provider))?;

    println!("[Auth Callback] provider={}, code_len={}, state={:?}", provider, code.len(), state);

    let db = get_or_init_pool().await?;

    let user = auth_service
        .handle_callback(&db, &provider, &plugin_filename, code, state)
        .await?;

    let jwt_token = create_jwt(&user)?;
    println!("[Auth Callback] Login success: user={}", user.nickname);
    Ok((format!("欢迎回来, {}!", user.nickname), jwt_token))
}

/// 获取当前登录用户 — 前端调用
#[post("/api/auth/me")]
pub async fn get_current_user() -> Result<Option<SessionUser>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        Ok(current_session_user())
    }
    #[cfg(not(feature = "server"))]
    { Ok(None) }
}

// ========== Echo ==========

#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> { Ok(input) }
