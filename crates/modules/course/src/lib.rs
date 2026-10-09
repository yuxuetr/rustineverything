pub mod course;
#[cfg(feature = "payments")]
pub mod pay_ui;
pub mod server;

use sdk::AppModule;

pub struct CourseModule;

impl AppModule for CourseModule {
  fn name(&self) -> &'static str {
    "Course"
  }
}
