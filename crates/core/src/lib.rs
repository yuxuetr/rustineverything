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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_prefix_plugin() {
        let wasm_path = "/Users/hal/.target/wasm32-unknown-unknown/release/prefix_plugin.wasm";
        
        if !std::path::Path::new(wasm_path).exists() {
            return; // Skip if not built yet in this environment
        }

        let wasm_bytes = fs::read(wasm_path).expect("Failed to read wasm file");
        let manager = PluginManager::new();

        let input = "Hello World";
        let result = manager.call_with_string(&wasm_bytes, "process_text", input).expect("Failed to call plugin");

        assert_eq!(result, "[Plugin: Prefix] Hello World");
    }
}
