pub mod cases;
pub mod server;
pub mod text;

use sdk::AppModule;

pub struct CasesModule;

impl AppModule for CasesModule {
  fn name(&self) -> &'static str {
    "Cases"
  }
}
