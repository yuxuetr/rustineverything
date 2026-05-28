//! MDX 嵌入组件注册表（Phase 2.1 引入，Phase 2.2 完整接入）。
//!
//! 设计目标：
//! - 让 widgets crate 不直接依赖任何业务模块（如 podcast）。
//! - 模块在 app 启动时通过 [`register`] 把自己的 `<X .../>` 嵌入组件
//!   注册到全局注册表；MDX 渲染管道遇到对应标签会自动查表渲染。
//! - 注册表里的组件本身必须 `Send + Sync`（不存 `Element`，仅存可被反复
//!   调用的渲染函数对象）；返回的 `Element` 在调用线程内构造、不跨线程
//!   存储，因此与 Dioxus 的单线程渲染模型兼容。
//!
//! 全局状态用 `OnceLock<RwLock<ComponentRegistry>>` 包装：
//! - 启动期写入（`register_default_components` + 各 module 的 `register_components`）。
//! - 渲染期只读（`render`），并发友好。
//! - 测试中可调用 `clear_for_tests` 重置。

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use dioxus::prelude::Element;

/// MDX 嵌入组件接口。
///
/// 实现者通过 `name()` 声明自身在 MDX 源文件中对应的 HTML 标签名
/// （例如 `<PodcastCard id="1" />` 对应 `name() == "PodcastCard"`）。
/// `render(attrs)` 在 MDX 渲染时被调用，由实现者决定如何把属性
/// 解析成具体 `Element`。
///
/// 注意：trait 上界要求 `Send + Sync`，是为了允许把组件实例存到全局
/// 注册表（跨线程访问）。具体实现里**禁止保存 `Element`**，因为
/// `Element` 含有 `Rc`，不可跨线程；本 trait 只在调用线程上构造 Element。
pub trait MdxComponent: Send + Sync {
  /// MDX 源文件中匹配的标签名（区分大小写）。
  fn name(&self) -> &'static str;

  /// 渲染单个 MDX 嵌入组件。`attrs` 是从 `<Tag a="b" c="d" />` 解析出
  /// 的属性表。实现者负责类型转换 / 默认值 / 错误降级。
  fn render(&self, attrs: &HashMap<String, String>) -> Element;
}

/// 组件注册表。按 `name` 唯一索引；重复注册会覆盖旧实现并返回 false
/// 以便调用方记录警告。
#[derive(Default)]
pub struct ComponentRegistry {
  components: HashMap<&'static str, Box<dyn MdxComponent>>,
}

impl ComponentRegistry {
  pub fn new() -> Self {
    Self::default()
  }

  /// 注册一个组件。返回 `true` 表示新注册成功，`false` 表示覆盖了
  /// 同名已注册组件（保留新实现，旧实现被 Drop）。
  pub fn register(&mut self, component: Box<dyn MdxComponent>) -> bool {
    let name = component.name();
    let existed = self.components.insert(name, component).is_some();
    !existed
  }

  /// 按 `name` 查询组件。未注册返回 `None`。
  pub fn lookup(&self, name: &str) -> Option<&dyn MdxComponent> {
    self.components.get(name).map(|b| b.as_ref())
  }

  /// 列出所有已注册组件名（按字典序排序，便于测试稳定性）。
  pub fn list(&self) -> Vec<&'static str> {
    let mut names: Vec<_> = self.components.keys().copied().collect();
    names.sort();
    names
  }

  pub fn len(&self) -> usize {
    self.components.len()
  }

  pub fn is_empty(&self) -> bool {
    self.components.is_empty()
  }

  /// 清空注册表。仅用于测试或宿主热更新。
  pub fn clear(&mut self) {
    self.components.clear();
  }
}

/// 全局单例。第一次访问时懒初始化为空 registry。
fn global() -> &'static RwLock<ComponentRegistry> {
  static REG: OnceLock<RwLock<ComponentRegistry>> = OnceLock::new();
  REG.get_or_init(|| RwLock::new(ComponentRegistry::new()))
}

