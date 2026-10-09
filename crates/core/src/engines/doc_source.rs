//! 外部索引文档来源注册表（IoC，Phase 重构 A3）。
//!
//! ## 角色
//! 让 [`module-search`](../../../modules/search) 不必在编译期依赖具体内容模块
//! （如 `module-cases`）即可把它们的数据纳入全文索引。内容模块的数据由组合根
//! `app` 在启动期通过 [`register_doc_source`] 注入；`module-search` 在收集动态
//! 索引文档时调用 [`collect_registered_docs`] 读取。
//!
//! 设计同 [`crate::PluginManager`] / `widgets::registry::ComponentRegistry` 的全局
//! `OnceLock<RwLock<…>>` 单例模式：启动期写入，运行期只读，并发友好。
//!
//! ## 为什么放在 core
//! `core` 是全工作区的基础设施层（被 search 与 app 同时依赖），把中立的
//! [`ExternalIndexedDoc`] 与注册表放这里，既不引入循环依赖，也让 search 只依赖
//! `core` 抽象而非兄弟内容模块。

use std::sync::{OnceLock, RwLock};

/// 中立的索引文档。字段与 `module_search::indexer::IndexedDocument` 一一对齐，
/// 但**不依赖** search crate，避免反向依赖。
#[derive(Debug, Clone)]
pub struct ExternalIndexedDoc {
  /// 业务类型（如 `"case"`）。search 据此映射到模块开关做过滤。
  pub kind: String,
  /// 业务侧唯一 id（如 case slug）。
  pub ref_id: String,
  pub title: String,
  /// 已组装好的可索引正文（调用方负责拼接 / 截断）。
  pub body: String,
  pub url: String,
  pub created_at: String,
}

type SourceFn = Box<dyn Fn() -> Vec<ExternalIndexedDoc> + Send + Sync>;

fn registry() -> &'static RwLock<Vec<SourceFn>> {
  static REG: OnceLock<RwLock<Vec<SourceFn>>> = OnceLock::new();
  REG.get_or_init(|| RwLock::new(Vec::new()))
}

/// 注册一个外部文档来源。`app` 在启动期对每个内容模块调用一次。
///
/// 锁中毒（极少见）时静默跳过本次注册，不 panic，以保证启动健壮。
pub fn register_doc_source<F>(f: F)
where
  F: Fn() -> Vec<ExternalIndexedDoc> + Send + Sync + 'static,
{
  if let Ok(mut guard) = registry().write() {
    guard.push(Box::new(f));
  }
}

/// 汇总所有已注册来源的文档。`module-search` 在动态索引收集阶段调用。
///
/// 锁中毒时返回空列表（索引降级，不阻塞），与既有 search 的 fail-soft 风格一致。
pub fn collect_registered_docs() -> Vec<ExternalIndexedDoc> {
  match registry().read() {
    Ok(guard) => guard.iter().flat_map(|f| f()).collect(),
    Err(_) => Vec::new(),
  }
}

/// 已注册来源的数量（调试 / 测试用）。
pub fn registered_source_count() -> usize {
  registry().read().map(|g| g.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn register_and_collect_roundtrip() {
    // 注意：全局注册表跨测试共享，这里只验证「注册后能取回自己注册的文档」。
    let before = collect_registered_docs().len();
    register_doc_source(|| {
      vec![ExternalIndexedDoc {
        kind: "test-kind".to_string(),
        ref_id: "t1".to_string(),
        title: "T".to_string(),
        body: "body".to_string(),
        url: "/t/1".to_string(),
        created_at: "2026-01-01".to_string(),
      }]
    });
    let after = collect_registered_docs();
    assert!(after.len() > before);
    assert!(after.iter().any(|d| d.ref_id == "t1" && d.kind == "test-kind"));
  }
}
