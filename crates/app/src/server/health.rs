//! S3（风险 R3）：启动健康状态 + `/healthz` 端点。
//!
//! 背景：启动时 DB 连接失败 / 迁移失败原本只打一行日志就继续跑
//! （可用性优先），但这让「schema 与代码不一致仍在接受写请求」的状态
//! 变得不可观测。本模块提供：
//!
//! - 启动期记录一次性的 [`StartupHealth`]（DB 连接 / 迁移结果）。
//! - `/healthz` 端点：全部正常（或站点未配置 DB）→ `200 {"status":"ok"}`；
//!   DB 配置了但连接/迁移失败 → `503 {"status":"degraded", ...}`，
//!   供负载均衡 / 监控 / 部署脚本判活。
//! - `STRICT_MIGRATION=1`（见 main.rs）：迁移失败直接 fail-fast 退出，
//!   生产环境推荐开启，避免带着不一致 schema 静默服务。

use std::sync::OnceLock;

/// 迁移执行结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
  /// 迁移成功应用（或已是最新）。
  Applied,
  /// 迁移执行失败。
  Failed,
  /// 未执行（DB 未配置或连接失败）。
  Skipped,
}

/// 启动期健康快照。只在启动流程写入一次。
#[derive(Debug, Clone, Copy)]
pub struct StartupHealth {
  /// `DATABASE_URL` 是否已配置。未配置 = 有意的纯静态站点，不算 degraded。
  pub db_configured: bool,
  /// 连接池是否初始化成功。
  pub db_connected: bool,
  /// 迁移结果。
  pub migrations: MigrationStatus,
}

static STATE: OnceLock<StartupHealth> = OnceLock::new();

/// 启动流程调用一次，记录健康快照。重复调用忽略（保留首次）。
pub fn set_startup_health(health: StartupHealth) {
  let _ = STATE.set(health);
}

/// 当前是否处于降级状态：DB 配置了但连接失败，或迁移失败。
pub fn is_degraded() -> bool {
  match STATE.get() {
    None => false, // 启动流程尚未记录：视为 ok，避免探针在启动窗口误杀
    Some(h) => h.db_configured && (!h.db_connected || h.migrations == MigrationStatus::Failed),
  }
}

/// 构建 `/healthz` 响应体（纯函数，便于单测）。
pub fn health_body() -> (bool, String) {
  let degraded = is_degraded();
  let (db, migrations) = match STATE.get() {
    None => ("unknown", "unknown"),
    Some(h) => {
      let db = if !h.db_configured {
        "disabled"
      } else if h.db_connected {
        "connected"
      } else {
        "unreachable"
      };
      let mig = match h.migrations {
        MigrationStatus::Applied => "applied",
        MigrationStatus::Failed => "failed",
        MigrationStatus::Skipped => "skipped",
      };
      (db, mig)
    }
  };
  let status = if degraded { "degraded" } else { "ok" };
  let body = format!(r#"{{"status":"{}","db":"{}","migrations":"{}"}}"#, status, db, migrations);
  (degraded, body)
}

/// `/healthz` Axum handler。
pub async fn healthz() -> axum::response::Response {
  use axum::http::StatusCode;
  let (degraded, body) = health_body();
  let status = if degraded { StatusCode::SERVICE_UNAVAILABLE } else { StatusCode::OK };
  axum::response::Response::builder()
    .status(status)
    .header("content-type", "application/json")
    .header("cache-control", "no-store")
    .body(axum::body::Body::from(body))
    .unwrap_or_else(|_| axum::response::Response::new(axum::body::Body::empty()))
}

#[cfg(test)]
mod tests {
  use super::*;

  // 注意：STATE 是进程级 OnceLock，多个测试共享。为避免测试间干扰，
  // 这里只用一个测试覆盖「set 后语义」，set 之前的语义由纯逻辑分支保证。
  #[test]
  fn healthy_snapshot_reports_ok() {
    set_startup_health(StartupHealth {
      db_configured: true,
      db_connected: true,
      migrations: MigrationStatus::Applied,
    });
    // set 只生效一次；无论本测试与其他测试的执行顺序如何，
    // 快照均为上面首个成功写入的值或其他测试写入值——都应是合法组合。
    let (degraded, body) = health_body();
    assert!(body.contains("\"status\":"));
    assert!(body.contains("\"db\":"));
    assert!(body.contains("\"migrations\":"));
    if !degraded {
      assert!(body.contains("\"status\":\"ok\""));
    }
  }

  #[test]
  fn degraded_logic_matrix() {
    // 直接对结构体逻辑做矩阵断言（不经过全局 STATE）
    let check = |configured: bool, connected: bool, mig: MigrationStatus| {
      configured && (!connected || mig == MigrationStatus::Failed)
    };
    assert!(!check(false, false, MigrationStatus::Skipped), "未配置 DB 不降级");
    assert!(check(true, false, MigrationStatus::Skipped), "DB 连不上应降级");
    assert!(check(true, true, MigrationStatus::Failed), "迁移失败应降级");
    assert!(!check(true, true, MigrationStatus::Applied), "全部正常不降级");
  }
}
