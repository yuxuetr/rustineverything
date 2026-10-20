pub mod course;
#[cfg(feature = "payments")]
pub mod pay_ui;
pub mod pricing;
#[cfg(feature = "payments")]
pub mod product;
pub mod server;

/// 宿主启动时用（`host::log_config_status`）。
#[cfg(all(feature = "payments", feature = "server"))]
pub use module_payment;

use sdk::AppModule;

pub struct CourseModule;

impl AppModule for CourseModule {
  fn name(&self) -> &'static str {
    "Course"
  }
}
