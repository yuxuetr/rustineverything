pub mod markdown;
pub mod server;

use rustineverything_sdk::AppModule;

pub struct BlogModule;

impl AppModule for BlogModule {
    fn name(&self) -> &'static str {
        "Blog"
    }
}
