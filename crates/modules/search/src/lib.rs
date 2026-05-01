pub mod search;
pub mod server;
#[cfg(feature = "server")]
pub mod engine;
#[cfg(feature = "server")]
pub mod indexer;
pub mod text;

use rustineverything_sdk::AppModule;

pub struct SearchModule;

impl AppModule for SearchModule {
    fn name(&self) -> &'static str {
        "Search"
    }
}
