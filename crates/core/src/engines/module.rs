//! [`ModuleEngine`]：站点业务模块的注册中心 + 启用/禁用开关。
//!
//! ## 角色
//! - **声明**：每个内置模块（blog / podcast / forum / docs / course / cases / ...）
//!   通过 [`ModuleSpec`] 描述自己的 id、显示名、路由、导航位置等元数据。
//! - **注册**：应用启动时各模块调用 [`ModuleEngine::register`] 把自己加入
//!   引擎；可在 [`Engine::init`] 阶段读 `EngineContext::site_config` 决定
//!   `enabled` 状态。
//! - **查询**：导航 / 搜索索引 / sitemap 等子系统通过 [`ModuleEngine::enabled_modules`]
//!   或 [`ModuleEngine::navigation`] 拿到 enabled 模块列表。
//!
//! ## site.json 整合
//! [`crate::settings::SiteConfig`] 新增 `modules: HashMap<String, ModuleSettings>`：
//! ```json
//! {
//!   "modules": {
//!     "forum": { "enabled": false },
//!     "podcast": { "enabled": true }
//!   }
//! }
//! ```
//! `ModuleEngine::init` 从 ctx 中读取 `modules` map，覆盖每个 `ModuleSpec.enabled`。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::settings::SiteConfig;

/// 单个业务模块的元数据。模块 crate 自己提供。
///
/// Phase 8.7 新增字段：
/// - `nav_label_key`：i18n 翻译 key（如 `"nav-blog"`），让 navbar 不再硬编码标签文案
/// - `static_path`：模块对外的静态首页路径（如 `/blog`），让 sitemap 自动收录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleSpec {
  /// 唯一 id，对应 site.json::modules 的 key（如 "blog" / "forum"）。
  pub id: String,
  /// 用于导航 / 文档的显示名（如 "Blog" / "论坛"）。
  pub label: String,
  /// 模块拥有的顶层路由列表（如 ["/blog", "/blog/:id"]）。
  /// ModuleEngine 不直接消费这一字段；用于 sitemap / 文档生成参考。
  pub routes: Vec<String>,
  /// 在导航条中的位置：越小越靠前。`None` 不出现在导航。
  pub nav_position: Option<i32>,
  /// i18n key 用于 navbar 渲染 label；`None` 时用 `label` 字面值 fallback。
  /// 例如 `"nav-blog"` → 在 i18n_fluent_plugin 中查表得到 "Blog" / "博客"。
  #[serde(default)]
  pub nav_label_key: Option<String>,
  /// 模块对外的静态首页（如 `"/blog"`）。Sitemap 会自动把启用模块的这条路径加入。
  /// `None` 表示该模块不需要 sitemap 收录单独的索引页。
  #[serde(default)]
  pub static_path: Option<String>,
  /// 是否启用。默认 `true`，可被 site.json::modules 覆盖。
  pub enabled: bool,
}

impl ModuleSpec {
  pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      routes: Vec::new(),
      nav_position: None,
      nav_label_key: None,
      static_path: None,
      enabled: true,
    }
  }

  pub fn with_routes(mut self, routes: impl IntoIterator<Item = impl Into<String>>) -> Self {
    self.routes = routes.into_iter().map(|r| r.into()).collect();
    self
  }

  pub fn with_nav_position(mut self, pos: i32) -> Self {
    self.nav_position = Some(pos);
    self
  }

  pub fn with_nav_label_key(mut self, key: impl Into<String>) -> Self {
    self.nav_label_key = Some(key.into());
    self
  }

  pub fn with_static_path(mut self, path: impl Into<String>) -> Self {
    self.static_path = Some(path.into());
    self
  }

  pub fn disabled(mut self) -> Self {
    self.enabled = false;
    self
  }
}

/// 模块在 site.json 中的配置开关。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleSettings {
  #[serde(default = "default_enabled")]
  pub enabled: bool,
}

fn default_enabled() -> bool {
  true
}

