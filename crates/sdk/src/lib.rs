use serde::{Deserialize, Serialize};
use std::mem;
use std::slice;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// 导出供 WASM 调用的分配函数
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf); // 告诉 Rust 不要释放这段内存
    ptr
}

/// 导出供 WASM 调用的释放函数
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, size);
}

/// 核心 Trait 定义
pub trait Plugin {
    fn manifest(&self) -> PluginManifest;
    fn on_load(&self) {}
}
