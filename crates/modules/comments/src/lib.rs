pub mod server;

use sdk::AppModule;

pub struct CommentsModule;

impl AppModule for CommentsModule {
  fn name(&self) -> &'static str {
    "Comments"
  }
}