/// 模块注册中心 + 引擎实现。
pub struct ModuleEngine {
  specs: Vec<ModuleSpec>,
  /// 由 `EngineContext` / `SiteConfig::modules` 读出的运行时覆盖。
  /// `init` 阶段填充，运行期不再修改。
  overrides: HashMap<String, ModuleSettings>,
}

/// Phase 3.4：站内内置模块清单。在 server fn 中使用。
///
/// 全部以 `enabled = true` 初始化，实际是否启用由
/// [`ModuleEngine::apply_site_config`] 读 `site.json::modules` 覆盖。
///
/// 这里不使用 `i18n` 翻译键以避免与插件耦合；前端用
/// `nav-{id}` 作为 i18n key（与现有 i18n_fluent_plugin 表一致）。
pub fn default_module_specs() -> Vec<ModuleSpec> {
  vec![
    ModuleSpec::new("blog", "Blog")
      .with_routes(["/blog", "/blog/:id"])
      .with_nav_position(10)
      .with_nav_label_key("nav-blog")
      .with_static_path("/blog"),
    ModuleSpec::new("podcast", "Podcast")
      .with_routes(["/podcast"])
      .with_nav_position(20)
      .with_nav_label_key("nav-podcast")
      .with_static_path("/podcast"),
    ModuleSpec::new("cases", "案例")
      .with_routes(["/case", "/case/:slug"])
      .with_nav_position(30)
      .with_nav_label_key("nav.cases")
      .with_static_path("/case"),
    ModuleSpec::new("forum", "论坛")
      .with_routes(["/topics", "/topics/new", "/topics/tag/:tag", "/topics/:id"])
      .with_nav_position(40)
      .with_nav_label_key("nav-forum")
      .with_static_path("/topics"),
    // Phase 6：内容板块（每个一条 SPA 路由 + 文章详情路由）。
    ModuleSpec::new("embedded", "嵌入式")
      .with_routes(["/embedded", "/embedded/:slug"])
      .with_nav_position(50)
      .with_nav_label_key("nav-embedded")
      .with_static_path("/embedded"),
    ModuleSpec::new("ai", "AI")
      .with_routes(["/ai", "/ai/:slug"])
      .with_nav_position(60)
      .with_nav_label_key("nav-ai")
      .with_static_path("/ai"),
    ModuleSpec::new("web3", "Web3")
      .with_routes(["/web3", "/web3/:slug"])
      .with_nav_position(70)
      .with_nav_label_key("nav-web3")
      .with_static_path("/web3"),
    ModuleSpec::new("wasm", "WASM")
      .with_routes(["/wasm", "/wasm/:slug"])
      .with_nav_position(80)
      .with_nav_label_key("nav-wasm")
      .with_static_path("/wasm"),
    ModuleSpec::new("cli", "CLI")
      .with_routes(["/cli", "/cli/:slug"])
      .with_nav_position(90)
      .with_nav_label_key("nav-cli")
      .with_static_path("/cli"),
    // 以下两个不出现在顶级 Navbar（`nav_position = None`），
    // 仅供 sitemap / 搜索 / 路由 gate 使用。
    ModuleSpec::new("course", "课程")
      .with_routes(["/course", "/course/:slug", "/course/:slug/:chapter/:lesson"])
      .with_nav_label_key("nav-course")
      .with_static_path("/course"),
    ModuleSpec::new("docs", "文档")
      .with_routes(["/docs", "/docs/*"])
      .with_nav_label_key("nav-docs")
      .with_static_path("/docs"),
  ]
}

/// 全局共享的 default_module_engine 缓存 slot（Phase 8.7）。
#[cfg(feature = "server")]
fn module_engine_slot() -> &'static std::sync::RwLock<Option<std::sync::Arc<ModuleEngine>>> {
  use std::sync::{Arc, OnceLock, RwLock};
  static CACHE: OnceLock<RwLock<Option<Arc<ModuleEngine>>>> = OnceLock::new();
  CACHE.get_or_init(|| RwLock::new(None))
}

