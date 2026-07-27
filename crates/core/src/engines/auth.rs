//! [`AuthEngine`]：包装 [`crate::auth::AuthService`] 的薄壳。
//!
//! AuthService 自身已经具备完整功能（OAuth provider 列表、登录 URL、
//! 回调处理、PKCE/state CSRF 防御）。Phase 1C.4 引入本壳是为了套上
//! Engine trait + EngineRegistry 生命周期；Phase 8.7 删掉了那套抽象，
//! 这里只剩一个轻包装供 server fn 在需要 `AuthService` 时统一构造。
//!
//! ## 仅 server feature
//! `AuthService` 依赖 sea-orm + reqwest 等仅在 server 端可用的 crate，
//! 因此 `AuthEngine` 也用 `cfg(feature = "server")` 包裹。

#![cfg(feature = "server")]

use std::path::PathBuf;

use crate::auth::{AuthConfig, AuthService};
use crate::settings::SiteConfig;

/// AuthEngine：包装 AuthService，提供少量便利方法。
pub struct AuthEngine {
  inner: AuthService,
}

impl AuthEngine {
  pub fn new(config: AuthConfig, plugin_dir: PathBuf) -> Self {
    Self { inner: AuthService::new(config, plugin_dir) }
  }

  /// 拿到内部的 `AuthService` 引用，便于复用现有 server fn。
  pub fn service(&self) -> &AuthService {
    &self.inner
  }

  /// 当 hot reload 等场景需要替换 service 时使用。
  pub fn replace_service(&mut self, service: AuthService) {
    self.inner = service;
  }

  /// 根据 site.json 判断登录是否启用。原 Engine::init 里的检查迁移到这里，
  /// 让调用方按需校验、记日志，而不是隐式在 init 钩子里做。
  pub fn warn_if_disabled(&self, site: &SiteConfig) {
    if !site.auth.enabled {
      tracing::warn!("auth: site.json::auth.enabled=false, login disabled");
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make() -> AuthEngine {
    AuthEngine::new(
      AuthConfig { base_url: "http://localhost:8080".to_string() },
      PathBuf::from("assets/plugins"),
    )
  }

  #[test]
  fn service_accessor_returns_underlying_auth_service() {
    let e = make();
    // 验证 service() 返回的对象上能调用现有方法
    assert_eq!(e.service().config.base_url, "http://localhost:8080");
  }

  #[test]
  fn warn_if_disabled_doesnt_panic_in_either_state() {
    let e = make();
    let mut cfg = SiteConfig::default();
    cfg.auth.enabled = false;
    e.warn_if_disabled(&cfg);
    cfg.auth.enabled = true;
    e.warn_if_disabled(&cfg);
  }

  #[tokio::test]
  async fn list_available_providers_empty_when_no_config() {
    let e = make();
    // SiteConfig::default() auth.enabled=false → 必然空
    let providers =
      e.service().list_available_providers(&crate::settings::SiteConfig::default()).await;
    assert!(providers.is_empty());
  }
}
