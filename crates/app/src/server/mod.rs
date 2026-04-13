use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use rustineverything_core::settings::SiteConfig;
use std::path::PathBuf;
use std::fs;

/// 自动探测资产根目录
/// 我们规定：代码在 crates/app，内容在 根目录/assets
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        // 兼容本地运行和工作区运行
        path = PathBuf::from("../../assets");
    }
    path
}

#[post("/api/site/config")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
    let config_path = get_asset_root().join("site.json");
    SiteConfig::from_file(config_path.to_str().unwrap())
        .map_err(|e| ServerFnError::new(format!("配置文件加载失败: {}", e)))
}

#[post("/api/i18n/translate")]
pub async fn translate_server(key: String, lang: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::PluginManager;
        let config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
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

#[post("/api/auth/login-url")]
pub async fn get_login_url(provider: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, CsrfToken};
        use rustineverything_core::auth::AuthConfig;
        
        let config = AuthConfig {
            github_client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            redirect_url: "http://localhost:8080/api/auth/callback".to_string(),
        };

        let client_id = config.github_client_id;
        let client_secret = config.github_client_secret;
        let redirect_uri = config.redirect_url;

        let (url, _) = match provider.as_str() {
            "github" => {
                let client = BasicClient::new(ClientId::new(client_id))
                    .set_client_secret(ClientSecret::new(client_secret))
                    .set_auth_uri(AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap())
                    .set_token_uri(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap())
                    .set_redirect_uri(RedirectUrl::new(redirect_uri).unwrap());
                client.authorize_url(CsrfToken::new_random).url()
            },
            _ => return Err(ServerFnError::new("不支持的登录平台")),
        };
        Ok(url.to_string())
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

#[post("/api/auth/callback")]
pub async fn auth_callback(code: String, provider: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::auth::{AuthService, AuthConfig};
        use rustineverything_core::db::init_db;
        let config = AuthConfig {
            github_client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            redirect_url: "http://localhost:8080/api/auth/callback".to_string(),
        };
        let db_url = std::env::var("DATABASE_URL").unwrap_or("postgres://postgres:password@localhost/rustineverything".to_string());
        let db = init_db(&db_url).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let auth_service = AuthService::new(config);
        let user = match provider.as_str() {
            "github" => auth_service.sync_github_user(&db, code).await.map_err(|e| ServerFnError::new(e.to_string()))?,
            _ => return Err(ServerFnError::new("同步失败")),
        };
        Ok(format!("欢迎回来, {}!", user.nickname))
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

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
