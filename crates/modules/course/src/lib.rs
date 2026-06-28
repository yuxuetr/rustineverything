#[cfg(feature = "server")]
pub mod alipay;
pub mod course;
pub mod pay_ui;
pub mod server;
#[cfg(feature = "server")]
pub mod wechat;

use sdk::AppModule;

pub struct CourseModule;

impl AppModule for CourseModule {
  fn name(&self) -> &'static str {
    "Course"
  }
}
