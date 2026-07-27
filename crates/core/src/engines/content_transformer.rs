//! [`ContentTransformerEngine`]：调度 content-transformer 插件链（Phase 9.3）。
//!
//! ## 角色
//! - **Pre stage**：宿主 server fn 加载 markdown 文件后、传给 widget Markdown
//!   组件前，按 site.json `content_transformers` 列表顺序逐个调用插件。
//!   前一个插件输出的 content 作为下一个的输入（chain 语义）。
//! - **Fail-open**：单个插件 trap / timeout / 返回非法 JSON 时跳过该插件，
//!   链路继续，最终交给 widget 的内容至少与原始一致。一个坏插件不能弄死一篇文章。
//! - **零开销短路**：空 transformer 列表（默认）→ `apply` 直接返回原 content，
//!   连一次 wasm 调用都不发；env `CONTENT_TRANSFORMER_DISABLE=true` 强制关闭。
//!
//! ## ABI
//! 见 [`sdk::content_transformer::FN_TRANSFORM_MARKDOWN`] 与
//! [`sdk::TransformRequest`] / [`sdk::TransformResponse`]：插件接收
//! `{ content, kind, stage }` JSON、返回 `{ content, changed }` JSON。
//! 宿主仅以 `content` 字段为准；`changed` 用作 trace 提示。
//!
//! Phase 9.3 当前只接 `stage = "pre"`。`"post"`（HTML 后处理）需要 SSR-only 路径，
//! 留待 Dioxus 端有更稳定的 SSR pipeline 后再加。
//!
//! ## 与 PluginManager 的关系
//! 引擎自身**不**持有 PluginManager；它只保存 transformer 路径列表，调用时由
//! caller 把 `&PluginManager` 传进来。这与 [`super::theme`] 风格略不同（那里
//! 通过 PluginEngine 包了一层 Arc），目的是让全局 helper
//! [`default_content_transformer_engine`] 能直接复用 [`crate::shared_plugin_manager`]
//! 的静态实例，避免重复构造模块缓存。

use std::path::PathBuf;

use crate::settings::SiteConfig;
use crate::PluginManager;

/// 控制是否启用 content-transformer 调度的环境变量。
/// 设置为 `"true"`（或 `"1"`）→ `apply` 始终直通；用于性能 benchmark / 紧急关闭。
pub const ENV_DISABLE: &str = "CONTENT_TRANSFORMER_DISABLE";

/// content-transformer 调度引擎。
///
/// `transformers` 是按声明顺序排列的插件路径，运行时按该顺序 chain 调用。
#[derive(Debug, Default, Clone)]
pub struct ContentTransformerEngine {
  transformers: Vec<PathBuf>,
}

impl ContentTransformerEngine {
  pub fn new() -> Self {
    Self::default()
  }

  /// 注册一个 transformer 插件路径。可重复调用。
  pub fn register(&mut self, path: PathBuf) {
    self.transformers.push(path);
  }

  /// 用 site.json 中的 `content_transformers` 列表装填（清空现有栈）。
  /// `asset_root` 指向资产根目录（`assets/`），拼接成 `asset_root/plugins/<file>`。
  pub fn apply_site_config(&mut self, site: &SiteConfig, asset_root: &std::path::Path) {
    let plugin_dir = asset_root.join("plugins");
    self.transformers = site
      .content_transformers
      .iter()
      .filter(|name| !name.is_empty())
      .map(|name| plugin_dir.join(name))
      .collect();
  }

  /// 当前注册的 transformer 路径列表（只读）。
  pub fn transformers(&self) -> &[PathBuf] {
    &self.transformers
  }

  /// 是否启用：env `CONTENT_TRANSFORMER_DISABLE=true|1` 时强制关闭，否则
  /// 只要列表非空就启用。空列表 = 实质关闭，避免任何 wasm 调用。
  pub fn is_enabled(&self) -> bool {
    if self.transformers.is_empty() {
      return false;
    }
    let disabled = std::env::var(ENV_DISABLE)
      .ok()
      .map(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "1"))
      .unwrap_or(false);
    !disabled
  }

  /// 串行 chain 跑所有 transformer 插件。
  ///
  /// Phase 9.3 当前 stage 仅 `"pre"` 落地；插件如果不识别也应直接 passthrough。
  ///
  /// Fail-open 语义（单条都不会中断链路）：
  /// - 插件调用本身报错（trap / timeout / output 超限） → 跳过，content 不变
  /// - 插件返回的 JSON 解析失败 → 跳过
  /// - 解析得到 `content` 字段为空字符串 → 跳过（防止误删整篇内容）
  #[cfg(feature = "server")]
  pub async fn apply(
    &self,
    manager: &PluginManager,
    content: &str,
    kind: &str,
    stage: &str,
  ) -> String {
    if !self.is_enabled() {
      return content.to_string();
    }

    let mut current = content.to_string();
    for path in &self.transformers {
      let req = sdk::TransformRequest {
        content: current.clone(),
        kind: kind.to_string(),
        stage: stage.to_string(),
      };
      let payload = match serde_json::to_string(&req) {
        Ok(s) => s,
        Err(e) => {
          tracing::warn!(error = %e, plugin = %path.display(), "content-transformer: skip — request serialize failed");
          continue;
        }
      };
      let out = match manager
        .call_path_with_string(path, sdk::content_transformer::FN_TRANSFORM_MARKDOWN, &payload)
        .await
      {
        Ok(s) => s,
        Err(e) => {
          tracing::warn!(error = %e, plugin = %path.display(), "content-transformer: skip — call failed");
          continue;
        }
      };
      let resp: sdk::TransformResponse = match serde_json::from_str(&out) {
        Ok(r) => r,
        Err(e) => {
          tracing::warn!(error = %e, plugin = %path.display(), "content-transformer: skip — invalid response JSON");
          continue;
        }
      };
      if resp.content.is_empty() {
        tracing::warn!(plugin = %path.display(), "content-transformer: skip — empty content (fail-open)");
        continue;
      }
      current = resp.content;
    }
    current
  }
}

