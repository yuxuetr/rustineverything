pub mod forum;
pub mod server;

use rustineverything_sdk::AppModule;

pub struct ForumModule;

impl AppModule for ForumModule {
    fn name(&self) -> &'static str {
        "Forum"
    }
}
