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

/// 加载 site.json 配置并构建 AuthService 的辅助函数 (server-only)
#[cfg(feature = "server")]
fn build_auth_service() -> (rustineverything_core::auth::AuthService, SiteConfig) {
    use rustineverything_core::auth::{AuthService, AuthConfig};

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let config = AuthConfig { base_url };
    let site_config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
    let auth_service = AuthService::new(config, get_asset_root().join("plugins"));
    (auth_service, site_config)
}

/// 根据 provider id 从 site.json 查找对应的插件文件名
#[cfg(feature = "server")]
fn find_plugin_filename(site_config: &SiteConfig, provider: &str) -> Option<String> {
    site_config.auth.providers.iter()
        .find(|p| p.id == provider)
        .map(|p| p.plugin.clone())
}

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

#[post("/api/auth/callback")]
pub async fn auth_callback(code: String, provider: String, state: Option<String>) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::db::init_db;

        let (auth_service, site_config) = build_auth_service();
        let plugin_filename = find_plugin_filename(&site_config, &provider)
            .ok_or_else(|| ServerFnError::new(format!("未在 site.json 中配置 provider: {}", provider)))?;

        println!("[Auth Callback] provider={}, code_len={}, state={:?}", provider, code.len(), state);

        let db_url = std::env::var("DATABASE_URL").unwrap_or("postgres://postgres:password@localhost/rustineverything".to_string());
        let db = init_db(&db_url).await.map_err(|e| {
            eprintln!("[Auth Callback] DB connection failed: {}", e);
            ServerFnError::new(e.to_string())
        })?;

        let user = auth_service.handle_callback(&db, &provider, &plugin_filename, code, state).await.map_err(|e| {
            eprintln!("[Auth Callback] handle_callback failed: {}", e);
            ServerFnError::new(e.to_string())
        })?;
        println!("[Auth Callback] Login success: user={}", user.nickname);
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
