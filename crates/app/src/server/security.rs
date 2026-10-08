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
//! - `Strict-Transport-Security` / `Permissions-Policy`（SEC-19）。
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
/// - `img-src`：站内 + data: + 四个 OAuth 头像 CDN（SEC-08）。不放行任意 https：
///   外部图片会泄露浏览者 IP，也是主题 CSS / 用户内容的数据外泄通道。新增登录方式或
///   作者内容要用外部图片时，在这里加具体主机（或用 `CSP_POLICY` 覆盖）。
/// - `connect-src`：生产只有 `'self'`；debug 构建（dx serve）额外放行 `ws: wss:`
///   给热重载（SEC-19）。站点本身不用 WebSocket，放行任意主机只会多一条外泄通道。
/// - `frame-src`：widgets 的 YouTube / Bilibili 嵌入组件。
pub fn default_csp() -> String {
  csp_for(cfg!(debug_assertions))
}

fn csp_for(dev: bool) -> String {
  let connect_src = if dev { "connect-src 'self' ws: wss:" } else { "connect-src 'self'" };
  [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: https://avatars.githubusercontent.com https://*.googleusercontent.com \
     https://cdn.discordapp.com https://pbs.twimg.com",
    "font-src 'self' data:",
    "media-src 'self' https:",
    connect_src,
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
  let mut out: Vec<(HeaderName, HeaderValue)> = Vec::with_capacity(6);
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
  // SEC-19：与 gateway 的 HSTS_VALUE 一致；浏览器只认 HTTPS 响应上的 HSTS，dev 的 http 无影响。
  out.push((
    HeaderName::from_static("strict-transport-security"),
    HeaderValue::from_static("max-age=31536000; includeSubDomains"),
  ));
  // 站点用不到的强权限 API 一律关掉（含嵌入的 YouTube / Bilibili iframe）；fullscreen 不在列。
  out.push((
    HeaderName::from_static("permissions-policy"),
    HeaderValue::from_static(
      "camera=(), microphone=(), geolocation=(), payment=(), usb=(), browsing-topics=()",
    ),
  ));
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

  /// SEC-08：`img-src` 不放行任意 https（外部图片会泄露浏览者 IP，也是 CSS 外泄通道），
  /// 只放 OAuth 头像 CDN。
  #[test]
  fn img_src_only_allows_self_data_and_avatar_cdns() {
    let csp = default_csp();
    let img_src = csp.split("; ").find(|d| d.starts_with("img-src ")).expect("img-src directive");
    assert_eq!(
      img_src,
      "img-src 'self' data: https://avatars.githubusercontent.com https://*.googleusercontent.com \
       https://cdn.discordapp.com https://pbs.twimg.com"
    );
  }

  /// SEC-19：生产构建的 `connect-src` 只有 `'self'`；`ws:` / `wss:` 只给 dx serve 热重载。
  #[test]
  fn connect_src_allows_websockets_only_in_dev() {
    let connect_src = |dev: bool| {
      csp_for(dev).split("; ").find(|d| d.starts_with("connect-src ")).map(str::to_string)
    };
    assert_eq!(connect_src(false).as_deref(), Some("connect-src 'self'"));
    assert_eq!(connect_src(true).as_deref(), Some("connect-src 'self' ws: wss:"));
  }

  #[test]
  fn build_headers_includes_all_baseline_headers() {
    let headers = build_security_headers(Some(&default_csp()));
    let get = |name: &str| {
      headers.iter().find(|(n, _)| n.as_str() == name).and_then(|(_, v)| v.to_str().ok())
    };
    assert!(get("content-security-policy").is_some());
    assert_eq!(get("x-content-type-options"), Some("nosniff"));
    assert!(get("referrer-policy").is_some());
    assert_eq!(get("x-frame-options"), Some("DENY"));
    // SEC-19：app 裸跑（不经 gateway）也要有 HSTS 和 Permissions-Policy。
    assert_eq!(get("strict-transport-security"), Some("max-age=31536000; includeSubDomains"));
    let permissions = get("permissions-policy").unwrap_or_default();
    for feature in ["camera=()", "microphone=()", "geolocation=()"] {
      assert!(permissions.contains(feature), "Permissions-Policy 缺少 {feature}");
    }
  }

  #[test]
  fn build_headers_without_csp_still_has_baseline() {
    let headers = build_security_headers(None);
    let names: Vec<&str> = headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"content-security-policy"));
    assert_eq!(names.len(), 5, "无 CSP 时应有 5 个基础头");
  }

  #[test]
  fn invalid_csp_value_is_skipped_not_panic() {
    let headers = build_security_headers(Some("bad\nvalue"));
    let names: Vec<&str> = headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"content-security-policy"), "非法 CSP 值应跳过");
    assert!(names.contains(&"x-content-type-options"));
  }
}
