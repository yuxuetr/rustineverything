//! Cli 内容板块（Phase 6）。结构同其它内容模块：`text` 持元数据 + 纯逻辑 +
//! 单测，`server` 扫描 `assets/topics/cli/`，`cli` 提供 RSX 页面。

pub mod cli;
pub mod server;
pub mod text;

use rustineverything_sdk::AppModule;

pub struct CliModule;

impl AppModule for CliModule {
  fn name(&self) -> &'static str {
    "CLI"
  }
}