/// Phase 3.4：由 site.json 装填的默认 ModuleEngine。
///
/// 仅在 server feature 下可用（需 `SiteConfig::from_file` + IO）。
///
/// Phase 8.7：内部加 `OnceLock<RwLock<Arc<ModuleEngine>>>` 缓存。原实现每次调用
/// 都做一次 fs 读 + serde_json 解析 site.json；该函数在 navbar / sitemap 等 hot
/// path 上被调用多次，浪费 IO + CPU。改造后：
/// - 首次调用：读 site.json 一次，写入 cache
/// - 后续命中：clone Arc，开销 O(1)
/// - admin 修改 site.json 后必须调 [`invalidate_default_module_engine`] 强制重读
#[cfg(feature = "server")]
pub fn default_module_engine() -> std::sync::Arc<ModuleEngine> {
  use std::sync::Arc;
  let slot = module_engine_slot();

  // 快路径：读锁拿到 Some 直接返回
  if let Ok(guard) = slot.read() {
    if let Some(arc) = guard.as_ref() {
      return arc.clone();
    }
  }

  // 慢路径：构建 + 写入。所有 IO 在锁外做以最小化争用。
  // S10：走 load_cached，与其它 site.json 读取点共享 mtime 缓存。
  let mut e = ModuleEngine::with_specs(default_module_specs());
  let site_path = crate::utils::get_asset_root().join("site.json");
  if let Ok(cfg) = SiteConfig::load_cached(site_path.to_str().unwrap_or_default()) {
    e.apply_site_config(&cfg);
  }
  let arc = Arc::new(e);
  if let Ok(mut guard) = slot.write() {
    *guard = Some(arc.clone());
  }
  arc
}

/// Phase 8.7：显式失效 default_module_engine 缓存。
/// admin server fn 写入 site.json 后必须调用，否则改动 7 天内（JWT TTL）
/// 都不会在 navbar / sitemap 生效。
#[cfg(feature = "server")]
pub fn invalidate_default_module_engine() {
  if let Ok(mut guard) = module_engine_slot().write() {
    *guard = None;
  }
}

impl Default for ModuleEngine {
  fn default() -> Self {
    Self::new()
  }
}

impl ModuleEngine {
  pub fn new() -> Self {
    Self { specs: Vec::new(), overrides: HashMap::new() }
  }

  /// 用预先填充的 specs 构造（便于测试 / 一次性注入多个模块）。
  pub fn with_specs<I: IntoIterator<Item = ModuleSpec>>(specs: I) -> Self {
    Self { specs: specs.into_iter().collect(), overrides: HashMap::new() }
  }

  /// 注册一个模块。重复 id 返回 `AppError::Other`。
  pub fn register(&mut self, spec: ModuleSpec) -> AppResult<()> {
    if self.specs.iter().any(|s| s.id == spec.id) {
      return Err(AppError::other(format!("模块已注册: {}", spec.id)));
    }
    self.specs.push(spec);
    Ok(())
  }

  /// 应用 site.json::modules 中的覆盖（init 阶段调用）。
  pub fn apply_site_config(&mut self, site: &SiteConfig) {
    self.overrides = site.modules.clone();
    for spec in self.specs.iter_mut() {
      if let Some(settings) = self.overrides.get(&spec.id) {
        spec.enabled = settings.enabled;
      }
    }
  }

  /// 检查某模块是否启用。未注册的 id 返回 `false`。
  pub fn is_enabled(&self, id: &str) -> bool {
    self.specs.iter().find(|s| s.id == id).map(|s| s.enabled).unwrap_or(false)
  }

  pub fn get(&self, id: &str) -> Option<&ModuleSpec> {
    self.specs.iter().find(|s| s.id == id)
  }

