//! 全应用统一错误类型 [`AppError`]。
//!
//! ## 设计目标
//! - **单一来源**：用一个枚举覆盖 Db / Plugin / Auth / Io / Validation 等常见
//!   失败模式，逐步替换原本散落的 `Box<dyn std::error::Error>` 返回值。
//! - **客户端不暴露内部细节**：错误转换到 `ServerFnError` 时，仅返回简短的
//!   人类可读消息（数据库错误统一返回"内部错误"），原始细节走日志。
//! - **零分配的常见路径**：Display 直接复用 enum 内部字段，Debug 自动派生。
//!
//! ## 用法
//! ```rust
//! # use rustineverything_core::error::{AppError, AppResult};
//! fn parse_id(s: &str) -> AppResult<i64> {
//!     s.parse::<i64>()
//!         .map_err(|e| AppError::validation(format!("invalid id: {}", e)))
//! }
//! ```
//!
//! ## 与 ServerFnError 的关系
//! 实现 `From<AppError> for ServerFnError`（仅在 `server` feature 下）：
//! - `Db(_)` → "内部错误"（日志会包含具体错误）
//! - 其他变体 → 各自的人类可读字符串
//!
//! 这样 server fn 可以直接 `?` 一个 `Result<_, AppError>`，调用方仅看到
//! 受控的错误信息。

use std::fmt;

/// 整个应用的统一错误类型。
#[derive(Debug)]
pub enum AppError {
  /// 数据库 / SeaORM 失败
  #[cfg(feature = "server")]
  Db(sea_orm::DbErr),
  /// 插件层（WASM 加载 / 调用 / ABI 不兼容）
  Plugin(String),
  /// 鉴权 / OAuth / Session 相关
  Auth(String),
  /// 标准库 IO
  Io(std::io::Error),
  /// 输入校验（用户输入不合法 / MIME 非法等）
  Validation(String),
  /// 兜底：尚未细化的错误，可逐步替换
  Other(String),
}

impl AppError {
  /// 构造一个 `Plugin` 错误（接受任何可转 `String` 的输入）
  pub fn plugin<T: Into<String>>(msg: T) -> Self {
    AppError::Plugin(msg.into())
  }

  /// 构造一个 `Auth` 错误
  pub fn auth<T: Into<String>>(msg: T) -> Self {
    AppError::Auth(msg.into())
  }

  /// 构造一个 `Validation` 错误
  pub fn validation<T: Into<String>>(msg: T) -> Self {
    AppError::Validation(msg.into())
  }

  /// 构造一个 `Other` 兜底错误
  pub fn other<T: Into<String>>(msg: T) -> Self {
    AppError::Other(msg.into())
  }

  /// 返回适合直接发给客户端的简短消息。**不包含**敏感内部信息。
  /// `Db` 永远只返回固定的 "内部错误"，原始 `DbErr` 详情写在日志里。
  pub fn client_message(&self) -> String {
    match self {
      #[cfg(feature = "server")]
      AppError::Db(_) => "内部错误".to_string(),
      AppError::Plugin(msg) => format!("插件错误: {}", msg),
      AppError::Auth(msg) => msg.clone(),
      AppError::Io(_) => "内部错误".to_string(),
      AppError::Validation(msg) => msg.clone(),
      AppError::Other(msg) => msg.clone(),
    }
  }
}

impl fmt::Display for AppError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      #[cfg(feature = "server")]
      AppError::Db(e) => write!(f, "Db error: {}", e),
      AppError::Plugin(msg) => write!(f, "Plugin error: {}", msg),
      AppError::Auth(msg) => write!(f, "Auth error: {}", msg),
      AppError::Io(e) => write!(f, "IO error: {}", e),
      AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
      AppError::Other(msg) => write!(f, "{}", msg),
    }
  }
}

impl std::error::Error for AppError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      #[cfg(feature = "server")]
      AppError::Db(e) => Some(e),
      AppError::Io(e) => Some(e),
      _ => None,
    }
  }
}

