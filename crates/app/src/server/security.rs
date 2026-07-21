//! S1（风险 R6）：统一安全响应头中间件。
//!
//! 在 Axum 层为**所有**响应注入基础安全头，作为 gateway 之外的默认防线
//! （gateway 是可选部署组件，app 裸跑时也应有安全头）：
//!
//! - `Content-Security-Policy`：保守策略。因为现状大量使用内联 `<style>` /
//!   `<script>`（`document::Style`、主题 CSS `dangerous_inner_html`、
//!   prism/mermaid 引导脚本）以及 Dioxus WASM，必须允许
//!   `'unsafe-inline'` + `'wasm-unsafe-eval'`。后续 nonce 化方向见下文注释。
//! - `X-Content-Type-Options: nosniff`：禁止 MIME 嗅探（uploads 目录风险）。
//! - `Referrer-Policy: strict-origin-when-cross-origin`。
//! - `X-Frame-Options: DENY` + CSP `frame-ancestors 'none'`：防点击劫持。
//!   注意这是「别人不能 iframe 我们」；我们自己嵌 YouTube / Bilibili 走
//!   `frame-src` 白名单，两者不冲突。
//!
//! ## 运维开关
//! - `CSP_POLICY`：完整覆盖默认 CSP（留空字符串 = 不发送 CSP 头）。
//! - `SECURITY_HEADERS_DISABLED=1`：整体禁用（本地排障用，生产勿开）。
//!
//! ## 后续 nonce 化方向（暂不实施）
//! 彻底移除 `'unsafe-inline'` 需要：每请求生成 nonce → 注入 SSR HTML 的所有
//! 内联 style/script 标签 → CSP 带 `'nonce-…'`。Dioxus 当前对 document::Style
//! 无 nonce 透传能力，等上游支持后再收紧。

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

/// 默认 CSP。目录说明见模块注释。
///
/// - `img-src … https:`：用户头像来自任意 OAuth 提供商 CDN。
/// - `connect-src … ws: wss:`：dx serve 开发态热重载走 WebSocket；
///   生产无 ws 连接时该白名单不构成额外风险面（仍受同源脚本约束）。
/// - `frame-src`：widgets 的 YouTube / Bilibili 嵌入组件。
pub fn default_csp() -> String {
  [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: https:",
    "font-src 'self' data:",
    "media-src 'self' https:",
    "connect-src 'self' ws: wss:",
    "frame-src https://www.youtube.com https://player.bilibili.com",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "frame-ancestors 'none'",
  ]
  .join("; ")
}

/// 解析生效的 CSP 值：`CSP_POLICY` env 覆盖 > 默认值。
/// 返回 `None` 表示运维显式配置了空字符串（= 不发送 CSP 头）。
fn effective_csp() -> Option<String> {
  match std::env::var("CSP_POLICY") {
    Ok(v) if v.trim().is_empty() => None,
    Ok(v) => Some(v),
    Err(_) => Some(default_csp()),
  }
}

/// 构建全部安全头（纯函数，便于单测）。value 构造失败的条目直接跳过
/// （不 panic；仅在非法 env 覆盖时可能发生）。
pub fn build_security_headers(csp: Option<&str>) -> Vec<(HeaderName, HeaderValue)> {
  let mut out: Vec<(HeaderName, HeaderValue)> = Vec::with_capacity(4);
  if let Some(csp) = csp {
    if let Ok(v) = HeaderValue::from_str(csp) {
      out.push((HeaderName::from_static("content-security-policy"), v));
    } else {
      tracing::warn!("security: CSP_POLICY contains invalid header characters; CSP not sent");
    }
  }
  out
    .push((HeaderName::from_static("x-content-type-options"), HeaderValue::from_static("nosniff")));
  out.push((
    HeaderName::from_static("referrer-policy"),
    HeaderValue::from_static("strict-origin-when-cross-origin"),
  ));
  out.push((HeaderName::from_static("x-frame-options"), HeaderValue::from_static("DENY")));
  out
}

/// 中间件本体：所有响应统一追加安全头（已存在同名头则不覆盖，
/// 允许具体路由按需下发更严格的策略）。
pub async fn security_headers_mw(req: Request, next: Next) -> Response {
  static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
  static HEADERS: std::sync::OnceLock<Vec<(HeaderName, HeaderValue)>> = std::sync::OnceLock::new();

  let disabled = *DISABLED
    .get_or_init(|| std::env::var("SECURITY_HEADERS_DISABLED").map(|v| v == "1").unwrap_or(false));

  let mut resp = next.run(req).await;
  if disabled {
    return resp;
  }

  let headers = HEADERS.get_or_init(|| build_security_headers(effective_csp().as_deref()));
  for (name, value) in headers {
    if !resp.headers().contains_key(name) {
      resp.headers_mut().insert(name.clone(), value.clone());
    }
  }
  resp
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_csp_contains_core_directives() {
    let csp = default_csp();
    for directive in [
      "default-src 'self'",
      "wasm-unsafe-eval",
      "frame-ancestors 'none'",
      "object-src 'none'",
      "frame-src https://www.youtube.com https://player.bilibili.com",
    ] {
      assert!(csp.contains(directive), "CSP 缺少指令: {}", directive);
    }
  }

  #[test]
  fn build_headers_includes_all_four() {
    let headers = build_security_headers(Some(&default_csp()));
    let names: Vec<&str> = headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"content-security-policy"));
    assert!(names.contains(&"x-content-type-options"));
    assert!(names.contains(&"referrer-policy"));
    assert!(names.contains(&"x-frame-options"));
  }

  #[test]
  fn build_headers_without_csp_still_has_baseline() {
    let headers = build_security_headers(None);
    let names: Vec<&str> = headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"content-security-policy"));
    assert_eq!(names.len(), 3, "无 CSP 时应有 3 个基础头");
  }

  #[test]
  fn invalid_csp_value_is_skipped_not_panic() {
    let headers = build_security_headers(Some("bad\nvalue"));
    let names: Vec<&str> = headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"content-security-policy"), "非法 CSP 值应跳过");
    assert!(names.contains(&"x-content-type-options"));
  }
}
