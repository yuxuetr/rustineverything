#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod auth;
#[cfg(feature = "server")]
pub mod db;
pub mod engines;
pub mod error;
pub mod i18n;
pub mod session;
pub mod settings;
pub mod utils;

// Re-export SDK types needed by the app crate
pub use rustineverything_sdk::AuthProviderDisplay;
pub use rustineverything_sdk::{capabilities, PluginManifest, SDK_ABI_VERSION};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;
use wasmi::{Engine, Linker, Module, Store};

/// 全局共享的 [`PluginManager`] 实例，让插件 `Module` 缓存跨 server fn 调用复用。
static SHARED_PLUGIN_MANAGER: LazyLock<PluginManager> = LazyLock::new(PluginManager::new);

/// 返回全局共享的 `PluginManager`。i18n / 主题 / Auth 等高频调用点推荐调用。
pub fn shared_plugin_manager() -> &'static PluginManager {
    &SHARED_PLUGIN_MANAGER
}

/// 插件缓存条目：记录预编译后的 wasmi `Module` + 文件 mtime。
struct CachedModule {
    module: Module,
    mtime: SystemTime,
}

pub struct PluginManager {
    engine: Engine,
    linker: Linker<()>,
    /// 提供按路径复用 `Module` 的能力。在 i18n / 主题等高频调用场景下
    /// 可以避免重复读文件 + 重复调用 `Module::new`。
    cache: Mutex<HashMap<PathBuf, CachedModule>>,
}

impl PluginManager {
    pub fn new() -> Self {
        let engine = Engine::default();
        let linker = Linker::new(&engine);
        Self {
            engine,
            linker,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 从缓存中取出 wasm 路径对应的 `Module`；mtime 变化时会重新加载。
    /// 调用者传入实际 wasm 文件路径，有助于跨调用复用。
    pub fn get_or_load_module(&self, path: &Path) -> Result<Module, Box<dyn std::error::Error>> {
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
        let module = Module::new(&self.engine, &bytes)?;

        // 写入缓存（错锁不阻塞调用，仅跳过本次缓存）
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                path.to_path_buf(),
                CachedModule {
                    module: module.clone(),
                    mtime,
                },
            );
        }

        Ok(module)
    }