  /// 已启用的模块列表，按注册顺序。
  pub fn enabled_modules(&self) -> Vec<&ModuleSpec> {
    self.specs.iter().filter(|s| s.enabled).collect()
  }

  /// 用于导航生成的模块列表：仅 enabled 且声明了 nav_position 的，按位置升序。
  pub fn navigation(&self) -> Vec<&ModuleSpec> {
    let mut nav: Vec<&ModuleSpec> =
      self.specs.iter().filter(|s| s.enabled && s.nav_position.is_some()).collect();
    nav.sort_by_key(|s| s.nav_position.unwrap_or(i32::MAX));
    nav
  }

  /// 已启用模块的 id 列表（搜索 / sitemap 通过 id 过滤数据源）。
  pub fn enabled_ids(&self) -> Vec<String> {
    self.specs.iter().filter(|s| s.enabled).map(|s| s.id.clone()).collect()
  }

  pub fn len(&self) -> usize {
    self.specs.len()
  }

  pub fn is_empty(&self) -> bool {
    self.specs.is_empty()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn site_with_modules(pairs: &[(&str, bool)]) -> SiteConfig {
    let mut cfg = SiteConfig::default();
    for (id, enabled) in pairs {
      cfg.modules.insert(id.to_string(), ModuleSettings { enabled: *enabled });
    }
    cfg
  }

  #[test]
  fn module_spec_builder() {
    let spec =
      ModuleSpec::new("blog", "Blog").with_routes(["/blog", "/blog/:id"]).with_nav_position(10);
    assert_eq!(spec.id, "blog");
    assert_eq!(spec.label, "Blog");
    assert_eq!(spec.routes, vec!["/blog", "/blog/:id"]);
    assert_eq!(spec.nav_position, Some(10));
    assert!(spec.enabled);
  }

  #[test]
  fn register_and_query() {
    let mut e = ModuleEngine::new();
    e.register(ModuleSpec::new("blog", "Blog").with_nav_position(1)).unwrap();
    e.register(ModuleSpec::new("forum", "Forum").with_nav_position(2)).unwrap();
    assert_eq!(e.len(), 2);
    assert!(e.is_enabled("blog"));
    assert!(e.is_enabled("forum"));
    assert!(!e.is_enabled("nonexistent"));
  }

  #[test]
  fn register_duplicate_id_fails() {
    let mut e = ModuleEngine::new();
    e.register(ModuleSpec::new("blog", "Blog")).unwrap();
    let err = e.register(ModuleSpec::new("blog", "Blog 2")).unwrap_err();
    assert!(matches!(err, AppError::Other(_)));
    assert_eq!(e.len(), 1);
  }

  #[test]
  fn site_config_disables_module() {
    let mut e = ModuleEngine::new();
    e.register(ModuleSpec::new("blog", "Blog")).unwrap();
    e.register(ModuleSpec::new("forum", "Forum")).unwrap();

    // 默认全启用
    assert_eq!(e.enabled_ids(), vec!["blog", "forum"]);

    // 通过 site.json::modules 关闭 forum
    e.apply_site_config(&site_with_modules(&[("forum", false)]));
    assert!(e.is_enabled("blog"));
    assert!(!e.is_enabled("forum"));
    assert_eq!(e.enabled_ids(), vec!["blog".to_string()]);
  }

  #[test]
  fn navigation_filters_by_enabled_and_position() {
    let mut e = ModuleEngine::new();
    e.register(ModuleSpec::new("blog", "Blog").with_nav_position(20)).unwrap();
    e.register(ModuleSpec::new("podcast", "Podcast").with_nav_position(10)).unwrap();
    e.register(ModuleSpec::new("forum", "Forum").with_nav_position(30)).unwrap();
    // 没有 nav_position 的不出现在导航
    e.register(ModuleSpec::new("admin", "Admin")).unwrap();

    e.apply_site_config(&site_with_modules(&[("forum", false)]));

    let nav = e.navigation();
    let ids: Vec<&str> = nav.iter().map(|s| s.id.as_str()).collect();
    // forum 被关闭 / admin 没有 nav_position，所以不在导航中
    assert_eq!(ids, vec!["podcast", "blog"]);
  }

  #[test]
  fn search_sources_only_includes_enabled() {
    let mut e = ModuleEngine::new();
    e.register(ModuleSpec::new("blog", "Blog")).unwrap();
    e.register(ModuleSpec::new("forum", "Forum")).unwrap();
    e.register(ModuleSpec::new("docs", "Docs")).unwrap();

    e.apply_site_config(&site_with_modules(&[("forum", false), ("docs", true)]));

    let ids = e.enabled_ids();
    assert!(ids.contains(&"blog".to_string()));
    assert!(ids.contains(&"docs".to_string()));
    assert!(!ids.contains(&"forum".to_string()));
  }

  #[test]
  fn module_settings_serde_default_enabled() {
    // site.json: { "modules": { "blog": {} } } → enabled 默认为 true
    let m: ModuleSettings = serde_json::from_str("{}").unwrap();
    assert!(m.enabled);
  }

  // Phase 8.7：删除了 `Engine` trait 后，原 engine_implements_trait 测试
  // 不再有意义；ModuleEngine 现在就是普通 struct。

  #[test]
  fn with_specs_constructor() {
    let specs = vec![ModuleSpec::new("blog", "Blog"), ModuleSpec::new("podcast", "Podcast")];
    let e = ModuleEngine::with_specs(specs);
    assert_eq!(e.len(), 2);
  }

  #[test]
  fn nav_sort_stable_with_equal_positions() {
    let mut e = ModuleEngine::new();
    e.register(ModuleSpec::new("a", "A").with_nav_position(10)).unwrap();
    e.register(ModuleSpec::new("b", "B").with_nav_position(10)).unwrap();
    let nav = e.navigation();
    // 两个相同位置时应保持注册顺序（sort_by_key 是稳定的）
    assert_eq!(nav[0].id, "a");
    assert_eq!(nav[1].id, "b");
  }

  /// Phase 8.7：cache 命中验证 —— 多次调用返回同一 Arc。
  #[cfg(feature = "server")]
  #[test]
  fn default_module_engine_returns_same_arc_on_repeat_calls() {
    // 在 fresh 状态下：第一次调用会读 site.json（可能不存在，没关系），
    // 第二次起都该走 cache。
    super::invalidate_default_module_engine();
    let a = super::default_module_engine();
    let b = super::default_module_engine();
    assert!(std::sync::Arc::ptr_eq(&a, &b), "cache hit 应当返回同一 Arc instance");
  }

  /// invalidate 后必须重建。
  #[cfg(feature = "server")]
  #[test]
  fn invalidate_forces_default_module_engine_to_rebuild() {
    super::invalidate_default_module_engine();
    let a = super::default_module_engine();
    super::invalidate_default_module_engine();
    let b = super::default_module_engine();
    assert!(!std::sync::Arc::ptr_eq(&a, &b), "invalidate 后应当重建为新 Arc");
  }

  /// nav_label_key / static_path 字段在默认 specs 上填好了。
  #[test]
  fn default_specs_have_nav_label_keys_and_static_paths() {
    let specs = default_module_specs();
    // 每个 spec 至少有 nav_label_key
    for s in &specs {
      assert!(s.nav_label_key.is_some(), "{} missing nav_label_key", s.id);
    }
    // 每个 spec 至少有 static_path（11 模块全有首页）
    for s in &specs {
      assert!(s.static_path.is_some(), "{} missing static_path", s.id);
    }
    // 抽几个验证具体值
    let by_id = |id: &str| specs.iter().find(|s| s.id == id).unwrap();
    assert_eq!(by_id("blog").static_path.as_deref(), Some("/blog"));
    assert_eq!(by_id("cases").static_path.as_deref(), Some("/case"));
    assert_eq!(by_id("forum").static_path.as_deref(), Some("/topics"));
    assert_eq!(by_id("blog").nav_label_key.as_deref(), Some("nav-blog"));
  }

  #[test]
  fn default_module_specs_contains_all_builtins() {
    let specs = default_module_specs();
    let ids: Vec<&str> = specs.iter().map(|s| s.id.as_str()).collect();
    for id in [
      "blog", "podcast", "course", "forum", "cases", "docs", "embedded", "ai", "web3", "wasm",
      "cli",
    ] {
      assert!(ids.contains(&id), "缺少内置模块: {}", id);
    }
    assert_eq!(specs.len(), 11);
  }

  #[test]
  fn default_specs_navbar_order() {
    // 默认 nav: blog / podcast / cases / forum + 5 个内容板块（剩下 course/docs nav_position = None）
    let specs = default_module_specs();
    let mut sorted: Vec<&ModuleSpec> = specs.iter().filter(|s| s.nav_position.is_some()).collect();
    sorted.sort_by_key(|s| s.nav_position.unwrap_or(i32::MAX));
    let ordered: Vec<&str> = sorted.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
      ordered,
      vec!["blog", "podcast", "cases", "forum", "embedded", "ai", "web3", "wasm", "cli"]
    );
  }

