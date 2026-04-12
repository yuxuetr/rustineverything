pub mod podcast;

use rustineverything_sdk::AppModule;

pub struct PodcastModule;

impl AppModule for PodcastModule {
    fn name(&self) -> &'static str {
        "Podcast"
    }
}
