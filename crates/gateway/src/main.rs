//! Pingora-based TLS terminator + reverse proxy for the rustineverything.app
//! deployment. Listens on :443 (TLS) and :80 (HTTP→HTTPS 301), proxies all
//! traffic to the app on `127.0.0.1:8080`.
//!
//! Run:
//!   TLS_CERT_PATH=/data/cert/rustineverything.app.cert \
//!   TLS_KEY_PATH=/data/cert/rustineverything.app.key \
//!     ./rie-gateway
//!
//! Reload TLS certs without dropping connections: `kill -HUP <pid>` (Pingora
//! handles graceful reload natively when the binary is re-exec'd via the
//! signal — see Pingora docs for the full upgrade ritual).
//!
//! ## Phase 8.3 安全 / 性能强化
//! - 注入 OWASP-style 安全响应头（HSTS / CSP / X-Content-Type-Options / X-Frame-Options）
//! - `X-Forwarded-For` 改 `insert`（覆盖客户端伪造），同时 strip RFC 7239 `Forwarded`
//! - 引入 `governor` 做 per-IP token-bucket 限流：写端点 10 req/min，
//!   其余 60 req/min；触发返回 429 + `Retry-After: 60`
//! - 所有特性可通过 env 覆盖；开发态 `RATE_LIMIT_DISABLE=true` 全关

use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;

use async_trait::async_trait;
use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{Quota, RateLimiter};
use once_cell::sync::Lazy;
use pingora_core::server::Server;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_core::Result;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{http_proxy_service, ProxyHttp, Session};

/// 反代上游 = 本机 app 容器；HTTP/1.1 + keep-alive。
const UPSTREAM_ADDR: &str = "127.0.0.1:8080";
const UPSTREAM_SNI: &str = ""; // 上游 plain HTTP，不需要 SNI

const HTTPS_BIND: &str = "0.0.0.0:443";
const HTTP_BIND: &str = "0.0.0.0:80";

/// 默认 CSP：来源仅限同源；图片允许 data:/https:（兼容博客内联 base64 与跨站封面图）；
/// 样式允许 inline（Dioxus 的 hydration 标记需要），脚本严格同源。可通过 `CSP_POLICY` 覆盖。
const DEFAULT_CSP: &str =
  "default-src 'self'; img-src 'self' data: https:; style-src 'self' 'unsafe-inline'; \
   script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'";

const HSTS_VALUE: &str = "max-age=31536000; includeSubDomains";

/// 写端点路径前缀：匹配前缀即按"写"配额限流。命中数量预计很低，hash set
/// 比 trie/regex 更轻；显式列出比通配更安全（避免误伤静态资产）。
const WRITE_PATH_PREFIXES: &[&str] = &[
  "/api/auth/",
  "/api/upload",
  "/api/comments/",
  "/api/topics/",
  "/api/admin/",
  "/api/forum/",
  "/api/i18n/translate", // 隐含的"写"语义：会触发 wasmi 调用
];

/// 写端点配额：每 IP 10 req/min（覆盖恶意 brute-force 评论 / topic 创建）。
const WRITE_QUOTA_PER_MIN: u32 = 10;
/// 读端点配额：每 IP 60 req/min（足以覆盖正常用户浏览 + 静态资源 prefetch）。
const READ_QUOTA_PER_MIN: u32 = 60;

type KeyedLimiter =
  RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock, governor::middleware::NoOpMiddleware>;

/// 写端点限流器：恒为全局单例。
static WRITE_LIMITER: Lazy<Arc<KeyedLimiter>> = Lazy::new(|| {
  let n = NonZeroU32::new(env_u32("RATE_LIMIT_WRITE_PER_MIN", WRITE_QUOTA_PER_MIN))
    .unwrap_or(NonZeroU32::new(WRITE_QUOTA_PER_MIN).expect("compile-time non-zero"));
  Arc::new(RateLimiter::keyed(Quota::per_minute(n)))
});

/// 读端点限流器。
static READ_LIMITER: Lazy<Arc<KeyedLimiter>> = Lazy::new(|| {
  let n = NonZeroU32::new(env_u32("RATE_LIMIT_READ_PER_MIN", READ_QUOTA_PER_MIN))
    .unwrap_or(NonZeroU32::new(READ_QUOTA_PER_MIN).expect("compile-time non-zero"));
  Arc::new(RateLimiter::keyed(Quota::per_minute(n)))
});

/// CSP header value：启动时计算一次，避免每次响应再读 env。
static CSP_HEADER: Lazy<String> =
  Lazy::new(|| std::env::var("CSP_POLICY").unwrap_or_else(|_| DEFAULT_CSP.to_string()));

