#[cfg(feature = "server")]
pub mod engine;
#[cfg(feature = "server")]
pub mod indexer;
pub mod search;
pub mod server;
pub mod text;

use sdk::AppModule;

/// Phase 6 内容板块的 kind（= module id，url 形如 `/<board>/<slug>`）。
/// 索引、模块开关过滤与查询的 kind 校验共用这一份。
pub const BOARD_KINDS: &[&str] = &["embedded", "ai", "web3", "wasm", "cli"];

pub struct SearchModule;

impl AppModule for SearchModule {
  fn name(&self) -> &'static str {
    "Search"
  }
}
