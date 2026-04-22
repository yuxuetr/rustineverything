use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site_name: String,
    pub site_description: String,
    pub active_theme: String,
    pub default_language: String,
    pub author: String,
    pub paths: HashMap<String, String>,
    pub navigation: Vec<NavItem>,
    #[serde(default)]
    pub auth: AuthSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    pub key: String,
    pub route: String,
}

/// 授权登录配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSettings {
    pub enabled: bool,
    pub providers: Vec<AuthProviderEntry>,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            providers: vec![],
        }
    }
}

/// 单个授权提供者配置项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProviderEntry {
    pub id: String,               // provider 标识，如 "github"
    pub plugin: String,           // 插件文件名，如 "github_auth_plugin.wasm"
}

impl SiteConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: SiteConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
}

impl Default for SiteConfig {
    fn default() -> Self {
        let mut paths = HashMap::new();
        paths.insert("plugins".to_string(), "assets/plugins".to_string());
        
        Self {
            site_name: "Rust in Everything".to_string(),
            site_description: "".to_string(),
            active_theme: "theme_ocean_plugin.wasm".to_string(),
            default_language: "zh".to_string(),
            author: "".to_string(),
            paths,
            navigation: vec![],
            auth: AuthSettings::default(),
        }
    }
}
