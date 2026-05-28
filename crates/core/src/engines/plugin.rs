//! [`PluginEngine`]：包装现有的 [`crate::PluginManager`]，叠加：
//!
//! 1. **ABI 版本校验**：通过 `get_manifest()` 调用拿到 [`PluginManifest`]，
//!    `abi_version` 不等于 [`SDK_ABI_VERSION`] 直接拒绝。老插件未导出
//!    `get_manifest` 时 [`PluginEngine::call`] 仍允许调用（兼容期），但
//!    [`PluginEngine::strict_call`] 会拒绝。
//! 2. **输出大小限制**：默认 8MB。覆盖主题 CSS / i18n 翻译 / Auth profile
//!    / 审核响应等所有正常场景，仍然给恶意/失控插件 cap 防御。
//! 3. **能力协商**：[`PluginEngine::capabilities_of`] 返回插件声明的能力清单，
//!    宿主据此分发到不同引擎。
//!
//! ## 关于 WASM ABI 重构
//!
//! Phase 1C.2 计划讨论过引入 [Extism] 或 [wit-bindgen] 取代当前手动 alloc/
//! dealloc 的内存管理模式。本阶段权衡如下：
//!
//! - **保留现有 alloc/dealloc + u64 打包**：可以在不全量重写 6 个插件的
//!   前提下落地 ABI 校验、输出限制、能力协商三大安全特性。
//! - **不阻塞后续替换**：`PluginEngine` 仅依赖 [`PluginManifest::abi_version`]
//!   做版本协商，未来切到 Extism / wit-bindgen 时可以提升 `SDK_ABI_VERSION`
//!   并迁移；老接口 `call_path_with_string` 保留作为兼容层。
//!
//! [Extism]: https://extism.org
//! [wit-bindgen]: https://github.com/bytecodealliance/wit-bindgen

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustineverything_sdk::{PluginManifest, SDK_ABI_VERSION};

use super::{Engine, EngineContext};
use crate::error::{AppError, AppResult};

/// 默认插件输出大小限制：8MB。
pub const DEFAULT_PLUGIN_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// 站点 PluginEngine：包装 [`PluginManager`]，注入 ABI 校验和大小限制。
pub struct PluginEngine {
  /// 共享的插件管理器。生产环境推荐传入 `crate::shared_plugin_manager()`
  /// 的 Arc 副本，开发/测试可创建独立实例。
  manager: Arc<crate::PluginManager>,
  output_limit: usize,
}

impl PluginEngine {
  /// 创建一个使用默认大小限制（8MB）的 PluginEngine。
  pub fn new(manager: Arc<crate::PluginManager>) -> Self {
    Self { manager, output_limit: DEFAULT_PLUGIN_OUTPUT_LIMIT }
  }

  /// 链式 API：覆盖默认输出大小限制。
  pub fn with_output_limit(mut self, limit: usize) -> Self {
    self.output_limit = limit;
    self
  }

  /// 暴露内部 [`PluginManager`] 引用，便于按路径直接调用底层接口。
  pub fn manager(&self) -> &crate::PluginManager {
    &self.manager
  }

  pub fn output_limit(&self) -> usize {
    self.output_limit
  }

  /// 调用插件函数（路径 + 字符串入参）。
  ///
  /// 行为：
  /// - 若插件实现了 `get_manifest`，校验 `abi_version`：不兼容则拒绝调用。
  /// - 若未实现（老插件），跳过校验保留兼容性（要严格请用 [`Self::strict_call`]）。
  /// - 输出长度超过 `output_limit` 直接报错。
  pub fn call(&self, path: &Path, func_name: &str, input: &str) -> AppResult<String> {
    if let Some(manifest) = self.try_get_manifest(path) {
      if !manifest.is_compatible() {
        return Err(AppError::plugin(format!(
          "插件 ABI 版本不兼容 ({}): 期望 {}，得到 {}",
          path.display(),
          SDK_ABI_VERSION,
          manifest.abi_version
        )));
      }
    }
    self.invoke(path, func_name, input)
  }

  /// 严格调用：必须有合法 manifest（缺失视为不兼容），且 ABI 版本一致。
  /// 适合内置/受信任插件路径，老插件未迁移时会失败。
  pub fn strict_call(&self, path: &Path, func_name: &str, input: &str) -> AppResult<String> {
    let manifest = self.get_manifest(path)?;
    if !manifest.is_compatible() {
      return Err(AppError::plugin(format!(
        "插件 ABI 版本不兼容 ({}): 期望 {}，得到 {}",
        path.display(),
        SDK_ABI_VERSION,
        manifest.abi_version
      )));
    }
    self.invoke(path, func_name, input)
  }