// ---- 自动转换 ----

#[cfg(feature = "server")]
impl From<sea_orm::DbErr> for AppError {
  fn from(e: sea_orm::DbErr) -> Self {
    AppError::Db(e)
  }
}

impl From<std::io::Error> for AppError {
  fn from(e: std::io::Error) -> Self {
    AppError::Io(e)
  }
}

impl From<String> for AppError {
  fn from(s: String) -> Self {
    AppError::Other(s)
  }
}

impl From<&str> for AppError {
  fn from(s: &str) -> Self {
    AppError::Other(s.to_string())
  }
}

impl From<serde_json::Error> for AppError {
  fn from(e: serde_json::Error) -> Self {
    AppError::Validation(format!("JSON parse error: {}", e))
  }
}

impl From<serde_yaml::Error> for AppError {
  fn from(e: serde_yaml::Error) -> Self {
    AppError::Validation(format!("YAML parse error: {}", e))
  }
}

/// wasmi 运行时错误（编译 / 实例化 / 调用 wasm）统一归到 `Plugin`。
impl From<wasmi::Error> for AppError {
  fn from(e: wasmi::Error) -> Self {
    AppError::Plugin(format!("wasm runtime error: {}", e))
  }
}

/// reqwest 错误（仅 core 内 OAuth HTTP 调用使用）归到 `Auth`。
#[cfg(feature = "server")]
impl From<reqwest::Error> for AppError {
  fn from(e: reqwest::Error) -> Self {
    AppError::Auth(format!("HTTP 请求失败: {}", e))
  }
}

#[cfg(feature = "server")]
impl From<AppError> for dioxus::fullstack::ServerFnError {
  fn from(e: AppError) -> Self {
    // 在 server 端把内部错误日志化，避免吞掉关键信息
    match &e {
      AppError::Db(inner) => {
        tracing::error!(error = %inner, "AppError::Db (forwarded to ServerFnError)");
      }
      AppError::Io(inner) => {
        tracing::error!(error = %inner, "AppError::Io (forwarded to ServerFnError)");
      }
      _ => {}
    }
    dioxus::fullstack::ServerFnError::new(e.client_message())
  }
}

/// 便捷类型别名
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn validation_round_trip() {
    let err = AppError::validation("blog_id is empty");
    assert!(matches!(err, AppError::Validation(_)));
    assert_eq!(err.client_message(), "blog_id is empty");
    assert_eq!(format!("{}", err), "Validation error: blog_id is empty");
  }

  #[test]
  fn plugin_constructor_works() {
    let err = AppError::plugin("module not found");
    assert_eq!(err.client_message(), "插件错误: module not found");
  }

  #[test]
  fn auth_message_passes_through() {
    // Auth 消息直接透传给客户端（用户需要看到具体错误）
    let err = AppError::auth("请先登录");
    assert_eq!(err.client_message(), "请先登录");
  }

  #[test]
  fn other_default_works() {
    let err: AppError = "boom".into();
    assert_eq!(err.client_message(), "boom");
  }

  #[test]
  fn io_error_redacted_to_internal() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "/etc/secrets");
    let err: AppError = io.into();
    // client_message 不能包含路径
    assert_eq!(err.client_message(), "内部错误");
    // 但 Display 会带细节（用于日志）
    assert!(format!("{}", err).contains("/etc/secrets"));
  }

  #[cfg(feature = "server")]
  #[test]
  fn db_error_redacted_to_internal() {
    // 用一个不会触发实际数据库连接的 DbErr 变体
    let db = sea_orm::DbErr::Custom("password=hunter2 leaked".to_string());
    let err: AppError = db.into();
    assert_eq!(err.client_message(), "内部错误");
    // 但内部 Display 仍然记录原文
    assert!(format!("{}", err).contains("password=hunter2"));
  }

  #[test]
  fn source_chain_for_io() {
    let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: AppError = io.into();
    let source = std::error::Error::source(&err);
    assert!(source.is_some(), "io error 应能向下追踪 source");
  }
}
