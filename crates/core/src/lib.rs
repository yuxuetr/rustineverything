#[cfg(feature = "server")]
pub mod auth;
#[cfg(feature = "server")]
pub mod db;
pub mod engines;
#[cfg(feature = "server")]
pub mod entities;
pub mod error;
pub mod i18n;
pub mod plugin_security;
pub mod session;
pub mod settings;
pub mod utils;

// Re-export SDK types needed by the app crate
pub use sdk::AuthProviderDisplay;
pub use sdk::{capabilities, PluginManifest, SDK_ABI_VERSION};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;
#[cfg(feature = "server")]
use wasmi::Store;
use wasmi::{Config, Engine, Linker, Module, StoreLimits, StoreLimitsBuilder};

/// 全局共享的 [`PluginManager`] 实例，让插件 `Module` 缓存跨 server fn 调用复用。
static SHARED_PLUGIN_MANAGER: LazyLock<PluginManager> = LazyLock::new(PluginManager::new);

/// 返回全局共享的 `PluginManager`。i18n / 主题 / Auth 等高频调用点推荐调用。
pub fn shared_plugin_manager() -> &'static PluginManager {
  &SHARED_PLUGIN_MANAGER
}

/// 默认 fuel 额度：100M ≈ 1 秒内核动作。可通过 `WASM_FUEL_LIMIT` env 覆盖。
const DEFAULT_WASM_FUEL: u64 = 100_000_000;
/// 默认线性内存上限：128 页 = 8 MiB（每页 64 KiB）。可通过 `WASM_MEMORY_PAGES` env 覆盖。
const DEFAULT_WASM_MEMORY_PAGES: u32 = 128;
/// 默认 host 端输出缓冲上限：8 MiB。任何插件返回长度被 clamp 到该值之内，避免恶意 len 触发巨缓冲分配。
const DEFAULT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
/// 默认单次 wasm 调用 wall-clock 超时（秒）。fuel 是主防线，timeout 是次防线兜底。
const DEFAULT_INVOKE_TIMEOUT_SECS: u64 = 5;
/// wasm 线性内存 page size。
const WASM_PAGE_SIZE: usize = 65536;

/// 插件缓存条目：记录预编译后的 wasmi `Module` + 文件 mtime。
struct CachedModule {
  module: Module,
  mtime: SystemTime,
}

/// 沙箱配置：fuel + 内存页 + 输出缓冲 + 超时。
#[derive(Clone, Copy, Debug)]
struct SandboxConfig {
  fuel: u64,
  memory_pages: u32,
  output_limit: usize,
  timeout_secs: u64,
}

impl SandboxConfig {
  fn from_env() -> Self {
    Self {
      fuel: read_env_u64("WASM_FUEL_LIMIT", DEFAULT_WASM_FUEL),
      memory_pages: read_env_u32("WASM_MEMORY_PAGES", DEFAULT_WASM_MEMORY_PAGES),
      output_limit: read_env_usize("WASM_OUTPUT_LIMIT", DEFAULT_OUTPUT_LIMIT),
      timeout_secs: read_env_u64("WASM_INVOKE_TIMEOUT_SECS", DEFAULT_INVOKE_TIMEOUT_SECS),
    }
  }

  fn memory_bytes(&self) -> usize {
    (self.memory_pages as usize).saturating_mul(WASM_PAGE_SIZE)
  }

  fn store_limits(&self) -> StoreLimits {
    StoreLimitsBuilder::new().memory_size(self.memory_bytes()).build()
  }
}