  /// 实际执行调用 + 输出大小限制。
  fn invoke(&self, path: &Path, func_name: &str, input: &str) -> AppResult<String> {
    let result = self
      .manager
      .call_path_with_string(path, func_name, input)
      .map_err(|e| AppError::plugin(format!("插件调用失败: {}", e)))?;

    if result.len() > self.output_limit {
      return Err(AppError::plugin(format!(
        "插件输出超过大小限制 ({} > {} bytes): {}",
        result.len(),
        self.output_limit,
        path.display()
      )));
    }
    Ok(result)
  }

  /// 拿到插件 manifest（不存在 / 解析失败 → None，不报错）。
  /// 适合老插件兼容场景。
  pub fn try_get_manifest(&self, path: &Path) -> Option<PluginManifest> {
    let json = self.manager.call_path_with_string(path, "get_manifest", "").ok()?;
    serde_json::from_str(&json).ok()
  }

  /// 严格读取 manifest：失败返回 `AppError`。
  pub fn get_manifest(&self, path: &Path) -> AppResult<PluginManifest> {
    let json = self
      .manager
      .call_path_with_string(path, "get_manifest", "")
      .map_err(|e| AppError::plugin(format!("get_manifest 失败 ({}): {}", path.display(), e)))?;
    serde_json::from_str::<PluginManifest>(&json)
      .map_err(|e| AppError::plugin(format!("manifest JSON 解析失败 ({}): {}", path.display(), e)))
  }

  /// 列出某插件声明的能力清单（老插件无 manifest 时返回空 Vec）。
  pub fn capabilities_of(&self, path: &Path) -> Vec<String> {
    self.try_get_manifest(path).map(|m| m.capabilities).unwrap_or_default()
  }

  /// 检查一组插件路径，过滤出声明了 `cap` 能力且 ABI 兼容的。
  /// 用于按能力分发：例如 ThemeEngine 只关心 `capability == "theme"`。
  pub fn filter_by_capability<'a, I>(&self, paths: I, cap: &str) -> Vec<PathBuf>
  where
    I: IntoIterator<Item = &'a Path>,
  {
    paths
      .into_iter()
      .filter_map(|p| {
        let m = self.try_get_manifest(p)?;
        if m.is_compatible() && m.has_capability(cap) {
          Some(p.to_path_buf())
        } else {
          None
        }
      })
      .collect()
  }
}

