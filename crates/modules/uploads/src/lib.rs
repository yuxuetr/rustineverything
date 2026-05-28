pub mod server;

use rustineverything_sdk::AppModule;

pub struct UploadsModule;

impl AppModule for UploadsModule {
  fn name(&self) -> &'static str {
    "Uploads"
  }
}
