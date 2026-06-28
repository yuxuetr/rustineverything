#[cfg(feature = "server")]
pub mod alipay;
pub mod course;
pub mod server;

use sdk::AppModule;

pub struct CourseModule;

impl AppModule for CourseModule {
  fn name(&self) -> &'static str {
    "Course"
  }
}