    /// 显式失效某个插件路径的缓存。供 admin 刷新 / hot reload 使用。
    pub fn invalidate(&self, path: &Path) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(path);
        }
    }

    /// 清空全部插件缓存。
    pub fn invalidate_all(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// 执行插件中的函数并传递字符串
    pub fn call_with_string(&self, wasm_bytes: &[u8], func_name: &str, input: &str) -> Result<String, Box<dyn std::error::Error>> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        self.invoke_module(&module, func_name, input)
    }

    /// 从路径加载插件（走缓存）并调用。高频调用场景推荐使用该接口，
    /// 避免重复 `fs::read` + `Module::new` 的开销。
    pub fn call_path_with_string(
        &self,
        path: &Path,
        func_name: &str,
        input: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let module = self.get_or_load_module(path)?;
        self.invoke_module(&module, func_name, input)
    }

    /// 对已预编译的 `Module` 调用指定导出函数。仅在 crate 内部使用。
    fn invoke_module(
        &self,
        module: &Module,
        func_name: &str,
        input: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut store = Store::new(&self.engine, ());
        let instance = self.linker.instantiate(&mut store, module)?.start(&mut store)?;

        // 1. 获取插件的线性内存
        let memory = instance.get_memory(&store, "memory").ok_or("WASM module has no memory export")?;

        // 2. 获取插件导出的分配函数
        let alloc_fn = instance.get_typed_func::<i32, i32>(&store, "alloc")?;
        let dealloc_fn = instance.get_typed_func::<(i32, i32), ()>(&store, "dealloc")?;

        // 3. 在插件中分配空间并写入输入字符串
        let input_bytes = input.as_bytes();
        let input_len = input_bytes.len() as i32;
        let input_ptr = alloc_fn.call(&mut store, input_len)?;
        memory.write(&mut store, input_ptr as usize, input_bytes)?;

        // 4. 调用目标函数 (ptr, len) -> u64
        let target_fn = instance.get_typed_func::<(i32, i32), u64>(&store, func_name)?;
        let packed_result = target_fn.call(&mut store, (input_ptr, input_len))?;

        // 5. 解析结果 (高32位ptr, 低32位len)
        let result_ptr = (packed_result >> 32) as i32;
        let result_len = (packed_result & 0xFFFFFFFF) as i32;

        let mut result_buf = vec![0u8; result_len as usize];
        memory.read(&store, result_ptr as usize, &mut result_buf)?;
        let result_str = String::from_utf8(result_buf)?;

        // 6. 清理内存
        dealloc_fn.call(&mut store, (input_ptr, input_len))?;
        dealloc_fn.call(&mut store, (result_ptr, result_len))?;

        Ok(result_str)
    }

    /// 聚合多个主题插件的 CSS
    pub fn aggregate_theme_css(&self, wasm_modules: &[Vec<u8>]) -> String {
        let mut aggregated_css = String::new();
        for wasm_bytes in wasm_modules {
            if let Ok(css) = self.call_with_string(wasm_bytes, "get_theme_css", "") {
                aggregated_css.push_str(&css);
                aggregated_css.push('\n');
            }
        }
        aggregated_css
    }

    /// 按路径聚合主题 CSS（走缓存）。
    pub fn aggregate_theme_css_paths(&self, paths: &[PathBuf]) -> String {
        let mut aggregated_css = String::new();
        for path in paths {
            if let Ok(css) = self.call_path_with_string(path, "get_theme_css", "") {
                aggregated_css.push_str(&css);
                aggregated_css.push('\n');
            }
        }
        aggregated_css
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_i18n_fluent_plugin() {
        let wasm_path = "../../target/wasm32-unknown-unknown/release/i18n_fluent_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() { return; }

        let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
        let manager = PluginManager::new();

        let input = serde_json::json!({
            "key": "nav-blog",
            "lang": "en"
        }).to_string();

        let result = manager.call_with_string(&wasm_bytes, "translate", &input).expect("Failed to call plugin");
        assert_eq!(result, "Blog");
    }

    #[test]
    fn test_theme_plugin() {
        let wasm_path = "../../target/wasm32-unknown-unknown/release/theme_ocean_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() { return; }

        let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
        let manager = PluginManager::new();

        let css = manager.aggregate_theme_css(&[wasm_bytes]);
        assert!(css.contains("--color-primary"));
    }

    /// 实际调用插件验证 cache hit：同一路径调用 N 次仅产生 1 个缓存条目。
    /// 该测试仅在插件 wasm 已构建时运行。
    #[test]
    fn test_path_cache_hit() {
        let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() {
            return;
        }
        let manager = PluginManager::new();
        let path = std::path::Path::new(wasm_path);
        let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
        for _ in 0..5 {
            let _ = manager.call_path_with_string(path, "translate", &input);
        }
        let cache = manager.cache.lock().unwrap();
        assert_eq!(cache.len(), 1, "应仅产生 1 个缓存条目");
    }

    #[test]
    fn test_invalidate_clears_cache_entry() {
        let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() {
            return;
        }
        let manager = PluginManager::new();
        let path = std::path::Path::new(wasm_path);
        let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
        let _ = manager.call_path_with_string(path, "translate", &input);
        assert_eq!(manager.cache.lock().unwrap().len(), 1);
        manager.invalidate(path);
        assert_eq!(manager.cache.lock().unwrap().len(), 0, "调用 invalidate 后缓存应为空");
    }

    #[test]
    fn test_invalidate_all_clears_cache() {
        let wasm_path = "../../assets/plugins/i18n_fluent_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() {
            return;
        }
        let manager = PluginManager::new();
        let path = std::path::Path::new(wasm_path);
        let input = serde_json::json!({"key": "nav-blog", "lang": "en"}).to_string();
        let _ = manager.call_path_with_string(path, "translate", &input);
        manager.invalidate_all();
        assert!(manager.cache.lock().unwrap().is_empty());
    }
}
