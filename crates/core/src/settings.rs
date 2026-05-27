use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::engines::module::ModuleSettings;

/// 默认布局名（Phase 3.3）。
pub const DEFAULT_LAYOUT: &str = "classic";

fn default_layout() -> String {
    DEFAULT_LAYOUT.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site_name: String,
    pub site_description: String,
    /// **单插件主题**（向后兼容）：当 [`SiteConfig::themes`] 为空时回退使用。
    /// 新代码请通过 [`SiteConfig::theme_stack`] 读取，不要直接读该字段。
    pub active_theme: String,
    /// 主题栈（Phase 3.1）：声明顺序 = CSS 输出顺序，后者覆盖前者。
    /// 元素为插件文件名（如 `theme_ocean_plugin.wasm`）。
    /// `#[serde(default)]` 保证旧 site.json 不破坏。
    #[serde(default)]
    pub themes: Vec<String>,
    /// 当前生效布局（Phase 3.3）：默认 `"classic"`。
    /// 受 [`crate::engines::layout::LayoutEngine`] 注册项约束；不存在时
    /// `LayoutShell` 应回退到 classic。
    #[serde(default = "default_layout")]
    pub active_layout: String,
    pub default_language: String,
    pub author: String,
    pub paths: HashMap<String, String>,
    pub navigation: Vec<NavItem>,
    #[serde(default)]
    pub auth: AuthSettings,
    /// 模块开关：key = ModuleSpec.id，value = `{ enabled: true|false }`。
    /// `ModuleEngine::init` 读取该字段覆盖默认 enabled 状态。
    #[serde(default)]
    pub modules: HashMap<String, ModuleSettings>,
    /// Phase 4.3：审核插件配置（独立于通用 `modules` 开关，因为还要装
    /// 插件清单与阈值）。
    /// 默认 disabled + 空插件列表 → 整条流水线短路，零开销 fail-open。
    /// 由 `crates/modules/moderation::ModerationPipeline::from_site_config`
    /// 读取并装载。
    #[serde(default)]
    pub moderation: ModerationSettings,
}

/// 审核功能在 site.json 中的配置块。**默认 disabled**，意味着评论 / 话题 /
/// 标注的提交路径不会调用任何 LLM。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModerationSettings {
    /// 总开关。默认 `false`。
    #[serde(default)]
    pub enabled: bool,
    /// 要装载的审核插件文件名列表（相对 `assets/plugins/`）。
    /// 即使 `enabled = true`，这里为空也等于没有 stage → 全部 Allow。
    #[serde(default)]
    pub plugins: Vec<String>,
    /// 可选：覆盖默认阈值（block_above = 0.9 / flag_above = 0.5）。
    /// 留空表示用默认。
    #[serde(default)]
    pub thresholds: Option<ModerationThresholdsConfig>,
}

