#[cfg(feature = "server")]
pub mod engine;
#[cfg(feature = "server")]
pub mod indexer;
pub mod search;
pub mod server;
pub mod text;

use sdk::AppModule;

pub struct SearchModule;

impl AppModule for SearchModule {
  fn name(&self) -> &'static str {
    "Search"
  }
}
