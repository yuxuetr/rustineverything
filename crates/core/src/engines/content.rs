//! [`ContentEngine`] 骨架（Phase 1C.4）。
//!
//! 完整的 MDX 组件注册系统在 Phase 2 落地（迁移 widgets crate）；本阶段仅
//! 落定义骨架与最小注册表，方便后续填充。
//!
//! ## 接口
//! - [`MdxComponent`]：单个 MDX 嵌入组件的定义。`name()` 唯一。`render(attrs)`
//!   返回原始 HTML 字符串（widgets crate 后续可以再封装为 Dioxus `Element`）。
//! - [`ComponentRegistry`]：按名注册 / 查询。
//! - [`ContentEngine`]：包装 ComponentRegistry，实现 [`Engine`] trait。

use std::collections::HashMap;

use serde_json::Value;

use super::{Engine, EngineContext};
use crate::error::{AppError, AppResult};

/// MDX 嵌入组件的定义。
///
/// 最小骨架：`name()` 唯一标识；`render(attrs)` 接收 JSON 形式的属性，
/// 返回 HTML 字符串。后续 Phase 2 widgets crate 会引入更高阶的 Dioxus
/// Element 渲染。
pub trait MdxComponent: Send + Sync {
  fn name(&self) -> &'static str;
  fn render(&self, attrs: &Value) -> String;
}

/// 组件注册表：按名查找。
#[derive(Default)]
pub struct ComponentRegistry {
  components: HashMap<&'static str, Box<dyn MdxComponent>>,
}

impl ComponentRegistry {
  pub fn new() -> Self {
    Self::default()
  }

  /// 注册一个组件。重复名返回 `AppError::Other`。
  pub fn register<C: MdxComponent + 'static>(&mut self, component: C) -> AppResult<()> {
    let name = component.name();
    if self.components.contains_key(name) {
      return Err(AppError::other(format!("MDX 组件已注册: {}", name)));
    }
    self.components.insert(name, Box::new(component));
    Ok(())
  }

  /// 查找一个组件。未注册返回 `None`。
  pub fn lookup(&self, name: &str) -> Option<&dyn MdxComponent> {
    self.components.get(name).map(|b| b.as_ref())
  }

  /// 列出所有已注册组件名。
  pub fn list(&self) -> Vec<&'static str> {
    let mut names: Vec<_> = self.components.keys().copied().collect();
    names.sort();
    names
  }

  /// 渲染：未知组件降级为占位 HTML 注释，避免页面渲染失败。
  pub fn render(&self, name: &str, attrs: &Value) -> String {
    match self.lookup(name) {
      Some(c) => c.render(attrs),
      None => format!("<!-- unknown MDX component: {} -->", name),
    }
  }

  pub fn len(&self) -> usize {
    self.components.len()
  }

  pub fn is_empty(&self) -> bool {
    self.components.is_empty()
  }
}

/// ContentEngine：包装 ComponentRegistry。
pub struct ContentEngine {
  registry: ComponentRegistry,
}

impl Default for ContentEngine {
  fn default() -> Self {
    Self::new()
  }
}

impl ContentEngine {
  pub fn new() -> Self {
    Self { registry: ComponentRegistry::new() }
  }

  pub fn registry(&self) -> &ComponentRegistry {
    &self.registry
  }

  pub fn registry_mut(&mut self) -> &mut ComponentRegistry {
    &mut self.registry
  }
}

impl Engine for ContentEngine {
  fn name(&self) -> &'static str {
    "content"
  }

  fn init(&mut self, _ctx: &EngineContext) -> AppResult<()> {
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

  struct YouTube;
  impl MdxComponent for YouTube {
    fn name(&self) -> &'static str {
      "YouTube"
    }
    fn render(&self, attrs: &Value) -> String {
      let id = attrs.get("id").and_then(|v| v.as_str()).unwrap_or("");
      format!("<iframe src=\"https://www.youtube.com/embed/{}\"></iframe>", id)
    }
  }

  struct Bilibili;
  impl MdxComponent for Bilibili {
    fn name(&self) -> &'static str {
      "Bilibili"
    }
    fn render(&self, _attrs: &Value) -> String {
      "<bilibili-stub/>".to_string()
    }
  }

  #[test]
  fn engine_name_is_content() {
    let e = ContentEngine::new();
    assert_eq!(<ContentEngine as Engine>::name(&e), "content");
  }

  #[test]
  fn register_and_lookup() {
    let mut r = ComponentRegistry::new();
    r.register(YouTube).unwrap();
    assert_eq!(r.len(), 1);
    let c = r.lookup("YouTube").expect("found");
    let html = c.render(&serde_json::json!({"id": "abc123"}));
    assert!(html.contains("youtube.com/embed/abc123"));
  }

  #[test]
  fn duplicate_component_name_fails() {
    let mut r = ComponentRegistry::new();
    r.register(YouTube).unwrap();
    let err = r.register(YouTube).unwrap_err();
    assert!(matches!(err, AppError::Other(_)));
  }

  #[test]
  fn unknown_component_renders_placeholder() {
    let r = ComponentRegistry::new();
    let html = r.render("Nonexistent", &Value::Null);
    assert!(html.contains("unknown MDX component"));
    assert!(html.contains("Nonexistent"));
  }

  #[test]
  fn list_is_sorted() {
    let mut r = ComponentRegistry::new();
    r.register(YouTube).unwrap();
    r.register(Bilibili).unwrap();
    assert_eq!(r.list(), vec!["Bilibili", "YouTube"]);
  }
}