fn env_u32(key: &str, default: u32) -> u32 {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn rate_limit_disabled() -> bool {
  matches!(std::env::var("RATE_LIMIT_DISABLE").as_deref(), Ok("1") | Ok("true") | Ok("TRUE"))
}

fn is_write_path(path: &str) -> bool {
  WRITE_PATH_PREFIXES.iter().any(|p| path.starts_with(p))
}

/// 从 Pingora session 拿到客户端 IP。直连场景 = TCP 对端；不再信任客户端
/// 提供的 XFF / X-Real-IP（gateway 是边界，不存在更外层代理）。
fn client_ip(session: &Session) -> Option<IpAddr> {
  let addr = session.client_addr()?;
  let socket = addr.as_inet()?;
  Some(socket.ip())
}

struct AppGateway;

#[async_trait]
impl ProxyHttp for AppGateway {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  /// 限流：每 IP 写端点 10/min，读端点 60/min；触发返回 429 + Retry-After。
  ///
  /// 该 filter 在 upstream_peer 之前执行；返回 Ok(true) 短路、阻止反代发往上游。
  async fn request_filter(&self, session: &mut Session, _ctx: &mut ()) -> Result<bool> {
    if rate_limit_disabled() {
      return Ok(false);
    }

    let Some(ip) = client_ip(session) else {
      // 拿不到 IP（unix socket 等异常场景）→ 放行；不要因为元数据缺失而误伤合法请求
      return Ok(false);
    };

    let path = session.req_header().uri.path();
    let limiter = if is_write_path(path) { &WRITE_LIMITER } else { &READ_LIMITER };

    if limiter.check_key(&ip).is_ok() {
      return Ok(false);
    }

    log::warn!("rate-limit: 429 ip={} path={}", ip, path);
    let mut resp = ResponseHeader::build(429, None).unwrap();
    resp.append_header("Retry-After", "60").ok();
    resp.append_header("Content-Type", "text/plain; charset=utf-8").ok();
    resp.append_header("Content-Length", "33").ok();
    session.write_response_header_ref(&resp, false).await.ok();
    session.write_response_body(Some("rate limit exceeded, retry later\n".into()), true).await.ok();
    Ok(true)
  }

  /// 选定上游：恒为 app 容器（单 upstream，不做 LB）。
  async fn upstream_peer(
    &self,
    _session: &mut Session,
    _ctx: &mut (),
  ) -> Result<Box<HttpPeer>> {
    Ok(Box::new(HttpPeer::new(UPSTREAM_ADDR, false, UPSTREAM_SNI.into())))
  }

  /// 上游请求改写：补 X-Forwarded-* 让 app 端 cookie/日志看到真实客户端 + scheme。
  ///
  /// Phase 8.3 修正：
  /// - `X-Forwarded-For` 由 `append` 改 `insert`：客户端伪造的同名头会被覆盖而非保留，
  ///   避免日志 / rate limit 决策被毒化
  /// - 同时移除 RFC 7239 `Forwarded`（不同标准的等价头，攻击者可用来绕过 XFF 校验）
  async fn upstream_request_filter(
    &self,
    session: &mut Session,
    req: &mut RequestHeader,
    _ctx: &mut (),
  ) -> Result<()> {
    req.insert_header("X-Forwarded-Proto", "https").ok();
    // 清掉可能的伪造 Forwarded（RFC 7239 变体）
    req.remove_header("Forwarded");

    if let Some(addr) = session.client_addr() {
      let ip = addr.to_string();
      req.insert_header("X-Real-IP", ip.clone()).ok();
      // insert 覆盖客户端送上来的伪造 XFF；如未来真有可信外层代理，
      // 在该 filter 里改回 append + 白名单校验来源 IP
      req.insert_header("X-Forwarded-For", ip).ok();
    } else {
      // 拿不到客户端地址也要清掉伪造 XFF
      req.remove_header("X-Forwarded-For");
    }
    Ok(())
  }

  /// 响应改写：注入 OWASP-style 安全头。任意可被替换的头都设为 `insert`，
  /// 避免上游 dioxus_fullstack 默认值与此处冲突造成重复头。
  async fn response_filter(
    &self,
    _session: &mut Session,
    upstream_response: &mut ResponseHeader,
    _ctx: &mut (),
  ) -> Result<()> {
    upstream_response.insert_header("Strict-Transport-Security", HSTS_VALUE).ok();
    upstream_response.insert_header("X-Content-Type-Options", "nosniff").ok();
    upstream_response.insert_header("X-Frame-Options", "DENY").ok();
    upstream_response.insert_header("Referrer-Policy", "strict-origin-when-cross-origin").ok();
    upstream_response.insert_header("Content-Security-Policy", CSP_HEADER.as_str()).ok();
    // Server header 减少版本指纹
    upstream_response.insert_header("Server", "rie-gateway").ok();
    Ok(())
  }
}

/// 80 端口专用：所有请求 301 到同 host 的 https 版本。
struct HttpToHttps;

#[async_trait]
impl ProxyHttp for HttpToHttps {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  async fn request_filter(&self, session: &mut Session, _ctx: &mut ()) -> Result<bool> {
    let req = session.req_header();
    let host = req.headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    let path = req.uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let location = format!("https://{}{}", host, path);

    let mut resp = ResponseHeader::build(301, None).unwrap();
    resp.append_header("Location", location).ok();
    resp.append_header("Content-Length", "0").ok();
    // end_of_stream=true：301 无 body，回写头即结束
    session.write_response_header_ref(&resp, true).await.ok();
    Ok(true) // short-circuit；不再走 upstream_peer
  }

  async fn upstream_peer(
    &self,
    _session: &mut Session,
    _ctx: &mut (),
  ) -> Result<Box<HttpPeer>> {
    // 永远走不到这里：request_filter 已 Ok(true) 短路。
    Err(pingora_core::Error::new_str("unreachable: redirect short-circuit returned"))
  }
}

fn main() {
  env_logger::init();

  let mut server = Server::new(None).unwrap();
  server.bootstrap();

  // 443: TLS 终止 + 反代到 app
  let mut proxy = http_proxy_service(&server.configuration, AppGateway);
  let cert = std::env::var("TLS_CERT_PATH")
    .expect("TLS_CERT_PATH 未配置：指向 fullchain.pem 路径");
  let key = std::env::var("TLS_KEY_PATH")
    .expect("TLS_KEY_PATH 未配置：指向 privkey.pem 路径");
  let mut tls = pingora_core::listeners::tls::TlsSettings::intermediate(&cert, &key)
    .expect("加载 TLS 证书 / 私钥失败：检查路径与权限");
  tls.enable_h2();
  proxy.add_tls_with_settings(HTTPS_BIND, None, tls);
  server.add_service(proxy);

  // 80: HTTP→HTTPS 301
  let mut redirect = http_proxy_service(&server.configuration, HttpToHttps);
  redirect.add_tcp(HTTP_BIND);
  server.add_service(redirect);

  log::info!(
    "rie-gateway: HTTPS={} (TLS={}), HTTP={} → 301 https; upstream={}; rate_limit_disabled={}",
    HTTPS_BIND, cert, HTTP_BIND, UPSTREAM_ADDR, rate_limit_disabled()
  );
  server.run_forever();
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn write_path_classification() {
    assert!(is_write_path("/api/auth/login"));
    assert!(is_write_path("/api/upload"));
    assert!(is_write_path("/api/comments/post"));
    assert!(is_write_path("/api/topics/create"));
    assert!(is_write_path("/api/admin/dashboard"));
    assert!(is_write_path("/api/forum/create"));
    assert!(is_write_path("/api/i18n/translate"));
    // 读端点 / 静态资源 → 走 60/min 桶
    assert!(!is_write_path("/blog/welcome"));
    assert!(!is_write_path("/api/theme/aggregated-css"));
    assert!(!is_write_path("/sitemap.xml"));
    assert!(!is_write_path("/static/main.css"));
  }

  #[test]
  fn rate_limit_env_disable_flag() {
    // SAFETY: 单测内串行；并发干扰可忽略
    unsafe { std::env::set_var("RATE_LIMIT_DISABLE", "true") };
    assert!(rate_limit_disabled());
    unsafe { std::env::set_var("RATE_LIMIT_DISABLE", "1") };
    assert!(rate_limit_disabled());
    unsafe { std::env::set_var("RATE_LIMIT_DISABLE", "false") };
    assert!(!rate_limit_disabled());
    unsafe { std::env::remove_var("RATE_LIMIT_DISABLE") };
    assert!(!rate_limit_disabled());
  }

  /// 真实跑限流器：4 req 用同一 IP（quota 3/min）— 前 3 个 Ok，第 4 个 Err。
  #[test]
  fn keyed_limiter_enforces_quota() {
    let limiter: KeyedLimiter = RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(3).unwrap()));
    let ip: IpAddr = "203.0.113.7".parse().unwrap();
    assert!(limiter.check_key(&ip).is_ok());
    assert!(limiter.check_key(&ip).is_ok());
    assert!(limiter.check_key(&ip).is_ok());
    assert!(limiter.check_key(&ip).is_err(), "第 4 次同 IP 调用应被拒");
    // 别的 IP 独立计费
    let other: IpAddr = "198.51.100.5".parse().unwrap();
    assert!(limiter.check_key(&other).is_ok());
  }
}
