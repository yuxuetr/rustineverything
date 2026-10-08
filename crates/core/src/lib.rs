// S9（风险 R12）：生产代码禁 unwrap/expect（workspace lints）；测试代码豁免。
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

// 非 server 构建只用到 `AuthProviderDisplay`（登录弹窗）。
pub mod auth;
#[cfg(feature = "server")]
pub mod db;
pub mod engines;
#[cfg(feature = "server")]
pub mod entities;
pub mod error;
pub mod i18n;
pub mod session;
pub mod settings;
pub mod utils;

pub use auth::AuthProviderDisplay;
