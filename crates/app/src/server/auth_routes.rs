//! S7（风险 R8）：OAuth 登录 / 回调 / 登出路由。从 `main.rs` 拆出，行为不变。
//!
//! 流程概览（Phase 7.2 多副本方案）：
//! 1. `/api/auth/login/{provider}`：生成 state + PKCE verifier，加密成
//!    `oauth_pkce` cookie 下发，302 到 OAuth 授权 URL。
//! 2. `/api/auth/callback/{provider}`：读 `oauth_pkce` cookie → 校验 →
//!    签发 JWT session cookie + 清 PKCE cookie + 跳转首页。
//! 3. `/api/auth/logout`：清 session cookie。

use axum::extract::{Path, Query};
use axum::http::{header::SET_COOKIE, HeaderMap};
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;

/// 挂载 OAuth 路由。`cookie_is_secure` 由 BASE_URL 是否 https 决定。
pub fn mount(router: Router, cookie_is_secure: bool) -> Router {
  router
    // 1. 处理登录跳转：生成 state + verifier，加密成 oauth_pkce cookie 下发，
    //    然后 302 到 OAuth 授权 URL。state / verifier 自此随浏览器走 → 支持多副本。
    .route(
      "/api/auth/login/{provider}",
      get(move |Path(provider): Path<String>| async move {
        use app_core::auth::build_pkce_set_cookie;
        match crate::server::prepare_login_for_provider(provider).await {
          Ok((url, cookie_value)) => {
            let set_cookie = build_pkce_set_cookie(&cookie_value, cookie_is_secure);
            let mut response = Redirect::temporary(&url).into_response();
            if let Ok(v) = set_cookie.parse() {
              response.headers_mut().insert(SET_COOKIE, v);
            }
            response
          }
          Err(e) => {
            tracing::error!(error = %e, "auth: prepare_login failed");
            Redirect::temporary("/?error=login_failed").into_response()
          }
        }
      }),
    )
    // 2. 处理 OAuth 回调：读 oauth_pkce cookie → 校验 + 签发 JWT Cookie + 清掉 PKCE cookie + 跳转。
    .route(
      "/api/auth/callback/{provider}",
      get(
        move |Path(provider): Path<String>,
              Query(params): Query<std::collections::HashMap<String, String>>,
              headers: HeaderMap| async move {
          use app_core::auth::{build_pkce_clear_cookie, extract_pkce_cookie, PkceCookiePayload};
          let code = params.get("code").cloned().unwrap_or_default();
          let received_state = params.get("state").cloned().unwrap_or_default();

          // 从 Cookie 头里抽出 oauth_pkce 并解密
          let cookie_value = headers
            .get_all(axum::http::header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(extract_pkce_cookie);

          let clear_pkce_cookie = build_pkce_clear_cookie(cookie_is_secure);
          let attach_clear = |mut resp: axum::response::Response| {
            if let Ok(v) = clear_pkce_cookie.parse() {
              resp.headers_mut().append(SET_COOKIE, v);
            }
            resp
          };

          let pkce = match cookie_value.as_deref().map(PkceCookiePayload::decode) {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
              tracing::warn!(error = %e, "auth: oauth_pkce cookie decode failed");
              return attach_clear(
                Redirect::temporary("/?error=auth_session_invalid").into_response(),
              );
            }
            None => {
              tracing::warn!("auth: oauth_pkce cookie missing on callback");
              return attach_clear(
                Redirect::temporary("/?error=auth_session_missing").into_response(),
              );
            }
          };

          match crate::server::auth_callback_internal(code, provider, received_state, pkce).await {
            Ok((_message, jwt_token)) => {
              let secure_flag = if cookie_is_secure { "; Secure" } else { "" };
              let session_cookie = format!(
                "session={}; HttpOnly; Path=/; Max-Age=604800; SameSite=Lax{}",
                jwt_token, secure_flag
              );
              let mut response = Redirect::temporary("/").into_response();
              if let Ok(v) = session_cookie.parse() {
                response.headers_mut().append(SET_COOKIE, v);
              }
              attach_clear(response)
            }
            Err(e) => {
              tracing::error!(error = %e, "auth callback failed");
              attach_clear(Redirect::temporary("/?error=auth_failed").into_response())
            }
          }
        },
      ),
    )
    // 3. 登出：清除 Cookie
    .route(
      "/api/auth/logout",
      get(move || async move {
        let secure_flag = if cookie_is_secure { "; Secure" } else { "" };
        let cookie_str =
          format!("session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax{}", secure_flag);
        let mut response = Redirect::temporary("/").into_response();
        if let Ok(cookie_val) = cookie_str.parse() {
          response.headers_mut().insert(axum::http::header::SET_COOKIE, cookie_val);
        }
        response
      }),
    )
}
