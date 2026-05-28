pub mod docs;
pub mod server;

use rustineverything_sdk::AppModule;

pub struct DocsModule;

impl AppModule for DocsModule {
  fn name(&self) -> &'static str {
    "Docs"
  }
}
