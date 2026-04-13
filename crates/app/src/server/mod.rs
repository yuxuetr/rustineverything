use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use rustineverything_core::settings::SiteConfig;
use std::path::PathBuf;

/// 自动探测资产根目录 (Native 端逻辑)
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        // 兼容开发环境路径
        path = PathBuf::from("../../assets");
    }
    path
}

/// [新增] 动态资产分发器：允许前端访问根目录 assets
/// 使用场景：播客音频、用户上传的大图、插件 WASM
#[post("/api/assets/stream")]
pub async fn stream_asset(path: String) -> Result<Vec<u8>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let full_path = get_asset_root().join(path.trim_start_matches('/'));
        if !full_path.exists() {
            return Err(ServerFnError::new(format!("资产不存在: {:?}", full_path)));
        }
        
        fs::read(&full_path).map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { Err(ServerFnError::new("仅服务端可用")) }
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
        let config = get_site_config().await.unwrap_or_default();
        let wasm_path = get_asset_root().join("plugins/i18n_fluent_plugin.wasm");
        
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
        let config = get_site_config().await.unwrap_or_default();
        let wasm_path = get_asset_root().join("plugins").join(&config.active_theme);
        
        if !wasm_path.exists() { return Ok("".to_string()); }
        let wasm_bytes = fs::read(wasm_path).map_err(|e| ServerFnError::new(e.to_string()))?;
        let manager = PluginManager::new();
        Ok(manager.aggregate_theme_css(&[wasm_bytes]))
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
      
      // 注意：这里返回的不再是标准 URL，而是给前端 stream_asset 用的路径
      Ok(format!("uploads/{}", filename))
  }
  #[cfg(not(feature = "server"))]
  { Ok("".to_string()) }
}

#[post("/api/content/blog")]
pub async fn get_blog_content(id: String) -> Result<String, ServerFnError> {
  let filename = match id.as_str() {
    "1" => "content/welcome.md",
    "2" => "blog/2026-01-10-python-struct/index.mdx",
    _ => return Err(ServerFnError::new("文章未找到")),
  };

  let filepath = get_asset_root().join(filename);
  fs::read_to_string(&filepath)
    .map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))
}

#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> { Ok(input) }
