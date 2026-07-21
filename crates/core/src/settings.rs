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
  /// Phase 9.2：插件 SHA256 lock map。key = 插件文件名（如
  /// `theme_ocean_plugin.wasm`），value = 期望的 SHA256 hex（64 字符小写）。
  ///
  /// 缺字段或值为空 → 加载时跳过校验（warn-only）；命中且不匹配 → 拒绝加载。
  /// 用 `cargo run -p app --bin lock_plugins` 一键生成。
  ///
  /// 防御场景：插件文件在文件系统层被偷换（供应链攻击 / 误覆盖）。
  #[serde(default)]
  pub plugins_lock: HashMap<String, String>,
  /// Phase 9.3：内容变换器插件文件名列表（相对 `assets/plugins/`），按顺序 chain
  /// 调用 —— 前一个的输出作为下一个的输入。声明顺序 = 执行顺序。
  ///
  /// 空列表（默认）→ ContentTransformerEngine 不实例化 / 不触发 wasm，零开销。
  /// fail-open：单个插件 trap / timeout / 非法输出，会跳过该插件继续。
  #[serde(default)]
  pub content_transformers: Vec<String>,
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
  /// URL 域名黑名单。命中即 Block（score=1.0），不走 LLM。
  /// 匹配规则（不区分大小写）：
  /// - 精确：`"scam.com"` 只匹配 host = `scam.com`
  /// - 通配：`"*.phishing.example"` 匹配 `sub.phishing.example` 与
  ///   `phishing.example` 本身
  ///
  /// 默认空 → 该 stage 不注册，零开销。
  #[serde(default)]
  pub url_blocklist: Vec<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthSettings {
  pub enabled: bool,
  pub providers: Vec<AuthProviderEntry>,
}

/// 单个授权提供者配置项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProviderEntry {
  pub id: String,     // provider 标识，如 "github"
  pub plugin: String, // 插件文件名，如 "github_auth_plugin.wasm"
}

impl SiteConfig {
  /// 从路径读取并解析 site.json（无缓存，每次磁盘 IO）。
  ///
  /// 适用于写后立即重读等必须绕过缓存的场景（admin 保存配置）；
  /// 读多写少的热路径请用 [`SiteConfig::load_cached`]。
  ///
  /// 返回统一的 [`crate::error::AppResult`]，底层 IO 失败会变为
  /// `AppError::Io`，不合法 JSON 会变为 `AppError::Validation`。
  pub fn from_file(path: &str) -> crate::error::AppResult<Self> {
    let content = std::fs::read_to_string(path)?;
    let config: SiteConfig = serde_json::from_str(&content)?;
    Ok(config)
  }

  /// S10（风险 R11）：按 (path, mtime) 缓存的统一读取入口。
  ///
  /// 此前多处 server fn（主题 CSS / 布局 / feed / auth）每次请求都
  /// `from_file` 直读磁盘 + serde 解析。改用本入口后：
  /// - mtime 未变 → 直接 clone Arc（一次 metadata 系统调用，无读盘/解析）
  /// - mtime 变化（admin 保存 / 手改文件）→ 自动重读，无需显式失效
  ///
  /// 缓存 key 为路径字符串；实际部署只有一个 site.json，表容量恒为 1。
  pub fn load_cached(path: &str) -> crate::error::AppResult<std::sync::Arc<Self>> {
    use std::collections::HashMap as Map;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::SystemTime;

    struct Entry {
      mtime: SystemTime,
      cfg: Arc<SiteConfig>,
    }
    static CACHE: OnceLock<Mutex<Map<String, Entry>>> = OnceLock::new();

    let mtime = std::fs::metadata(path)?.modified()?;
    let cache = CACHE.get_or_init(|| Mutex::new(Map::new()));

    if let Ok(guard) = cache.lock() {
      if let Some(entry) = guard.get(path) {
        if entry.mtime == mtime {
          return Ok(entry.cfg.clone());
        }
      }
    }

    // 未命中 / mtime 变化 → 锁外重读，写回缓存（锁失败仅跳过本次缓存）。
    let cfg = Arc::new(Self::from_file(path)?);
    if let Ok(mut guard) = cache.lock() {
      guard.insert(path.to_string(), Entry { mtime, cfg: cfg.clone() });
    }
    Ok(cfg)
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
      plugins_lock: HashMap::new(),
      content_transformers: Vec::new(),
    }
  }
}

impl SiteConfig {
  /// Phase 9.2：查询某插件的预期 SHA256 hex。`None` 表示 site.json 未给该
  /// 插件登记 lock（warn-only 模式，调用方应记日志但放行）。
  pub fn plugin_sha256(&self, plugin_file_name: &str) -> Option<&str> {
    self.plugins_lock.get(plugin_file_name).map(String::as_str).filter(|s| !s.is_empty())
  }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // 测试 setup：Default + 逐字段赋值更易读
mod tests {
  use super::*;

  // ─── S10 load_cached ──────────────────────────────────