fn read_env_u64(key: &str, default: u64) -> u64 {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn read_env_u32(key: &str, default: u32) -> u32 {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn read_env_usize(key: &str, default: usize) -> usize {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

pub struct PluginManager {
  engine: Engine,
  linker: Linker<StoreLimits>,
  /// 提供按路径复用 `Module` 的能力。在 i18n / 主题等高频调用场景下
  /// 可以避免重复读文件 + 重复调用 `Module::new`。
  cache: Mutex<HashMap<PathBuf, CachedModule>>,
  /// Phase 8.5：theme CSS 输出按 (path, mtime) 缓存。
  /// `get_theme_css("")` 的结果只取决于插件二进制本身，
  /// 同 mtime 下永远返回同一 CSS，可以跨请求复用。
  /// 命中时省掉一次 wasmi instantiate + 调用 + memory I/O。
  theme_css_cache: Mutex<HashMap<PathBuf, (SystemTime, Arc<String>)>>,
  /// Phase 9.2：插件 SHA256 lock 表。
  /// key = 插件**文件名**（不是完整路径），value = 期望 SHA256 hex 小写。
  /// 启动时由 app 从 `site.json` 灌入；加载时自动比对，不匹配则拒绝。
  /// 空表 = warn-only 模式（fork 用户首次部署无 lock，先放行）。
  plugins_lock: Mutex<HashMap<String, String>>,
  sandbox: SandboxConfig,
}

impl Default for PluginManager {
  fn default() -> Self {
    Self::new()
  }
}

impl PluginManager {
  pub fn new() -> Self {
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let linker = Linker::<StoreLimits>::new(&engine);
    Self {
      engine,
      linker,
      cache: Mutex::new(HashMap::new()),
      theme_css_cache: Mutex::new(HashMap::new()),
      plugins_lock: Mutex::new(HashMap::new()),
      sandbox: SandboxConfig::from_env(),
    }
  }

  /// Phase 9.2：灌入插件 SHA256 lock 表。通常由 app 启动时从 `site.json`
  /// 读取 [`crate::settings::SiteConfig::plugins_lock`] 调一次。
  ///
  /// 多次调用 = 完全替换（不合并）；空 map = 关闭 lock 校验（warn-only）。
  ///
  /// 仅在 `server` feature 下有效（sha2 依赖在 server 才启用）。
  #[cfg(feature = "server")]
  pub fn set_plugins_lock(&self, lock: HashMap<String, String>) {
    if let Ok(mut guard) = self.plugins_lock.lock() {
      *guard = lock;
    }
  }

  /// Phase 9.2：查询路径对应文件名的 expected sha256（小写 hex）。
  #[cfg(feature = "server")]
  fn expected_sha256_for(&self, path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    self
      .plugins_lock
      .lock()
      .ok()
      .and_then(|guard| guard.get(file_name).cloned())
      .filter(|s| !s.is_empty())
  }

  /// 当前 host 端输出缓冲上限（bytes）。用于上层 PluginEngine 决定结果 cap。
  pub fn output_limit(&self) -> usize {
    self.sandbox.output_limit
  }

  /// 当前 fuel 配额（单次调用），用于诊断 / 测试。
  pub fn fuel_limit(&self) -> u64 {
    self.sandbox.fuel
  }

  /// 当前每实例线性内存 page 上限。
  pub fn memory_pages_limit(&self) -> u32 {
    self.sandbox.memory_pages
  }

  /// 从缓存中取出 wasm 路径对应的 `Module`；mtime 变化时会重新加载。
  /// 调用者传入实际 wasm 文件路径，有助于跨调用复用。
  ///
  /// Phase 9.2：首次加载时跑 [`plugin_security::scan_imports`]——
  /// 当前宿主未暴露任何 host fn，任何 import 即拒。失败转为
  /// `AppError::Plugin`，避免后续 instantiate 给出隐晦错误。
  pub fn get_or_load_module(&self, path: &Path) -> crate::error::AppResult<Module> {
    let mtime = std::fs::metadata(path)?.modified()?;

    // 读路径与当前 mtime 后在锁外进行预编译，避免重调时锁占用率过高。
    if let Ok(cache) = self.cache.lock() {
      if let Some(entry) = cache.get(path) {
        if entry.mtime == mtime {
          return Ok(entry.module.clone());
        }
      }
    }

    // 未命中 / mtime 变化 → 重新加载。读二进制 + 预编译
    let bytes = std::fs::read(path)?;

    // Phase 9.2: SHA256 lock。site.json 有 lock 条目则比对；不匹配拒绝。
    // 无条目 = warn-only（fork 用户首次部署允许）。
    #[cfg(feature = "server")]
    if let Some(expected) = self.expected_sha256_for(path) {
      if let Err(detail) = plugin_security::verify_sha256(&bytes, &expected) {
        return Err(crate::error::AppError::plugin(format!(
          "plugin {} failed sha256 lock: {}",
          path.display(),
          detail
        )));
      }
    }

    let module = Module::new(&self.engine, &bytes)?;

    // Phase 9.2: import scan。任何 import 即拒（白名单 = ∅）。
    if let Err(detail) = plugin_security::scan_imports(&module) {
      return Err(crate::error::AppError::plugin(format!(
        "plugin {} failed import scan: {}",
        path.display(),
        detail
      )));
    }

    // 写入缓存（错锁不阻塞调用，仅跳过本次缓存）
    if let Ok(mut cache) = self.cache.lock() {
      cache.insert(path.to_path_buf(), CachedModule { module: module.clone(), mtime });
    }

    Ok(module)
  }

  /// 显式失效某个插件路径的缓存。供 admin 刷新 / hot reload 使用。
  ///
  /// 同时失效 Module 缓存 + theme CSS 输出缓存；如果上层将来添加更多
  /// 路径维度的 output cache 也应在这里清。
  pub fn invalidate(&self, path: &Path) {
    if let Ok(mut cache) = self.cache.lock() {
      cache.remove(path);
    }
    if let Ok(mut cache) = self.theme_css_cache.lock() {
      cache.remove(path);
    }
  }

  /// 清空全部插件缓存。
  pub fn invalidate_all(&self) {
    if let Ok(mut cache) = self.cache.lock() {
      cache.clear();
    }
    if let Ok(mut cache) = self.theme_css_cache.lock() {
      cache.clear();
    }
  }

  /// 执行插件中的函数并传递字符串。
  ///
  /// 沙箱保护（[`SandboxConfig`]）：fuel cap 防死循环；linear memory cap 防 OOM；
  /// tokio timeout 兜底；输出长度 clamp 防 host 巨缓冲分配。
  /// 调用本身走 [`tokio::task::spawn_blocking`]，避免 wasmi 同步执行卡住 tokio worker。
  #[cfg(feature = "server")]
  pub async fn call_with_string(
    &self,
    wasm_bytes: &[u8],
    func_name: &str,
    input: &str,
  ) -> crate::error::AppResult<String> {
    let module = Module::new(&self.engine, wasm_bytes)?;
    self.invoke_module(module, func_name, input).await
  }

  /// 从路径加载插件（走缓存）并调用。
  #[cfg(feature = "server")]
  pub async fn call_path_with_string(
    &self,
    path: &Path,
    func_name: &str,
    input: &str,
  ) -> crate::error::AppResult<String> {
    let module = self.get_or_load_module(path)?;
    self.invoke_module(module, func_name, input).await
  }

  /// 对已预编译的 `Module` 调用指定导出函数。私有路径，所有公开 API 都走该 fn。
  #[cfg(feature = "server")]
  async fn invoke_module(
    &self,
    module: Module,
    func_name: &str,
    input: &str,
  ) -> crate::error::AppResult<String> {
    use crate::error::AppError;
    use std::time::Duration;

    let engine = self.engine.clone();
    let linker = self.linker.clone();
    let sandbox = self.sandbox;
    let func_name_owned = func_name.to_string();
    let func_name_for_err = func_name_owned.clone();
    let input = input.to_string();
    let timeout = Duration::from_secs(sandbox.timeout_secs);

    let fut = tokio::task::spawn_blocking(move || {
      invoke_module_sync(&engine, &linker, &module, &func_name_owned, &input, sandbox)
    });

    match tokio::time::timeout(timeout, fut).await {
      Ok(Ok(result)) => result,
      Ok(Err(join_err)) => Err(AppError::plugin(format!("wasm worker join failed: {}", join_err))),
      Err(_) => Err(AppError::plugin(format!(
        "wasm invoke timed out after {}s ({})",
        sandbox.timeout_secs, func_name_for_err
      ))),
    }
  }

  /// 聚合多个主题插件的 CSS（直接传字节）。
  #[cfg(feature = "server")]
  pub async fn aggregate_theme_css(&self, wasm_modules: &[Vec<u8>]) -> String {
    let mut aggregated_css = String::new();
    for wasm_bytes in wasm_modules {
      if let Ok(css) = self.call_with_string(wasm_bytes, "get_theme_css", "").await {
        aggregated_css.push_str(&css);
        aggregated_css.push('\n');
      }
    }
    aggregated_css
  }

  /// 按路径聚合主题 CSS：除了走 Module 缓存，还会在 mtime 不变的情况下
  /// 直接复用上次的 CSS 输出（Phase 8.5）。
  ///
  /// 性能差：在 navbar 上每渲染一次页面会调用此 fn 一次；过去每次都跑
  /// 一遍完整的 wasmi instantiate + alloc/dealloc。引入 mtime cache 后，
  /// 同一插件二进制下只跑一次 wasm，后续都是 HashMap 查找。
  #[cfg(feature = "server")]
  pub async fn aggregate_theme_css_paths(&self, paths: &[PathBuf]) -> String {
    let mut aggregated_css = String::new();
    for path in paths {
      // 取 mtime；拿不到（文件被删 / 权限错）也不报错，走 uncached 路径
      let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

      // 第一步：检查 cache
      if let Some(mtime) = mtime {
        if let Ok(cache) = self.theme_css_cache.lock() {
          if let Some((cached_mtime, cached_css)) = cache.get(path) {
            if *cached_mtime == mtime {
              aggregated_css.push_str(cached_css);
              aggregated_css.push('\n');
              continue;
            }
          }
        }
      }

      // 缓存未命中 / mtime 不一致 → 调插件 + 写回 cache
      if let Ok(css) = self.call_path_with_string(path, "get_theme_css", "").await {
        // Phase 9.2: theme CSS allowlist。命中黑名单 pattern 整段跳过 + warn。
        // 防 CSS 注入做数据外渗（`url(http://evil.com/?cookie=...)` 之类）。
        let hits = plugin_security::sanitize_theme_css(&css);
        if !hits.is_empty() {
          tracing::warn!(
            target: "plugin_security",
            plugin = %path.display(),
            patterns = ?hits,
            "theme CSS rejected: matched blacklist patterns"
          );
          continue;
        }
        if let Some(mtime) = mtime {
          if let Ok(mut cache) = self.theme_css_cache.lock() {
            cache.insert(path.to_path_buf(), (mtime, Arc::new(css.clone())));
          }
        }
        aggregated_css.push_str(&css);
        aggregated_css.push('\n');
      }
    }
    aggregated_css
  }

  /// 沙箱校验一段 wasm 字节是否是合法的站点插件。
  ///
  /// 用于 hot reload（Phase 5.1）：admin 上传的 wasm 在落盘前必须先编译 +
  /// 实例化，确认它能在本宿主的 [`wasmi`] 引擎上运行且导出了 ABI 约定的
  /// `memory` / `alloc` / `dealloc`。校验本身也跑在 fuel + 内存限制 + timeout
  /// 之下，防止恶意 `start` 函数直接卡死上传流程。
  ///
  /// Phase 9.2：在 instantiate 前增加 import scan（白名单 = ∅），让上传链路
  /// 第一时间拒绝有 host fn 依赖的非法插件。
  #[cfg(feature = "server")]
  pub async fn validate_plugin_bytes(&self, bytes: &[u8]) -> Result<(), String> {
    use std::time::Duration;

    let module =
      Module::new(&self.engine, bytes).map_err(|e| format!("无法编译为合法 wasm 模块: {}", e))?;

    plugin_security::scan_imports(&module).map_err(|e| format!("import 扫描拒绝: {}", e))?;

    let engine = self.engine.clone();
    let linker = self.linker.clone();
    let sandbox = self.sandbox;
    let timeout = Duration::from_secs(sandbox.timeout_secs);

    let fut =
      tokio::task::spawn_blocking(move || validate_plugin_sync(&engine, &linker, &module, sandbox));

    match tokio::time::timeout(timeout, fut).await {
      Ok(Ok(result)) => result,
      Ok(Err(e)) => Err(format!("校验线程异常: {}", e)),
      Err(_) => Err(format!("插件校验超时 ({}s)", sandbox.timeout_secs)),
    }
  }

  /// Phase 9.2：对一段 wasm 字节跑完整安全检测（import scan + manifest
  /// 一致性 + 可选 SHA256 比对），返回结构化报告。
  ///
  /// 用法：
  /// - `admin_upload_plugin` 收到上传后立即调，hard failure 即拒绝
  /// - `lock_plugins` CLI 跑一遍生成 site.json `plugins_lock` 字段
  ///
  /// 实例化检查（fuel / memory / timeout）走另一条路径
  /// [`Self::validate_plugin_bytes`]，这里只做静态扫描。
  #[cfg(feature = "server")]
  pub async fn scan_uploaded_plugin(
    &self,
    bytes: &[u8],
    expected_sha256: Option<&str>,
  ) -> plugin_security::SecurityReport {
    use plugin_security::SecurityReport;

    let module_result = Module::new(&self.engine, bytes);

    let (imports_ok, imports_detail) = match &module_result {
      Ok(m) => match plugin_security::scan_imports(m) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e)),
      },
      Err(e) => (false, Some(format!("module decode failed: {}", e))),
    };

    let (manifest_ok, manifest_detail, manifest_extras) = match &module_result {
      Ok(m) => match self.call_with_string(bytes, "get_manifest", "").await {
        Ok(json) => match serde_json::from_str::<sdk::PluginManifest>(&json) {
          Ok(manifest) => match plugin_security::verify_manifest_consistency(&manifest, m) {
            Ok(extras) => (true, None, extras),
            Err(detail) => (false, Some(detail), Vec::new()),
          },
          Err(e) => (false, Some(format!("manifest JSON 解析失败: {}", e)), Vec::new()),
        },
        Err(e) => (false, Some(format!("get_manifest 调用失败: {}", e)), Vec::new()),
      },
      Err(_) => (false, Some("wasm 解码失败，跳过 manifest 检查".into()), Vec::new()),
    };

    let sha256_status = expected_sha256.map(|hex| plugin_security::verify_sha256(bytes, hex));

    SecurityReport {
      imports_ok,
      imports_detail,
      manifest_ok,
      manifest_detail,
      manifest_extras,
      sha256_status,
    }
  }
}

