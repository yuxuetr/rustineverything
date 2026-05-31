//! [`ThemeEngine`] 骨架：聚合主题 WASM 插件输出的 CSS。
//!
//! ## Phase 3.1 能力
//! - **主题栈**：`SiteConfig.themes` 声明顺序即 CSS 输出顺序，后者覆盖前者。
//!   当 `themes` 为空时回退到单插件 `active_theme`。
//! - **用户覆盖**：[`theme_with_override`] 接收可选的 cookie 主题文件名，
//!   存在时覆盖栈的最后一项（表示“用户选择的最顶层主题”）。
//! - **插件调用**：通过 [`PluginEngine`] 调 `get_theme_css`，调用失败跳过不阻断其他主题。
//! - **`init`**：调用 [`ThemeEngine::apply_site_config`] 使用 [`SiteConfig::theme_stack`]。

use std::path::PathBuf;
use std::sync::Arc;

use super::plugin::PluginEngine;
use super::{Engine, EngineContext};
use crate::error::AppResult;
use crate::settings::SiteConfig;

/// ThemeEngine：管理主题插件路径列表，按顺序聚合 CSS。
pub struct ThemeEngine {
  plugin: Arc<PluginEngine>,
  /// 当前生效的主题插件路径列表。索引顺序即覆盖顺序（后者覆盖前者）。
  themes: Vec<PathBuf>,
}

impl ThemeEngine {
  pub fn new(plugin: Arc<PluginEngine>) -> Self {
    Self { plugin, themes: Vec::new() }
  }

  /// 注册一个主题插件路径。可重复调用。
  pub fn register_theme(&mut self, path: PathBuf) {
    self.themes.push(path);
  }

  /// 替换全部主题栈。
  pub fn set_themes(&mut self, themes: Vec<PathBuf>) {
    self.themes = themes;
  }

  /// 当前注册的主题插件路径列表（只读）。
  pub fn themes(&self) -> &[PathBuf] {
    &self.themes
  }

  /// 从 [`SiteConfig::theme_stack`] 装填主题栈（Phase 3.1）。
  ///
  /// `asset_root` 需指向资产根目录（`assets`），本函数拼接
  /// `asset_root/plugins/<filename>` 作为插件路径。会清空现有栈。
  pub fn apply_site_config(&mut self, site: &SiteConfig, asset_root: &std::path::Path) {
    let plugin_dir = asset_root.join("plugins");
    self.themes = site
      .theme_stack()
      .into_iter()
      .filter(|name| !name.is_empty())
      .map(|name| plugin_dir.join(name))
      .collect();
  }

  /// 聚合所有主题插件的 CSS。失败的插件会被跳过（不阻断其他主题）。
  #[cfg(feature = "server")]
  pub async fn aggregate_css(&self) -> String {
    let mut out = String::new();
    for path in &self.themes {
      match self.plugin.call(path, "get_theme_css", "").await {
        Ok(css) => {
          out.push_str(&css);
          out.push('\n');
        }
        Err(e) => {
          tracing::warn!(theme = %path.display(), error = %e, "theme: skipping plugin");
        }
      }
    }
    out
  }
}

/// 纯函数：从主题栈 + 可选 cookie 覆盖计算实际生效的插件路径列表。
///
/// 语义：`override_filename` 为用户 cookie 中的主题文件名（如 `theme_sunset_plugin.wasm`）：
/// - `None` / 空串 → 保留原栈（仅过滤不存在的文件）
/// - `Some(name)` 且 `plugin_dir/name` 存在 → 覆盖栈最后一项（若栈为空则追加一项）
/// - `Some(name)` 但文件不存在 → 记录 stderr，保留原栈
///
/// 提供为 `pub` 以便在 server fn、单测、未来 admin 预览中复用。
pub fn theme_with_override(
  stack: &[PathBuf],
  plugin_dir: &std::path::Path,
  override_filename: Option<&str>,
) -> Vec<PathBuf> {
  // 1. 过滤不存在的文件，避免后续 wasmi 加载失败
  let mut result: Vec<PathBuf> = stack.iter().filter(|p| p.exists()).cloned().collect();

  // 2. 覆盖最后一项
  let candidate =
    override_filename.map(str::trim).filter(|s| !s.is_empty()).map(|name| plugin_dir.join(name));
  if let Some(path) = candidate {
    if path.exists() {
      if result.is_empty() {
        result.push(path);
      } else {
        let last = result.len() - 1;
        result[last] = path;
      }
    } else {
      tracing::warn!(
          theme = %path.display(),
          "theme: cookie override file not found, ignoring"
      );
    }
  }

  result
}

