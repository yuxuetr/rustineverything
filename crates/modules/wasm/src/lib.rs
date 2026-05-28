//! Wasm 内容板块（Phase 6）。结构同其它内容模块：`text` 持元数据 + 纯逻辑 +
//! 单测，`server` 扫描 `assets/topics/wasm/`，`wasm` 提供 RSX 页面。

pub mod server;
pub mod text;
pub mod wasm;

use rustineverything_sdk::AppModule;

pub struct WasmModule;

impl AppModule for WasmModule {
  fn name(&self) -> &'static str {
    "WASM"
  }
}