/// `ModerationSettings::thresholds` 的可选覆盖。字段缺失时不影响其它字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModerationThresholdsConfig {
    pub block_above: Option<f32>,
    pub flag_above: Option<f32>,
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
    /// 从路径读取并解析 site.json。
    ///
    /// 返回统一的 [`crate::error::AppResult`]，底层 IO 失败会变为
    /// `AppError::Io`，不合法 JSON 会变为 `AppError::Validation`。
    pub fn from_file(path: &str) -> crate::error::AppResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: SiteConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 解析后的主题栈（Phase 3.1）。
    ///
    /// 优先级：
    /// 1. 非空 `themes` 直接返回 clone
    /// 2. 否则若 `active_theme` 非空，返回单元素栈（向后兼容）
    /// 3. 否则返回空 Vec
    pub fn theme_stack(&self) -> Vec<String> {
        if !self.themes.is_empty() {
            return self.themes.clone();
        }
        if !self.active_theme.is_empty() {
            return vec![self.active_theme.clone()];
        }
        Vec::new()
    }

    /// 当前布局名（空字符串回退到 `"classic"`）。
    pub fn active_layout_or_default(&self) -> &str {
        if self.active_layout.is_empty() {
            DEFAULT_LAYOUT
        } else {
            self.active_layout.as_str()
        }
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
            themes: Vec::new(),
            active_layout: DEFAULT_LAYOUT.to_string(),
            default_language: "zh".to_string(),
            author: "".to_string(),
            paths,
            navigation: vec![],
            auth: AuthSettings::default(),
            modules: HashMap::new(),
            moderation: ModerationSettings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_stack_prefers_themes_field() {
        let mut cfg = SiteConfig::default();
        cfg.themes = vec!["a.wasm".to_string(), "b.wasm".to_string()];
        cfg.active_theme = "legacy.wasm".to_string();
        assert_eq!(cfg.theme_stack(), vec!["a.wasm", "b.wasm"]);
    }

    #[test]
    fn theme_stack_falls_back_to_active_theme() {
        let mut cfg = SiteConfig::default();
        cfg.themes = Vec::new();
        cfg.active_theme = "legacy.wasm".to_string();
        assert_eq!(cfg.theme_stack(), vec!["legacy.wasm"]);
    }

    #[test]
    fn theme_stack_empty_when_both_empty() {
        let mut cfg = SiteConfig::default();
        cfg.themes = Vec::new();
        cfg.active_theme = String::new();
        assert!(cfg.theme_stack().is_empty());
    }

    #[test]
    fn active_layout_falls_back_to_classic() {
        let mut cfg = SiteConfig::default();
        cfg.active_layout = String::new();
        assert_eq!(cfg.active_layout_or_default(), "classic");
        cfg.active_layout = "minimal".to_string();
        assert_eq!(cfg.active_layout_or_default(), "minimal");
    }

    #[test]
    fn deserialize_missing_themes_and_layout_uses_defaults() {
        // Phase 3.1 之前的 site.json 不含 themes / active_layout 字段
        let json = r#"{
            "site_name": "X",
            "site_description": "",
            "active_theme": "theme_ocean_plugin.wasm",
            "default_language": "zh",
            "author": "",
            "paths": {},
            "navigation": []
        }"#;
        let cfg: SiteConfig = serde_json::from_str(json).expect("parse");
        assert!(cfg.themes.is_empty());
        assert_eq!(cfg.active_layout, "classic");
        assert_eq!(
            cfg.theme_stack(),
            vec!["theme_ocean_plugin.wasm".to_string()]
        );
    }

    // ─── Phase 4.3 moderation settings ───────────────────────

    #[test]
    fn moderation_defaults_to_disabled_empty() {
        let cfg = SiteConfig::default();
        assert!(!cfg.moderation.enabled);
        assert!(cfg.moderation.plugins.is_empty());
        assert!(cfg.moderation.thresholds.is_none());
    }

    #[test]
    fn moderation_back_compat_when_field_missing() {
        // 老 site.json 完全没有 moderation block → 默认 disabled
        let json = r#"{
            "site_name": "X",
            "site_description": "",
            "active_theme": "t.wasm",
            "default_language": "zh",
            "author": "",
            "paths": {},
            "navigation": []
        }"#;
        let cfg: SiteConfig = serde_json::from_str(json).expect("parse");
        assert!(!cfg.moderation.enabled);
    }

    #[test]
    fn moderation_parses_full_block() {
        let json = r#"{
            "site_name": "X",
            "site_description": "",
            "active_theme": "t.wasm",
            "default_language": "zh",
            "author": "",
            "paths": {},
            "navigation": [],
            "moderation": {
                "enabled": true,
                "plugins": ["moderation_llm_default.wasm"],
                "thresholds": { "block_above": 0.95, "flag_above": 0.6 }
            }
        }"#;
        let cfg: SiteConfig = serde_json::from_str(json).expect("parse");
        assert!(cfg.moderation.enabled);
        assert_eq!(cfg.moderation.plugins, vec!["moderation_llm_default.wasm"]);
        let t = cfg.moderation.thresholds.unwrap();
        assert_eq!(t.block_above, Some(0.95));
        assert_eq!(t.flag_above, Some(0.6));
    }
}
