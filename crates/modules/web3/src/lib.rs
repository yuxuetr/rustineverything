//! Web3 内容板块（Phase 6）。结构同其它内容模块：`text` 持元数据 + 纯逻辑 +
//! 单测，`server` 扫描 `assets/topics/web3/`，`web3` 提供 RSX 页面。

pub mod web3;
pub mod server;
pub mod text;

use rustineverything_sdk::AppModule;

pub struct Web3Module;

impl AppModule for Web3Module {
    fn name(&self) -> &'static str {
        "Web3"
    }
}