/// 注册一个组件到全局注册表。模块在 app 启动时调用一次即可。
///
/// 返回 `true` 表示新注册成功，`false` 表示覆盖了同名组件
/// （应当视为告警情况，但不会 panic 以保证启动健壮）。
pub fn register(component: Box<dyn MdxComponent>) -> bool {
  match global().write() {
    Ok(mut g) => g.register(component),
    Err(_) => false,
  }
}

/// 按 `name` 渲染组件。若未注册返回 `None`（MDX 渲染管道会将其降级
/// 为占位 span，避免破坏整篇文章）。
pub fn render(name: &str, attrs: &HashMap<String, String>) -> Option<Element> {
  let g = global().read().ok()?;
  let c = g.lookup(name)?;
  Some(c.render(attrs))
}

/// 列出全局注册表中已注册的所有组件名。主要用于调试 / admin 后台。
pub fn list_registered() -> Vec<&'static str> {
  match global().read() {
    Ok(g) => g.list(),
    Err(_) => Vec::new(),
  }
}

/// 全局注册表中已注册的组件数。
pub fn registered_count() -> usize {
  match global().read() {
    Ok(g) => g.len(),
    Err(_) => 0,
  }
}

/// 仅供测试 / admin reload 使用：清空注册表。
#[doc(hidden)]
pub fn clear_for_tests() {
  if let Ok(mut g) = global().write() {
    g.clear();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 测试用空组件：不依赖 `rsx!` 宏（渲染路径需要 dioxus
  /// `html` feature）。`render` 在这里走不到：本文件只验证注册表的
  /// 存取 / 查找 / 裁减 / 排序语义；MDX 渲染路径的集成测试交由
  /// `mdx.rs` 中的 `parse_attrs` / `detect_registered_tag` 完成。
  struct StubComponent {
    name: &'static str,
  }
  impl MdxComponent for StubComponent {
    fn name(&self) -> &'static str {
      self.name
    }
    fn render(&self, _attrs: &HashMap<String, String>) -> Element {
      // 单测不会调用 render（只验证注册表存取语义）。
      // 用 unreachable! 避免构造真实 VNode。
      unreachable!("stub component render() should not be called in registry unit tests")
    }
  }

  #[test]
  fn registry_register_and_lookup() {
    let mut r = ComponentRegistry::new();
    assert!(r.is_empty());
    assert!(r.register(Box::new(StubComponent { name: "Stub" })));
    assert_eq!(r.len(), 1);
    assert!(r.lookup("Stub").is_some());
    assert!(r.lookup("Other").is_none());
  }

  #[test]
  fn registry_register_overwrite_returns_false() {
    let mut r = ComponentRegistry::new();
    assert!(r.register(Box::new(StubComponent { name: "Dup" })));
    // 第二次注册同名组件应返回 false（覆盖语义）。
    assert!(!r.register(Box::new(StubComponent { name: "Dup" })));
    assert_eq!(r.len(), 1);
  }

  #[test]
  fn registry_list_is_sorted() {
    let mut r = ComponentRegistry::new();
    r.register(Box::new(StubComponent { name: "Bilibili" }));
    r.register(Box::new(StubComponent { name: "YouTube" }));
    r.register(Box::new(StubComponent { name: "Audio" }));
    assert_eq!(r.list(), vec!["Audio", "Bilibili", "YouTube"]);
  }

  #[test]
  fn registry_clear_removes_all() {
    let mut r = ComponentRegistry::new();
    r.register(Box::new(StubComponent { name: "A" }));
    r.register(Box::new(StubComponent { name: "B" }));
    r.clear();
    assert!(r.is_empty());
  }

  #[test]
  fn lookup_returns_correct_component_by_name() {
    let mut r = ComponentRegistry::new();
    r.register(Box::new(StubComponent { name: "YouTube" }));
    r.register(Box::new(StubComponent { name: "Bilibili" }));
    let yt = r.lookup("YouTube").expect("YouTube registered");
    assert_eq!(yt.name(), "YouTube");
    let bili = r.lookup("Bilibili").expect("Bilibili registered");
    assert_eq!(bili.name(), "Bilibili");
  }
}