// ─── 全局共享缓存（与 ModuleEngine 同款 OnceLock 模式） ─────────────────

#[cfg(feature = "server")]
fn slot() -> &'static std::sync::RwLock<Option<std::sync::Arc<ContentTransformerEngine>>> {
  use std::sync::{Arc, OnceLock, RwLock};
  static CACHE: OnceLock<RwLock<Option<Arc<ContentTransformerEngine>>>> = OnceLock::new();
  CACHE.get_or_init(|| RwLock::new(None))
}

/// 全局共享的 ContentTransformerEngine。首次调用时读 site.json 装载 transformer 路径
/// 列表；后续命中直接 clone Arc。
///
/// admin 修改 site.json 后必须调 [`invalidate_default_content_transformer_engine`]
/// 强制重读，否则进程内一直沿用首次装填的列表。
#[cfg(feature = "server")]
pub fn default_content_transformer_engine() -> std::sync::Arc<ContentTransformerEngine> {
  use std::sync::Arc;
  let slot = slot();

  if let Ok(guard) = slot.read() {
    if let Some(arc) = guard.as_ref() {
      return arc.clone();
    }
  }

  let mut e = ContentTransformerEngine::new();
  let site_path = crate::utils::get_asset_root().join("site.json");
  // S10：走 load_cached，与其它 site.json 读取点共享 mtime 缓存。
  if let Ok(cfg) = SiteConfig::load_cached(site_path.to_str().unwrap_or_default()) {
    e.apply_site_config(&cfg, &crate::utils::get_asset_root());
  }
  let arc = Arc::new(e);
  if let Ok(mut guard) = slot.write() {
    *guard = Some(arc.clone());
  }
  arc
}

/// admin server fn 写完 site.json 后调用，强制下一次 [`default_content_transformer_engine`]
/// 重建。
#[cfg(feature = "server")]
pub fn invalidate_default_content_transformer_engine() {
  if let Ok(mut guard) = slot().write() {
    *guard = None;
  }
}

/// 顶层便利函数：用 default engine + shared PluginManager 跑一次 pre transform。
///
/// Server fn 加载完 markdown 后调一次即可。链路空 / 关闭时零开销直通。
#[cfg(feature = "server")]
pub async fn apply_default_pre(content: &str, kind: &str) -> String {
  let engine = default_content_transformer_engine();
  if !engine.is_enabled() {
    return content.to_string();
  }
  engine.apply(crate::shared_plugin_manager(), content, kind, "pre").await
}

#[cfg(all(test, feature = "server"))]
#[allow(clippy::field_reassign_with_default)] // 测试 setup：Default + 逐字段赋值更易读
mod tests {
  use super::*;

  #[test]
  fn register_and_list() {
    let mut e = ContentTransformerEngine::new();
    e.register(PathBuf::from("a.wasm"));
    e.register(PathBuf::from("b.wasm"));
    assert_eq!(e.transformers().len(), 2);
  }

  #[test]
  fn apply_site_config_filters_empty_names() {
    let mut e = ContentTransformerEngine::new();
    let mut cfg = SiteConfig::default();
    cfg.content_transformers =
      vec!["content_toc_plugin.wasm".to_string(), String::new(), "other.wasm".to_string()];
    e.apply_site_config(&cfg, std::path::Path::new("assets"));
    let names: Vec<_> = e
      .transformers()
      .iter()
      .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
      .collect();
    assert_eq!(names, vec!["content_toc_plugin.wasm", "other.wasm"]);
    assert!(e.transformers()[0].starts_with("assets/plugins"));
  }

  #[test]
  fn empty_transformer_list_is_disabled() {
    let e = ContentTransformerEngine::new();
    assert!(!e.is_enabled());
  }

  #[tokio::test]
  async fn apply_with_empty_list_returns_original_content_zero_cost() {
    let e = ContentTransformerEngine::new();
    let manager = PluginManager::new();
    let original = "# Hello\n\nworld";
    let result = e.apply(&manager, original, "blog", "pre").await;
    assert_eq!(result, original);
  }

  #[tokio::test]
  async fn apply_skips_nonexistent_plugin_fail_open() {
    let mut e = ContentTransformerEngine::new();
    e.register(PathBuf::from("/tmp/__no_such_content_transformer__.wasm"));
    let manager = PluginManager::new();
    let original = "# Hello";
    let result = e.apply(&manager, original, "blog", "pre").await;
    // fail-open：插件不存在 → 跳过 → 原文返回
    assert_eq!(result, original);
  }
}
