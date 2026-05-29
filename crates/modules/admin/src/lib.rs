pub mod admin;
pub mod server;

use sdk::AppModule;

pub struct AdminModule;

impl AppModule for AdminModule {
  fn name(&self) -> &'static str {
    "Admin"
  }
}