impl Engine for ThemeEngine {
  fn name(&self) -> &'static str {
    "theme"
  }

  fn init(&mut self, ctx: &EngineContext) -> AppResult<()> {
    // Phase 3.1：走主题栈语义，成为唯一装填路径。
    self.apply_site_config(&ctx.site_config, &ctx.asset_root);
    Ok(())
  }

  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
    self
  }
}

#[cfg(all(test, feature = "server"))]
#[allow(clippy::field_reassign_with_default)] // 测试 setup：Default + 逐字段赋值更易读
mod tests {
  use super::*;
  use std::fs;

  fn make() -> ThemeEngine {
    let pm = Arc::new(crate::PluginManager::new());
    let pe = Arc::new(PluginEngine::new(pm));
    ThemeEngine::new(pe)
  }

  /// 在给定目录下创建占位主题文件，返回全路径。
  fn touch_plugin(dir: &std::path::Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, b"\0asm").expect("write fake wasm");
    p
  }

  #[test]
  fn name_is_theme() {
    let e = make();
    assert_eq!(<ThemeEngine as Engine>::name(&e), "theme");
  }

  #[test]
  fn register_and_set() {
    let mut e = make();
    e.register_theme(PathBuf::from("a.wasm"));
    e.register_theme(PathBuf::from("b.wasm"));
    assert_eq!(e.themes().len(), 2);
    e.set_themes(vec![PathBuf::from("c.wasm")]);
    assert_eq!(e.themes(), &[PathBuf::from("c.wasm")]);
  }

  #[test]
  fn init_loads_active_theme_from_site_config() {
    let mut e = make();
    let ctx = EngineContext::for_tests();
    e.init(&ctx).unwrap();
    assert_eq!(e.themes().len(), 1);
    assert!(e.themes()[0].ends_with("theme_ocean_plugin.wasm"));
  }

  #[tokio::test]
  async fn aggregate_css_with_no_themes_is_empty() {
    let e = make();
    assert_eq!(e.aggregate_css().await, "");
  }

  #[test]
  fn apply_site_config_uses_theme_stack() {
    let mut e = make();
    let mut cfg = SiteConfig::default();
    cfg.themes = vec!["base.wasm".to_string(), "ocean.wasm".to_string()];
    e.apply_site_config(&cfg, std::path::Path::new("assets"));
    let names: Vec<_> =
      e.themes().iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["base.wasm", "ocean.wasm"]);
    assert!(e.themes()[0].starts_with("assets/plugins"));
  }

  #[test]
  fn apply_site_config_falls_back_to_active_theme() {
    let mut e = make();
    let mut cfg = SiteConfig::default();
    cfg.themes = Vec::new();
    cfg.active_theme = "legacy.wasm".to_string();
    e.apply_site_config(&cfg, std::path::Path::new("assets"));
    assert_eq!(e.themes().len(), 1);
    assert!(e.themes()[0].ends_with("legacy.wasm"));
  }

  #[test]
  fn override_replaces_last_layer_when_file_exists() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path();
    let base = touch_plugin(plugin_dir, "theme_base.wasm");
    let ocean = touch_plugin(plugin_dir, "theme_ocean_plugin.wasm");
    let _sunset = touch_plugin(plugin_dir, "theme_sunset_plugin.wasm");

    let stack = vec![base.clone(), ocean.clone()];
    let result = theme_with_override(&stack, plugin_dir, Some("theme_sunset_plugin.wasm"));
    assert_eq!(result.len(), 2);
    assert!(result[0].ends_with("theme_base.wasm"));
    assert!(result[1].ends_with("theme_sunset_plugin.wasm"));
  }

  #[test]
  fn override_appends_when_stack_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path();
    let _sunset = touch_plugin(plugin_dir, "theme_sunset_plugin.wasm");
    let result = theme_with_override(&[], plugin_dir, Some("theme_sunset_plugin.wasm"));
    assert_eq!(result.len(), 1);
    assert!(result[0].ends_with("theme_sunset_plugin.wasm"));
  }

  #[test]
  fn override_skipped_when_file_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path();
    let base = touch_plugin(plugin_dir, "theme_base.wasm");
    let stack = vec![base.clone()];
    let result = theme_with_override(&stack, plugin_dir, Some("missing.wasm"));
    assert_eq!(result, vec![base]);
  }

  #[test]
  fn override_none_keeps_existing_stack_filtered() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path();
    let exists = touch_plugin(plugin_dir, "a.wasm");
    let missing = plugin_dir.join("b.wasm"); // 未创建文件
    let stack = vec![exists.clone(), missing];
    let result = theme_with_override(&stack, plugin_dir, None);
    assert_eq!(result, vec![exists]);
  }

  #[test]
  fn override_empty_string_treated_as_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path();
    let base = touch_plugin(plugin_dir, "a.wasm");
    let stack = vec![base.clone()];
    let r = theme_with_override(&stack, plugin_dir, Some("   "));
    assert_eq!(r, vec![base]);
  }
}
