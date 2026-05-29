pub mod docs;
pub mod server;

use sdk::AppModule;

pub struct DocsModule;

impl AppModule for DocsModule {
  fn name(&self) -> &'static str {
    "Docs"
  }
}
