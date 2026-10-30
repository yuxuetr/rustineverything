//! Pingora-based TLS terminator + reverse proxy for the rustineverything.app
//! deployment. Listens on :443 (TLS) and :80 (HTTP→HTTPS 301), proxies all
//! traffic to the app on `127.0.0.1:8080`.
//!
//! Run:
//!   TLS_CERT_PATH=/data/cert/rustineverything.app.cert \
//!   TLS_KEY_PATH=/data/cert/rustineverything.app.key \
//!     ./rie-gateway
//!
//! 证书只在启动时读：换证书后重启网关（`scripts/cert_refresh.sh` 默认
//! `docker restart rie-gateway`）。不要发 SIGHUP：pingora 不处理它，进程直接退出。
//! SIGTERM 是平滑停机，宽限期见 `main` 里的 `ServerConf`。
//!
//! ## Phase 8.3 安全 / 性能强化
//! - 注入 HSTS；应用未下发时补 X-Content-Type-Options / X-Frame-Options / Referrer-Policy。
//!   CSP 只由应用设置，gateway 原样转发（SEC-10）
//! - `X-Forwarded-For` 改 `insert`（覆盖客户端伪造），同时 strip RFC 7239 `Forwarded`
//! - 限流只在 app 里做（`crates/app/src/server/rate_limit.rs`，按 XFF 分桶、只限 `/api/*`）。
//!   gateway 原先也按 IP 限流，但把静态资源算在内：一次冷加载约 57 个请求，
//!   读配额 60/min 刷新一次就会 429，`/api/auth/me` 还被算进 10/min 的写配额

use async_trait::async_trait;
use pingora_core::server::configuration::{Opt, ServerConf};
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

const HSTS_VALUE: &str = "max-age=31536000; includeSubDomains";

/// 下游 HTTP/1 keep-alive 连接两个请求之间最多空闲多久（秒）。
const DOWNSTREAM_IDLE_SECS: u64 = 60;

/// pingora 读完请求头后把 HTTP/1.1 keep-alive 设成无限（`Some(0)`），
/// 下一个请求之前的那次读就没有超时；换回有限值。`None`（客户端要求 close）不动。
fn cap_keepalive(session: &mut Session) {
  if session.get_keepalive() == Some(0) {
    session.set_keepalive(Some(DOWNSTREAM_IDLE_SECS));
  }
}

struct AppGateway;

#[async_trait]
impl ProxyHttp for AppGateway {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  async fn early_request_filter(&self, session: &mut Session, _ctx: &mut ()) -> Result<()> {
    cap_keepalive(session);
    Ok(())
  }

  /// 选定上游：恒为 app 容器（单 upstream，不做 LB）。
  async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
    Ok(Box::new(HttpPeer::new(UPSTREAM_ADDR, false, UPSTREAM_SNI.into())))
  }

  /// 上游请求改写：补 X-Forwarded-* 让 app 端 cookie/日志看到真实客户端 + scheme。
  ///
  /// Phase 8.3 修正：
  /// - `X-Forwarded-For` 由 `append` 改 `insert`：客户端伪造的同名头会被覆盖而非保留，
  ///   避免日志 / app 限流按伪造 IP 分桶
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

    if let Some(ip) = session.client_addr().and_then(forwarded_ip) {
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

  /// pingora 0.8.0 的默认实现在复用连接上出错时连 POST 也重试；照 0.9.0 的默认策略只重试幂等方法。
  fn error_while_proxy(
    &self,
    peer: &HttpPeer,
    session: &mut Session,
    e: Box<pingora_core::Error>,
    _ctx: &mut (),
    client_reused: bool,
  ) -> Box<pingora_core::Error> {
    let mut e = e.more_context(format!("Peer: {peer}"));
    if !session.req_header().method.is_idempotent() || session.as_ref().retry_buffer_truncated() {
      e.set_retry(false);
    } else {
      e.retry.decide_reuse(client_reused);
    }
    e
  }

  /// 响应改写：补安全头，见 [`apply_security_headers`]。
  async fn response_filter(
    &self,
    _session: &mut Session,
    upstream_response: &mut ResponseHeader,
    _ctx: &mut (),
  ) -> Result<()> {
    apply_security_headers(upstream_response);
    Ok(())
  }
}

/// 写给 app 的客户端地址只要 IP：带端口时每条 TCP 连接都不一样，app 按它分桶限流就等于没限。
fn forwarded_ip(addr: &pingora_core::protocols::l4::socket::SocketAddr) -> Option<String> {
  Some(addr.as_inet()?.ip().to_string())
}

