pub mod server;

use sdk::AppModule;

pub struct UploadsModule;

impl AppModule for UploadsModule {
  fn name(&self) -> &'static str {
    "Uploads"
  }
}
