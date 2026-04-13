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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    pub key: String,
    pub route: String,
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
        }
    }
}
