pub mod entities;
pub mod auth;
pub mod db;

use wasmi::{Engine, Linker, Module, Store};
use std::sync::Arc;

pub struct PluginManager {
    engine: Engine,
    linker: Linker<()>,
}

impl PluginManager {
    pub fn new() -> Self {
        let engine = Engine::default();
        let linker = Linker::new(&engine);
        Self { engine, linker }
    }

    /// 执行插件中的函数并传递字符串
    pub fn call_with_string(&self, wasm_bytes: &[u8], func_name: &str, input: &str) -> Result<String, Box<dyn std::error::Error>> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, ());
        let instance = self.linker.instantiate(&mut store, &module)?.start(&mut store)?;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_i18n_fluent_plugin() {
        let wasm_path = "/Users/hal/.target/wasm32-unknown-unknown/release/i18n_fluent_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() { return; }

        let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
        let manager = PluginManager::new();

        // 模拟翻译请求
        let input = serde_json::json!({
            "key": "nav-blog",
            "lang": "en"
        }).to_string();

        let result = manager.call_with_string(&wasm_bytes, "translate", &input).expect("Failed to call plugin");
        assert_eq!(result, "Blog");

        let input_zh = serde_json::json!({
            "key": "nav-blog",
            "lang": "zh"
        }).to_string();

        let result_zh = manager.call_with_string(&wasm_bytes, "translate", &input_zh).expect("Failed to call plugin");
        assert_eq!(result_zh, "博客");
    }

    #[test]
    fn test_theme_plugin() {
        let wasm_path = "/Users/hal/.target/wasm32-unknown-unknown/release/theme_ocean_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() { return; }

        let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
        let manager = PluginManager::new();

        let css = manager.aggregate_theme_css(&[wasm_bytes]);
        assert!(css.contains("--color-primary"));
        assert!(css.contains("oklch"));
        println!("Theme Plugin Test Passed! Aggregated CSS:\n{}", css);
    }
}
