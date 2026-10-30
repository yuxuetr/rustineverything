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

/// 索引文档的 kind 属于哪个模块（模块开关 / 编译分层据此过滤）。`None` = 不属于可关的模块。
pub fn module_of_kind(kind: &str) -> Option<&str> {
  match kind {
    "blog" => Some("blog"),
    "doc" => Some("docs"),
    "topic" => Some("forum"),
    "case" => Some("cases"),
    "paper" => Some("papers"),
    "course" => Some("course"),
    k if BOARD_KINDS.contains(&k) => Some(k),
    _ => None,
  }
}

pub struct SearchModule;

impl AppModule for SearchModule {
  fn name(&self) -> &'static str {
    "Search"
  }
}