/// 安全响应头：
/// - HSTS 与 `Server` 由 gateway 负责（TLS 在这里终止），总是覆盖。
/// - 通用头（nosniff / X-Frame-Options / Referrer-Policy）应用也会下发；
///   只在缺失时补，保留应用按路由给出的值。
/// - **CSP 不碰**（SEC-10）：唯一定义在 `crates/app/src/server/security.rs`，
///   gateway 覆盖会丢掉 `'wasm-unsafe-eval'` 与内联启动脚本许可，hydration 全挂。
fn apply_security_headers(resp: &mut ResponseHeader) {
  resp.insert_header("Strict-Transport-Security", HSTS_VALUE).ok();
  // Server header 减少版本指纹
  resp.insert_header("Server", "rie-gateway").ok();
  for (name, value) in [
    ("X-Content-Type-Options", "nosniff"),
    ("X-Frame-Options", "DENY"),
    ("Referrer-Policy", "strict-origin-when-cross-origin"),
  ] {
    if !resp.headers.contains_key(name) {
      resp.insert_header(name, value).ok();
    }
  }
}

/// 80 端口专用：所有请求 301 到同 host 的 https 版本。
struct HttpToHttps;

#[async_trait]
impl ProxyHttp for HttpToHttps {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  async fn request_filter(&self, session: &mut Session, _ctx: &mut ()) -> Result<bool> {
    cap_keepalive(session);
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

  async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
    // 永远走不到这里：request_filter 已 Ok(true) 短路。
    Err(pingora_core::Error::new_str("unreachable: redirect short-circuit returned"))
  }
}

fn main() {
  env_logger::init();

  // 默认宽限期 300 秒，docker 10 秒后就 SIGKILL；在那之前自己退出
  let conf = ServerConf {
    grace_period_seconds: Some(5),
    graceful_shutdown_timeout_seconds: Some(1),
    ..Default::default()
  };
  let mut server = Server::new_with_opt_and_conf(None::<Opt>, conf);
  server.bootstrap();

  // 443: TLS 终止 + 反代到 app
  let mut proxy = http_proxy_service(&server.configuration, AppGateway);
  let cert = std::env::var("TLS_CERT_PATH").expect("TLS_CERT_PATH 未配置：指向 fullchain.pem 路径");
  let key = std::env::var("TLS_KEY_PATH").expect("TLS_KEY_PATH 未配置：指向 privkey.pem 路径");
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
    "rie-gateway: HTTPS={} (TLS={}), HTTP={} → 301 https; upstream={}",
    HTTPS_BIND,
    cert,
    HTTP_BIND,
    UPSTREAM_ADDR
  );
  server.run_forever();
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn forwarded_ip_drops_port() {
    let v4 =
      pingora_core::protocols::l4::socket::SocketAddr::Inet("203.0.113.7:51234".parse().unwrap());
    assert_eq!(forwarded_ip(&v4).as_deref(), Some("203.0.113.7"));
    let v6 =
      pingora_core::protocols::l4::socket::SocketAddr::Inet("[2001:db8::1]:443".parse().unwrap());
    assert_eq!(forwarded_ip(&v6).as_deref(), Some("2001:db8::1"));
  }

  fn header<'a>(resp: &'a ResponseHeader, name: &str) -> Option<&'a str> {
    resp.headers.get(name).and_then(|v| v.to_str().ok())
  }

  /// SEC-10：CSP 只由应用决定，gateway 原样转发（不覆盖、不补）。
  #[test]
  fn csp_from_app_passes_through_untouched() {
    let app_csp = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'";
    let mut resp = ResponseHeader::build(200, None).unwrap();
    resp.insert_header("Content-Security-Policy", app_csp).unwrap();
    apply_security_headers(&mut resp);
    assert_eq!(header(&resp, "content-security-policy"), Some(app_csp));
    assert_eq!(resp.headers.get_all("content-security-policy").iter().count(), 1);

    let mut bare = ResponseHeader::build(200, None).unwrap();
    apply_security_headers(&mut bare);
    assert_eq!(header(&bare, "content-security-policy"), None);
  }

  /// 应用已下发的通用安全头保留应用的值；缺失时 gateway 补默认值。HSTS 由 gateway 负责。
  #[test]
  fn generic_headers_fill_only_when_missing() {
    let mut resp = ResponseHeader::build(200, None).unwrap();
    resp.insert_header("X-Frame-Options", "SAMEORIGIN").unwrap();
    apply_security_headers(&mut resp);
    assert_eq!(header(&resp, "x-frame-options"), Some("SAMEORIGIN"));
    assert_eq!(header(&resp, "x-content-type-options"), Some("nosniff"));
    assert_eq!(header(&resp, "referrer-policy"), Some("strict-origin-when-cross-origin"));
    assert_eq!(header(&resp, "strict-transport-security"), Some(HSTS_VALUE));
  }
}