  fn minimal_site_json(name: &str) -> String {
    format!(
      r#"{{"site_name":"{}","site_description":"","active_theme":"t.wasm","default_language":"zh","author":"","paths":{{}},"navigation":[]}}"#,
      name
    )
  }

  /// 同 mtime 下重复读取应命中缓存（返回同一个 Arc）。
  #[test]
  fn load_cached_hits_on_same_mtime() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("site.json");
    std::fs::write(&path, minimal_site_json("A")).expect("write");
    let p = path.to_str().expect("utf8 path");

    let first = SiteConfig::load_cached(p).expect("first load");
    let second = SiteConfig::load_cached(p).expect("second load");
    assert!(std::sync::Arc::ptr_eq(&first, &second), "同 mtime 应返回同一 Arc");
    assert_eq!(first.site_name, "A");
  }

  /// mtime 变化后应自动重读新内容。
  #[test]
  fn load_cached_reloads_on_mtime_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("site.json");
    std::fs::write(&path, minimal_site_json("OLD")).expect("write old");
    let p = path.to_str().expect("utf8 path");
    let first = SiteConfig::load_cached(p).expect("first load");
    assert_eq!(first.site_name, "OLD");

    // 写新内容并显式把 mtime 向后拨 2 秒（避免文件系统 mtime 粒度引发 flaky）
    std::fs::write(&path, minimal_site_json("NEW")).expect("write new");
    let f = std::fs::OpenOptions::new().append(true).open(&path).expect("open");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    f.set_modified(later).expect("set mtime");

    let second = SiteConfig::load_cached(p).expect("reload");
    assert_eq!(second.site_name, "NEW", "mtime 变化应重读");
  }

  #[test]
  fn load_cached_missing_file_errors() {
    assert!(SiteConfig::load_cached("/nonexistent/__site__.json").is_err());
  }

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
    assert_eq!(cfg.theme_stack(), vec!["theme_ocean_plugin.wasm".to_string()]);
  }

  // ─── Phase 4.3 moderation settings ───────────────────────

  #[test]
  fn moderation_defaults_to_disabled_empty() {
    let cfg = SiteConfig::default();
    assert!(!cfg.moderation.enabled);
    assert!(cfg.moderation.plugins.is_empty());
    assert!(cfg.moderation.thresholds.is_none());
    assert!(cfg.moderation.url_blocklist.is_empty());
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

  // ─── Phase 9.2 plugins_lock ──────────────────────────────

  #[test]
  fn plugins_lock_defaults_to_empty() {
    let cfg = SiteConfig::default();
    assert!(cfg.plugins_lock.is_empty());
  }

  #[test]
  fn plugins_lock_back_compat_when_field_missing() {
    // 老 site.json 不含 plugins_lock 字段 → 默认空
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
    assert!(cfg.plugins_lock.is_empty());
  }

  #[test]
  fn plugins_lock_parses_entries() {
    let json = r#"{
            "site_name": "X",
            "site_description": "",
            "active_theme": "t.wasm",
            "default_language": "zh",
            "author": "",
            "paths": {},
            "navigation": [],
            "plugins_lock": {
                "theme_ocean_plugin.wasm": "deadbeef",
                "i18n_fluent_plugin.wasm": "cafebabe"
            }
        }"#;
    let cfg: SiteConfig = serde_json::from_str(json).expect("parse");
    assert_eq!(cfg.plugin_sha256("theme_ocean_plugin.wasm"), Some("deadbeef"));
    assert_eq!(cfg.plugin_sha256("i18n_fluent_plugin.wasm"), Some("cafebabe"));
    assert_eq!(cfg.plugin_sha256("unknown.wasm"), None);
  }

  #[test]
  fn plugins_lock_empty_string_treated_as_missing() {
    let mut cfg = SiteConfig::default();
    cfg.plugins_lock.insert("x.wasm".into(), String::new());
    assert_eq!(cfg.plugin_sha256("x.wasm"), None, "empty string should be treated as missing");
  }

  // ─── Phase 9.3 content_transformers ───────────────────────

  #[test]
  fn content_transformers_default_empty() {
    let cfg = SiteConfig::default();
    assert!(cfg.content_transformers.is_empty());
  }

  #[test]
  fn content_transformers_back_compat_when_field_missing() {
    // 老 site.json 不含 content_transformers 字段 → 默认空
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
    assert!(cfg.content_transformers.is_empty());
  }

  #[test]
  fn content_transformers_parses_ordered_list() {
    let json = r#"{
            "site_name": "X",
            "site_description": "",
            "active_theme": "t.wasm",
            "default_language": "zh",
            "author": "",
            "paths": {},
            "navigation": [],
            "content_transformers": [
                "content_toc_plugin.wasm",
                "content_lazyload_plugin.wasm"
            ]
        }"#;
    let cfg: SiteConfig = serde_json::from_str(json).expect("parse");
    assert_eq!(cfg.content_transformers.len(), 2);
    assert_eq!(cfg.content_transformers[0], "content_toc_plugin.wasm");
    assert_eq!(cfg.content_transformers[1], "content_lazyload_plugin.wasm");
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