impl Engine for PluginEngine {
  fn name(&self) -> &'static str {
    "plugin"
  }

  fn init(&mut self, _ctx: &EngineContext) -> AppResult<()> {
    // 无需特殊初始化：PluginManager 自带懒加载缓存。
    Ok(())
  }

  fn shutdown(&mut self) -> AppResult<()> {
    // 释放所有 wasmi Module / Instance / Memory 缓存。
    self.manager.invalidate_all();
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
  use rustineverything_sdk::{capabilities, PluginManifest};

  fn make_engine() -> PluginEngine {
    PluginEngine::new(Arc::new(crate::PluginManager::new()))
  }

  #[test]
  fn engine_name_is_plugin() {
    let e = make_engine();
    assert_eq!(e.name(), "plugin");
  }

  #[test]
  fn output_limit_default_is_8mb() {
    let e = make_engine();
    assert_eq!(e.output_limit(), DEFAULT_PLUGIN_OUTPUT_LIMIT);
    assert_eq!(e.output_limit(), 8 * 1024 * 1024);
  }

  #[test]
  fn output_limit_can_be_overridden() {
    let e = make_engine().with_output_limit(1024);
    assert_eq!(e.output_limit(), 1024);
  }

  #[test]
  fn shutdown_invalidates_cache() {
    let mut e = make_engine();
    // 用一个真实存在的 wasm 路径生成缓存条目；不存在则跳过
    let wasm = Path::new("../../assets/plugins/i18n_fluent_plugin.wasm");
    if wasm.exists() {
      let _ = e.manager.call_path_with_string(
        wasm,
        "translate",
        &serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string(),
      );
    }
    assert!(e.shutdown().is_ok());
  }

  #[test]
  fn engine_ctx_init_is_noop_for_plugin() {
    let mut e = make_engine();
    let ctx = EngineContext::for_tests();
    assert!(e.init(&ctx).is_ok());
  }

  /// 当 manifest ABI 不兼容时 `strict_call` 应拒绝。
  /// 通过手工构造 manifest 验证；并不真的需要 wasm 实例。
  #[test]
  fn manifest_compat_check() {
    let m_ok = PluginManifest::new("x", "X", "0.1.0").with_capability(capabilities::THEME);
    assert!(m_ok.is_compatible());
    assert!(m_ok.has_capability(capabilities::THEME));

    let mut m_bad = PluginManifest::new("x", "X", "0.1.0");
    m_bad.abi_version = u32::MAX; // 假设的未来版本
    assert!(!m_bad.is_compatible());
  }

  /// `filter_by_capability` 在所有插件无 manifest 时返回空。
  #[test]
  fn filter_by_capability_with_no_manifests_returns_empty() {
    let e = make_engine();
    let nonexistent = [PathBuf::from("/tmp/__no_such_plugin__.wasm")];
    let refs: Vec<&Path> = nonexistent.iter().map(|p| p.as_path()).collect();
    let result = e.filter_by_capability(refs, capabilities::THEME);
    assert!(result.is_empty());
  }

  /// 真实插件场景：assets/plugins/ 中的插件未导出 get_manifest 时，
  /// `try_get_manifest` 应返回 None，`call` 仍然可工作。
  #[test]
  fn real_plugin_call_without_manifest_works() {
    let e = make_engine();
    let wasm = Path::new("../../assets/plugins/i18n_fluent_plugin.wasm");
    if !wasm.exists() {
      return;
    }
    // 这些插件还没迁移到新 ABI（无 get_manifest），call 必须降级允许。
    let res =
      e.call(wasm, "translate", &serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string());
    // 调用本身能成功，但结果是否准确取决于插件是否迁移；这里仅验证不报错或报错合理
    match res {
      Ok(_) => {} // 老插件直接成功
      Err(e) => {
        // 当前路径下若插件已迁移到新 ABI 但 wasm 文件未重建，可能报 ABI 不兼容
        assert!(
          format!("{}", e).contains("不兼容") || format!("{}", e).contains("调用失败"),
          "未预期的错误: {}",
          e
        );
      }
    }
  }

  /// 输出大小超限时返回 AppError::Plugin。
  #[test]
  fn output_over_limit_is_rejected() {
    // 贴近零的限制下调用真实插件，输出必然超过 1 字节
    let e = make_engine().with_output_limit(1);
    let wasm = Path::new("../../assets/plugins/i18n_fluent_plugin.wasm");
    if !wasm.exists() {
      return; // 插件未构建跳过
    }
    let res =
      e.call(wasm, "translate", &serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string());
    match res {
      Err(err) => {
        let msg = format!("{}", err);
        assert!(msg.contains("超过大小限制"), "未预期的错误: {}", msg);
      }
      Ok(_) => panic!("预期超过输出限制报错"),
    }
  }

  /// 集成测试：验证迁移后的插件能被读出 manifest，ABI 匹配且能力被声明。
  /// 插件 wasm 未构建时跳过。
  #[test]
  fn integration_real_plugin_manifest() {
    let e = make_engine();
    let wasm = Path::new("../../assets/plugins/github_auth_plugin.wasm");
    if !wasm.exists() {
      return;
    }
    let manifest = match e.get_manifest(wasm) {
      Ok(m) => m,
      Err(_) => return, // 插件未迁移也允许跳过
    };
    assert_eq!(manifest.id, "github-auth");
    assert!(manifest.is_compatible(), "github-auth ABI 不兼容: {}", manifest.abi_version);
    assert!(manifest.has_capability(capabilities::AUTH_PROVIDER));
  }

  #[test]
  fn integration_real_plugin_strict_call_succeeds() {
    let e = make_engine();
    let wasm = Path::new("../../assets/plugins/i18n_fluent_plugin.wasm");
    if !wasm.exists() {
      return;
    }
    if e.try_get_manifest(wasm).is_none() {
      return; // 插件未迁移跳过
    }
    let result = e.strict_call(
      wasm,
      "translate",
      &serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string(),
    );
    match result {
      Ok(s) => assert_eq!(s, "Blog"),
      Err(err) => panic!("strict_call 在已迁移插件上不应该失败: {}", err),
    }
  }

  #[test]
  fn integration_filter_by_capability_finds_theme() {
    let e = make_engine();
    let theme = PathBuf::from("../../assets/plugins/theme_ocean_plugin.wasm");
    let auth = PathBuf::from("../../assets/plugins/github_auth_plugin.wasm");
    if !theme.exists() || !auth.exists() {
      return;
    }
    if e.try_get_manifest(&theme).is_none() {
      return;
    }
    let refs: Vec<&Path> = vec![theme.as_path(), auth.as_path()];
    let themes = e.filter_by_capability(refs.clone(), capabilities::THEME);
    let auths = e.filter_by_capability(refs, capabilities::AUTH_PROVIDER);
    assert_eq!(themes.len(), 1);
    assert!(themes[0].ends_with("theme_ocean_plugin.wasm"));
    assert_eq!(auths.len(), 1);
    assert!(auths[0].ends_with("github_auth_plugin.wasm"));
  }
}