/// 同步执行 wasmi 调用：装载 Store + 设置 fuel + 设置 ResourceLimiter + 跑 alloc/call/dealloc。
///
/// 输出长度在 host 分配缓冲前 clamp 到 `sandbox.output_limit`，避免恶意插件返回 `len = u32::MAX`
/// 导致 host 端 OOM。
#[cfg(feature = "server")]
fn invoke_module_sync(
  engine: &Engine,
  linker: &Linker<StoreLimits>,
  module: &Module,
  func_name: &str,
  input: &str,
  sandbox: SandboxConfig,
) -> crate::error::AppResult<String> {
  use crate::error::AppError;

  let mut store = Store::new(engine, sandbox.store_limits());
  store.limiter(|s| s);
  store.set_fuel(sandbox.fuel).map_err(|e| AppError::plugin(format!("set_fuel failed: {}", e)))?;

  let instance = linker.instantiate(&mut store, module)?.start(&mut store)?;

  let memory = instance
    .get_memory(&store, "memory")
    .ok_or_else(|| AppError::plugin("WASM module has no memory export"))?;

  let alloc_fn = instance.get_typed_func::<i32, i32>(&store, "alloc")?;
  let dealloc_fn = instance.get_typed_func::<(i32, i32), ()>(&store, "dealloc")?;

  let input_bytes = input.as_bytes();
  let input_len = input_bytes.len() as i32;
  let input_ptr = alloc_fn.call(&mut store, input_len)?;
  memory
    .write(&mut store, input_ptr as usize, input_bytes)
    .map_err(|e| AppError::plugin(format!("wasm memory write failed: {}", e)))?;

  let target_fn = instance.get_typed_func::<(i32, i32), u64>(&store, func_name)?;
  let packed_result = target_fn.call(&mut store, (input_ptr, input_len))?;

  let result_ptr = (packed_result >> 32) as i32;
  let raw_result_len = (packed_result & 0xFFFFFFFF) as i32;

  // 在 host 端 clamp 输出长度：插件可能返回恶意巨大的 len（甚至 u32::MAX 高位 bit）。
  // 任何 < 0 视为 0；超出 output_limit 直接报错而不是默默截断（避免数据损坏被静默吞掉）。
  if raw_result_len < 0 {
    return Err(AppError::plugin(format!(
      "plugin returned negative output length ({})",
      raw_result_len
    )));
  }
  let result_len_usize = raw_result_len as usize;
  if result_len_usize > sandbox.output_limit {
    return Err(AppError::plugin(format!(
      "plugin output size {} exceeds limit {} bytes",
      result_len_usize, sandbox.output_limit
    )));
  }

  let mut result_buf = vec![0u8; result_len_usize];
  memory
    .read(&store, result_ptr as usize, &mut result_buf)
    .map_err(|e| AppError::plugin(format!("wasm memory read failed: {}", e)))?;
  let result_str = String::from_utf8(result_buf)
    .map_err(|e| AppError::plugin(format!("plugin output not valid UTF-8: {}", e)))?;

  dealloc_fn.call(&mut store, (input_ptr, input_len))?;
  dealloc_fn.call(&mut store, (result_ptr, raw_result_len))?;

  Ok(result_str)
}

