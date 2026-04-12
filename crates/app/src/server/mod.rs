use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;

// 定义基础路径
const TARGET_WASM_DIR: &str = "/Users/hal/.target/wasm32-unknown-unknown/release";

#[post("/api/i18n/translate")]
pub async fn translate_server(key: String, lang: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::PluginManager;
        let wasm_path = format!("{}/i18n_fluent_plugin.wasm", TARGET_WASM_DIR);
        if !std::path::Path::new(&wasm_path).exists() { return Ok(key); }
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
        let wasm_path = format!("{}/theme_ocean_plugin.wasm", TARGET_WASM_DIR);
        if !std::path::Path::new(&wasm_path).exists() { return Ok("".to_string()); }
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
        let client = match provider.as_str() {
            "github" => BasicClient::new(ClientId::new(config.github_client_id)).set_client_secret(ClientSecret::new(config.github_client_secret)).set_auth_uri(AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap()).set_token_uri(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()).set_redirect_uri(RedirectUrl::new(config.redirect_url).unwrap()),
            "google" => BasicClient::new(ClientId::new(config.google_client_id)).set_client_secret(ClientSecret::new(config.google_client_secret)).set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap()).set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()).set_redirect_uri(RedirectUrl::new(config.redirect_url).unwrap()),
            _ => return Err(ServerFnError::new("Unsupported provider")),
        };
        let (url, _) = client.authorize_url(CsrfToken::new_random).url();
        Ok(url.to_string())
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
      let data = base64::engine::general_purpose::STANDARD.decode(base64_str).map_err(|e| ServerFnError::new(format!("Failed to decode base64: {}", e)))?;
      let filename = format!("{}_{}", chrono::Utc::now().timestamp(), name);
      let dir_path = "assets/uploads";
      if !std::path::Path::new(dir_path).exists() { fs::create_dir_all(dir_path).map_err(|e| ServerFnError::new(e.to_string()))?; }
      let path = format!("{}/{}", dir_path, filename);
      fs::write(&path, &data).map_err(|e| ServerFnError::new(format!("Failed to write file: {}", e)))?;
      Ok(format!("/uploads/{}", filename))
  }
  #[cfg(not(feature = "server"))]
  { Ok("".to_string()) }
}

#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> {
  Ok(input)
}
