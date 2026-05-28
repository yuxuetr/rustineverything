pub mod server;

use rustineverything_sdk::AppModule;

pub struct CommentsModule;

impl AppModule for CommentsModule {
  fn name(&self) -> &'static str {
    "Comments"
  }
}