  #[test]
  fn disabling_forum_propagates_to_all_consumer_views() {
    // Phase 3.6 验收门禁：关闭 forum 后所有消费者视图一致地排除它。
    let mut e = ModuleEngine::with_specs(default_module_specs());
    let mut cfg = SiteConfig::default();
    cfg.modules.insert("forum".to_string(), ModuleSettings { enabled: false });
    e.apply_site_config(&cfg);

    // 1) is_enabled 单点查询
    assert!(!e.is_enabled("forum"));
    assert!(e.is_enabled("blog"));
    assert!(e.is_enabled("docs"));

    // 2) navigation 中不出现
    let nav_ids: Vec<&str> = e.navigation().iter().map(|s| s.id.as_str()).collect();
    assert!(!nav_ids.contains(&"forum"));
    assert_eq!(nav_ids, vec!["blog", "podcast", "cases", "embedded", "ai", "web3", "wasm", "cli"]);

    // 3) enabled_ids 中不出现 - 即搜索源 / sitemap 都会过滤掉 forum
    let enabled = e.enabled_ids();
    assert!(!enabled.contains(&"forum".to_string()));
    // 但其他 5 个内置模块都在
    for id in ["blog", "podcast", "course", "cases", "docs"] {
      assert!(enabled.contains(&id.to_string()), "缺少 {}", id);
    }

    // 4) enabled_modules() spec 列表也不含 forum
    let mods: Vec<&str> = e.enabled_modules().iter().map(|s| s.id.as_str()).collect();
    assert!(!mods.contains(&"forum"));
  }

  #[test]
  fn applying_disabled_overrides_drops_from_navigation() {
    let mut e = ModuleEngine::with_specs(default_module_specs());
    let mut cfg = SiteConfig::default();
    cfg.modules.insert("forum".to_string(), ModuleSettings { enabled: false });
    cfg.modules.insert("blog".to_string(), ModuleSettings { enabled: false });
    e.apply_site_config(&cfg);
    let ids: Vec<&str> = e.navigation().iter().map(|s| s.id.as_str()).collect();
    // 关闭 blog + forum 后剩 podcast / cases + 5 个内容板块
    assert_eq!(ids, vec!["podcast", "cases", "embedded", "ai", "web3", "wasm", "cli"]);
    // enabled_ids 里也不含被关闭的 (但仍包含未出现在 nav 的 course / docs)
    let enabled = e.enabled_ids();
    assert!(!enabled.contains(&"forum".to_string()));
    assert!(!enabled.contains(&"blog".to_string()));
    assert!(enabled.contains(&"course".to_string()));
    assert!(enabled.contains(&"docs".to_string()));
  }
}
