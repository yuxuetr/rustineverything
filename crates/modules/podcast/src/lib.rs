pub mod podcast;
pub mod server;

use rustineverything_sdk::AppModule;

pub struct PodcastModule;

impl AppModule for PodcastModule {
    fn name(&self) -> &'static str {
        "Podcast"
    }
}
