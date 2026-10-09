//! [`LayoutEngine`] 骨架 + [`LayoutPack`] trait（Phase 1C.4）。
//!
//! 完整实现（Classic / Minimal 两种 Rust crate-form 布局包 + 切换 UI）
//! 在 Phase 3.3 落地。当前仅落地接口与最小注册表，方便后续渐进式迁移。
//!
//! ## 设计要点
//! - **LayoutPack** 描述一种站点布局：name() 唯一标识，slot 渲染等具体能力
//!   未来由 trait 方法补充（render_navbar / render_footer / ...）。当前阶段
//!   只锁定 name + label，避免在尚未确定 widgets crate 之前过度设计。
//! - **LayoutEngine** 维护已注册布局并记录默认布局名（来自 site.json，未来字段）。

use crate::error::{AppError, AppResult};
use crate::settings::SiteConfig;

/// 单个布局包的描述。具体渲染方法在后续 Phase 3.3 / 2.1 widgets crate
/// 落地后补充（如 `fn navbar() -> Element`）。
pub trait LayoutPack: Send + Sync {
  /// 唯一名（`"classic"` / `"minimal"` / 未来 `"magazine"` / `"docs"`）。
  fn name(&self) -> &'static str;
  /// 用户/管理员可见的展示名（如 "Classic" / "Minimal"）。
  fn label(&self) -> &'static str;
}

/// LayoutEngine：注册中心 + 当前默认布局。
pub struct LayoutEngine {
  packs: Vec<Box<dyn LayoutPack>>,
  /// 当前生效的布局名。`init` 时从 `SiteConfig` 读取（未来字段）。
  active: Option<String>,
}

impl Default for LayoutEngine {
  fn default() -> Self {
    Self::new()
  }
}

impl LayoutEngine {
  pub fn new() -> Self {
    Self { packs: Vec::new(), active: None }
  }

  /// 注册一个 layout pack。重复名返回错误。
  pub fn register<L: LayoutPack + 'static>(&mut self, pack: L) -> AppResult<()> {
    if self.packs.iter().any(|p| p.name() == pack.name()) {
      return Err(AppError::other(format!("Layout 已注册: {}", pack.name())));
    }
    self.packs.push(Box::new(pack));
    Ok(())
  }

  /// 设置当前生效的布局。
  pub fn set_active(&mut self, name: impl Into<String>) {
    self.active = Some(name.into());
  }

  pub fn active(&self) -> Option<&str> {
    self.active.as_deref()
  }

  /// 已注册的布局名清单。
  pub fn names(&self) -> Vec<&'static str> {
    self.packs.iter().map(|p| p.name()).collect()
  }

  pub fn len(&self) -> usize {
    self.packs.len()
  }

  pub fn is_empty(&self) -> bool {
    self.packs.is_empty()
  }
}

impl LayoutEngine {
  /// 原 Engine::init 行为：从 SiteConfig 读取 active_layout（默认 classic）。
  /// 未注册的布局不会被拒绝——上层 Navbar 在 server fn 层面以「未知 → classic」回退。
  pub fn apply_site_config(&mut self, site: &SiteConfig) {
    self.active = Some(site.active_layout_or_default().to_string());
  }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // 测试 setup：Default + 逐字段赋值更易读
mod tests {
  use super::*;

  struct ClassicLayout;
  impl LayoutPack for ClassicLayout {
    fn name(&self) -> &'static str {
      "classic"
    }
    fn label(&self) -> &'static str {
      "Classic"
    }
  }

  struct MinimalLayout;
  impl LayoutPack for MinimalLayout {
    fn name(&self) -> &'static str {
      "minimal"
    }
    fn label(&self) -> &'static str {
      "Minimal"
    }
  }

  // Phase 8.7：删除了 Engine trait，原 engine_name_is_layout 不再有意义。

  #[test]
  fn register_two_distinct_layouts() {
    let mut e = LayoutEngine::new();
    e.register(ClassicLayout).unwrap();
    e.register(MinimalLayout).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e.names(), vec!["classic", "minimal"]);
  }

  #[test]
  fn duplicate_layout_name_fails() {
    let mut e = LayoutEngine::new();
    e.register(ClassicLayout).unwrap();
    let err = e.register(ClassicLayout).unwrap_err();
    assert!(matches!(err, AppError::Other(_)));
  }

  #[test]
  fn active_layout_is_recorded() {
    let mut e = LayoutEngine::new();
    e.register(ClassicLayout).unwrap();
    e.set_active("classic");
    assert_eq!(e.active(), Some("classic"));
  }

  #[test]
  fn apply_site_config_reads_active_layout() {
    // 默认 site_config.active_layout = "classic"
    let mut e = LayoutEngine::new();
    e.apply_site_config(&SiteConfig::default());
    assert_eq!(e.active(), Some("classic"));
  }

  #[test]
  fn apply_site_config_picks_minimal_when_site_says_minimal() {
    let mut cfg = SiteConfig::default();
    cfg.active_layout = "minimal".to_string();
    let mut e = LayoutEngine::new();
    e.apply_site_config(&cfg);
    assert_eq!(e.active(), Some("minimal"));
  }
}
