//! [`ThemeEngine`] 骨架：聚合主题 WASM 插件输出的 CSS。
//!
//! Phase 1C.4 仅落地接口与最小实现；完整能力（主题栈 / 用户切换 / cookie
//! 持久）在 Phase 3.1 完成。当前实现：
//! - 通过 [`PluginEngine`] 调用每个主题插件的 `get_theme_css` 函数，
//!   按声明顺序拼接。
//! - 提供 [`ThemeEngine::register_theme`] 接收主题插件路径列表。
//! - `init` 阶段读取 `SiteConfig.active_theme`（已有字段）作为默认主题。

use std::path::PathBuf;
use std::sync::Arc;

use super::plugin::PluginEngine;
use super::{Engine, EngineContext};
use crate::error::AppResult;

/// ThemeEngine 骨架：管理主题插件路径列表，按顺序聚合 CSS。
pub struct ThemeEngine {
    plugin: Arc<PluginEngine>,
    /// 当前生效的主题插件路径列表。索引顺序即覆盖顺序（后者覆盖前者）。
    themes: Vec<PathBuf>,
}

impl ThemeEngine {
    pub fn new(plugin: Arc<PluginEngine>) -> Self {
        Self {
            plugin,
            themes: Vec::new(),
        }
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

    /// 聚合所有主题插件的 CSS。失败的插件会被跳过（不阻断其他主题）。
    pub fn aggregate_css(&self) -> String {
        let mut out = String::new();
        for path in &self.themes {
            match self.plugin.call(path, "get_theme_css", "") {
                Ok(css) => {
                    out.push_str(&css);
                    out.push('\n');
                }
                Err(e) => {
                    eprintln!("[ThemeEngine] 跳过主题 {}: {}", path.display(), e);
                }
            }
        }
        out
    }
}

impl Engine for ThemeEngine {
    fn name(&self) -> &'static str {
        "theme"
    }

    fn init(&mut self, ctx: &EngineContext) -> AppResult<()> {
        // 当前 SiteConfig 仅支持单个 active_theme（向后兼容）。
        // 后续 Phase 3.1 会改为 `themes: Vec<String>` 多层栈。
        if !ctx.site_config.active_theme.is_empty() {
            let p = ctx
                .asset_root
                .join("plugins")
                .join(&ctx.site_config.active_theme);
            self.themes.push(p);
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> ThemeEngine {
        let pm = Arc::new(crate::PluginManager::new());
        let pe = Arc::new(PluginEngine::new(pm));
        ThemeEngine::new(pe)
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

    #[test]
    fn aggregate_css_with_no_themes_is_empty() {
        let e = make();
        assert_eq!(e.aggregate_css(), "");
    }
}
