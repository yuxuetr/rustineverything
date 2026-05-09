//! [`AuthEngine`]：包装现有 [`crate::auth::AuthService`]，让它实现统一的
//! [`Engine`] trait（Phase 1C.4）。
//!
//! AuthService 自身已经具备完整功能（OAuth provider 列表、登录 URL、
//! 回调处理、PKCE/state CSRF 防御）。本引擎只在 [`Engine`] 接口下转发，
//! 保持向后兼容；后续可逐步把 server fn 改为通过 EngineRegistry 拿到
//! `AuthEngine`，避免 `app/server/mod.rs::build_auth_service` 之类的临时
//! 构造代码。
//!
//! ## 仅 server feature
//! `AuthService` 依赖 sea-orm + reqwest 等仅在 server 端可用的 crate，
//! 因此 `AuthEngine` 也用 `cfg(feature = "server")` 包裹。

#![cfg(feature = "server")]

use std::path::PathBuf;

use crate::auth::{AuthConfig, AuthService};

use super::{Engine, EngineContext};
use crate::error::AppResult;

/// AuthEngine：重命名后的 AuthService 包装，提供 [`Engine`] 接口。
pub struct AuthEngine {
    inner: AuthService,
}

impl AuthEngine {
    pub fn new(config: AuthConfig, plugin_dir: PathBuf) -> Self {
        Self {
            inner: AuthService::new(config, plugin_dir),
        }
    }

    /// 拿到内部的 `AuthService` 引用，便于复用现有 server fn。
    pub fn service(&self) -> &AuthService {
        &self.inner
    }

    /// 当 hot reload 等场景需要替换 service 时使用。
    pub fn replace_service(&mut self, service: AuthService) {
        self.inner = service;
    }
}

impl Engine for AuthEngine {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn init(&mut self, ctx: &EngineContext) -> AppResult<()> {
        // 当 site_config.auth.enabled 为 false 时仅发个日志提示；
        // AuthService 内部 list_available_providers 会自然返回空列表。
        if !ctx.site_config.auth.enabled {
            eprintln!("[AuthEngine] site.json::auth.enabled=false，登录功能已禁用");
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> AuthEngine {
        AuthEngine::new(
            AuthConfig {
                base_url: "http://localhost:8080".to_string(),
            },
            PathBuf::from("assets/plugins"),
        )
    }

    #[test]
    fn engine_name_is_auth() {
        let e = make();
        assert_eq!(<AuthEngine as Engine>::name(&e), "auth");
    }

    #[test]
    fn init_ok_with_default_ctx() {
        let mut e = make();
        let ctx = EngineContext::for_tests();
        assert!(e.init(&ctx).is_ok());
    }

    #[test]
    fn service_accessor_returns_underlying_auth_service() {
        let e = make();
        // 验证 service() 返回的对象上能调用现有方法
        assert_eq!(e.service().config.base_url, "http://localhost:8080");
    }

    #[test]
    fn list_available_providers_empty_when_no_config() {
        let e = make();
        // SiteConfig::default() auth.enabled=false → 必然空
        let providers = e
            .service()
            .list_available_providers(&crate::settings::SiteConfig::default());
        assert!(providers.is_empty());
    }
}