#[cfg(feature = "server")]
fn validate_plugin_sync(
  engine: &Engine,
  linker: &Linker<StoreLimits>,
  module: &Module,
  sandbox: SandboxConfig,
) -> Result<(), String> {
  let mut store = Store::new(engine, sandbox.store_limits());
  store.limiter(|s| s);
  store.set_fuel(sandbox.fuel).map_err(|e| format!("set_fuel failed: {}", e))?;

  let instance = linker
    .instantiate(&mut store, module)
    .map_err(|e| format!("实例化失败: {}", e))?
    .start(&mut store)
    .map_err(|e| format!("启动失败: {}", e))?;

  if instance.get_memory(&store, "memory").is_none() {
    return Err("插件缺少 `memory` 导出".to_string());
  }
  instance
    .get_typed_func::<i32, i32>(&store, "alloc")
    .map_err(|_| "插件缺少 `alloc(i32) -> i32` 导出".to_string())?;
  instance
    .get_typed_func::<(i32, i32), ()>(&store, "dealloc")
    .map_err(|_| "插件缺少 `dealloc(i32, i32)` 导出".to_string())?;
  Ok(())
}

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;
  use std::fs;

  #[tokio::test]
  async fn test_i18n_fluent_plugin() {
    let wasm_path = "../../target/wasm32-unknown-unknown/release/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }

    let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
    let manager = PluginManager::new();

    let input = serde_json::json!({
        "key": "nav-blog",
        "lang": "en"
    })
    .to_string();

    let result = manager
      .call_with_string(&wasm_bytes, "translate", &input)
      .await
      .expect("Failed to call plugin");
    assert_eq!(result, "Blog");
  }

  #[tokio::test]
  async fn test_theme_plugin() {
    let wasm_path = "../../target/wasm32-unknown-unknown/release/theme_ocean_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }

    let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
    let manager = PluginManager::new();

    let css = manager.aggregate_theme_css(&[wasm_bytes]).await;
    assert!(css.contains("--color-primary"));
  }

  /// 实际调用插件验证 cache hit：同一路径调用 N 次仅产生 1 个缓存条目。
  /// 该测试仅在插件 wasm 已构建时运行。
  #[tokio::test]
  async fn test_path_cache_hit() {
    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    let manager = PluginManager::new();
    let path = std::path::Path::new(wasm_path);
    let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
    for _ in 0..5 {
      let _ = manager.call_path_with_string(path, "translate", &input).await;
    }
    let cache = manager.cache.lock().unwrap();
    assert_eq!(cache.len(), 1, "应仅产生 1 个缓存条目");
  }

  #[tokio::test]
  async fn test_invalidate_clears_cache_entry() {
    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    let manager = PluginManager::new();
    let path = std::path::Path::new(wasm_path);
    let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
    let _ = manager.call_path_with_string(path, "translate", &input).await;
    assert_eq!(manager.cache.lock().unwrap().len(), 1);
    manager.invalidate(path);
    assert_eq!(manager.cache.lock().unwrap().len(), 0, "调用 invalidate 后缓存应为空");
  }

  #[tokio::test]
  async fn test_invalidate_all_clears_cache() {
    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    let manager = PluginManager::new();
    let path = std::path::Path::new(wasm_path);
    let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
    let _ = manager.call_path_with_string(path, "translate", &input).await;
    manager.invalidate_all();
    assert!(manager.cache.lock().unwrap().is_empty());
  }

  /// Phase 5.1 内存回收代理测试：反复 invalidate + 重新加载，缓存条目数
  /// 始终保持为 1，旧 `Module` 句柄在 invalidate 时被 Drop（不会累积）。
  /// 真正的 RSS 长跑监测在 `docs/OPERATIONS.md` 记录，单测只验证缓存不泄漏。
  #[tokio::test]
  async fn test_reload_evicts_old_module_cache_stays_bounded() {
    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    let manager = PluginManager::new();
    let path = std::path::Path::new(wasm_path);
    let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
    for _ in 0..50 {
      let _ = manager.call_path_with_string(path, "translate", &input).await;
      manager.invalidate(path);
      let _ = manager.call_path_with_string(path, "translate", &input).await;
      assert_eq!(
        manager.cache.lock().unwrap().len(),
        1,
        "重复 reload 后缓存条目应恒为 1，不应累积旧 Module"
      );
    }
  }

  /// `validate_plugin_bytes` 接受真实插件 wasm、拒绝垃圾字节。
  #[tokio::test]
  async fn test_validate_plugin_bytes() {
    let manager = PluginManager::new();
    // 垃圾字节：不是 wasm
    assert!(manager.validate_plugin_bytes(b"not a wasm module").await.is_err());
    assert!(manager.validate_plugin_bytes(&[]).await.is_err());

    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    let bytes = fs::read(wasm_path).expect("read plugin");
    assert!(manager.validate_plugin_bytes(&bytes).await.is_ok(), "真实插件 wasm 应通过结构校验");
  }

  /// 沙箱：fuel 默认值（无 env override）应为 100M；通过 `set_fuel` 注入到 Store
  /// 已在 invoke_module_sync 内验证。
  #[test]
  fn sandbox_defaults_match_documentation() {
    let manager = PluginManager::new();
    assert_eq!(manager.fuel_limit(), DEFAULT_WASM_FUEL);
    assert_eq!(manager.memory_pages_limit(), DEFAULT_WASM_MEMORY_PAGES);
    assert_eq!(manager.output_limit(), DEFAULT_OUTPUT_LIMIT);
  }

  /// Phase 8.5：theme CSS chunk cache 命中验证。同一 path 调用 N 次后，
  /// cache 内只会出现 1 个条目；测试不要求 wasmi 实际执行次数（mock 太重），
  /// 只验证缓存条目层级 + invalidate 的语义。
  #[tokio::test]
  async fn theme_css_chunk_cache_populates_and_invalidates() {
    let wasm_path = "../../assets/plugins/theme_ocean_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    let manager = PluginManager::new();
    let path = std::path::Path::new(wasm_path);
    // 第一次：填充 cache
    let css1 = manager.aggregate_theme_css_paths(&[path.to_path_buf()]).await;
    assert!(!css1.is_empty(), "首次应当产出真实 CSS");
    assert_eq!(
      manager.theme_css_cache.lock().unwrap().len(),
      1,
      "首次调用后 theme_css_cache 应该恰有 1 条"
    );

    // 第二次：命中 cache，结果应一致
    let css2 = manager.aggregate_theme_css_paths(&[path.to_path_buf()]).await;
    assert_eq!(css1, css2, "缓存命中应返回相同字节");
    assert_eq!(manager.theme_css_cache.lock().unwrap().len(), 1, "命中不应新增条目");

    // invalidate 后清空
    manager.invalidate(path);
    assert!(
      manager.theme_css_cache.lock().unwrap().is_empty(),
      "invalidate 应同时清 Module + theme CSS cache"
    );

    // 重新调用 → cache 重新填充
    let _css3 = manager.aggregate_theme_css_paths(&[path.to_path_buf()]).await;
    assert_eq!(manager.theme_css_cache.lock().unwrap().len(), 1);

    // invalidate_all 清空
    manager.invalidate_all();
    assert!(manager.theme_css_cache.lock().unwrap().is_empty());
  }

  /// 沙箱：fuel 极小 → 真实插件无法完成 alloc/写入，wasmi 应返回 trap 错误，
  /// 而不是 hang。验证 [`Config::consume_fuel`] + [`Store::set_fuel`] 路径生效。
  #[tokio::test]
  async fn fuel_exhaustion_traps_quickly() {
    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    // SAFETY: 单测序列化运行
    unsafe {
      std::env::set_var("WASM_FUEL_LIMIT", "1");
    }
    let manager = PluginManager::new();
    unsafe {
      std::env::remove_var("WASM_FUEL_LIMIT");
    }
    assert_eq!(manager.fuel_limit(), 1);
    let path = std::path::Path::new(wasm_path);
    let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
    let start = std::time::Instant::now();
    let res = manager.call_path_with_string(path, "translate", &input).await;
    let elapsed = start.elapsed();
    assert!(res.is_err(), "fuel=1 时应当 trap，结果: {:?}", res);
    // 必须快速失败，不能因为没 fuel 限制而 hang。给 1s 充裕余地。
    assert!(elapsed < std::time::Duration::from_secs(1), "fuel trap 耗时过长: {:?}", elapsed);
  }

  /// 沙箱：恶意插件返回 `result_len` 超过 host output_limit 时，
  /// host 应拒绝分配巨缓冲并报清晰错误，而不是直接 `vec![0u8; huge]`。
  ///
  /// 实现细节：在 invoke_module_sync 内部，packed_result 解出的 len 先与
  /// `sandbox.output_limit` 比较，超限直接返回 `AppError::plugin`。
  /// 这里通过把 PluginManager 的 output_limit 调成 1 字节并跑真实插件来代理验证
  /// （真实插件至少返回若干字节）。
  #[tokio::test]
  async fn output_length_is_clamped_before_alloc() {
    let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      return;
    }
    // 用 env override 把 output_limit 设成 1 byte
    // SAFETY: 单测序列化运行；其他测试不依赖该值
    unsafe {
      std::env::set_var("WASM_OUTPUT_LIMIT", "1");
    }
    let manager = PluginManager::new();
    unsafe {
      std::env::remove_var("WASM_OUTPUT_LIMIT");
    }
    assert_eq!(manager.output_limit(), 1);
    let path = std::path::Path::new(wasm_path);
    let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
    let res = manager.call_path_with_string(path, "translate", &input).await;
    match res {
      Err(e) => {
        assert!(format!("{}", e).contains("exceeds limit"), "应当因输出超限拒绝，实际错误: {}", e)
      }
      Ok(_) => panic!("output_limit=1 时不应允许真实插件返回完整字符串"),
    }
  }
}
