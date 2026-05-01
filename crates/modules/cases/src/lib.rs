pub mod cases;
pub mod server;
pub mod text;

use rustineverything_sdk::AppModule;

pub struct CasesModule;

impl AppModule for CasesModule {
    fn name(&self) -> &'static str {
        "Cases"
    }
}
