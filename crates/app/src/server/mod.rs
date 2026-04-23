use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use rustineverything_core::settings::SiteConfig;
use rustineverything_core::session::SessionUser;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use std::fs;

/// 自动探测资产根目录
#[allow(dead_code)]
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        path = PathBuf::from("../../assets");
    }
    path
}

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
        use rustineverything_core::PluginManager;
        let _config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join("i18n_fluent_plugin.wasm");

        if !wasm_path.exists() { return Ok(key); }
        let wasm_bytes = fs::read(wasm_path).map_err(|e| ServerFnError::new(e.to_string()))?;
        let manager = PluginManager::new();
        let input = serde_json::json!({ "key": key, "lang": lang }).to_string();
        manager.call_with_string(&wasm_bytes, "translate", &input).map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { Ok(key) }
}

// ========== 主题 ==========

#[post("/api/theme/aggregated-css")]
pub async fn get_aggregated_theme_css() -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::PluginManager;
        let config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join(&config.active_theme);

        if !wasm_path.exists() { return Ok("".to_string()); }
        let wasm_bytes = fs::read(wasm_path).map_err(|e| ServerFnError::new(e.to_string()))?;
        let manager = PluginManager::new();
        Ok(manager.aggregate_theme_css(&[wasm_bytes]))
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

// ========== Auth 辅助 (server-only) ==========

#[cfg(feature = "server")]
fn build_auth_service() -> (rustineverything_core::auth::AuthService, SiteConfig) {
    use rustineverything_core::auth::{AuthService, AuthConfig};

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
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
    use rustineverything_core::db::init_db;
    use rustineverything_core::session::create_jwt;

    let (auth_service, site_config) = build_auth_service();
    let plugin_filename = find_plugin_filename(&site_config, &provider)
        .ok_or_else(|| format!("未在 site.json 中配置 provider: {}", provider))?;

    println!("[Auth Callback] provider={}, code_len={}, state={:?}", provider, code.len(), state);

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/rustineverything".to_string());
    let db = init_db(&db_url).await?;

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

// ========== 评论系统 (数据库) ==========

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub id: String,
    pub blog_id: String,
    pub content: String,
    pub author: String,
    pub author_avatar: Option<String>,
    pub user_id: Option<i32>,
    pub date: String,
}

#[post("/api/comments/list")]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::db::init_db;
        use rustineverything_core::entities::{comment, user};
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, QueryOrder};

        let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
        let db = init_db(&db_url).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let results = comment::Entity::find()
            .filter(comment::Column::BlogId.eq(&blog_id))
            .find_also_related(user::Entity)
            .order_by_desc(comment::Column::CreatedAt)
            .all(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let comments = results
            .into_iter()
            .map(|(c, u)| Comment {
                id: c.id.to_string(),
                blog_id: c.blog_id,
                content: c.content,
                author: u.as_ref().map(|u| u.nickname.clone()).unwrap_or_else(|| "已注销".to_string()),
                author_avatar: u.as_ref().and_then(|u| u.avatar_url.clone()),
                user_id: Some(c.user_id),
                date: c.created_at.format("%Y-%m-%d %H:%M").to_string(),
            })
            .collect();

        Ok(comments)
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

#[post("/api/comments/post")]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::db::init_db;
        use rustineverything_core::entities::comment;
        use sea_orm::{EntityTrait, Set};
        use chrono::Utc;

        let session_user = current_session_user()
            .ok_or_else(|| ServerFnError::new("请先登录后再发表评论"))?;

        let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
        let db = init_db(&db_url).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let new_comment = comment::ActiveModel {
            blog_id: Set(blog_id.clone()),
            user_id: Set(session_user.id),
            content: Set(content),
            created_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        };
        comment::Entity::insert(new_comment)
            .exec(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        get_comments(blog_id).await
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

// ========== 上传 / Echo ==========

#[post("/api/upload")]
pub async fn upload_image(name: String, data_base64: String) -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
      use base64::Engine as _;
      let base64_str = data_base64.split(',').last().unwrap_or(&data_base64);
      let data = base64::engine::general_purpose::STANDARD.decode(base64_str).map_err(|e| ServerFnError::new(format!("解码失败: {}", e)))?;

      let filename = format!("{}_{}", chrono::Utc::now().timestamp(), name);
      let dir_path = get_asset_root().join("uploads");
      if !dir_path.exists() { fs::create_dir_all(&dir_path).map_err(|e| ServerFnError::new(e.to_string()))?; }

      let path = dir_path.join(&filename);
      fs::write(&path, &data).map_err(|e| ServerFnError::new(format!("保存失败: {}", e)))?;

      Ok(format!("/uploads/{}", filename))
  }
  #[cfg(not(feature = "server"))]
  { Ok("".to_string()) }
}

#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> { Ok(input) }
