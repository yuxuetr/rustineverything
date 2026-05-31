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

use async_trait::async_trait;
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

struct AppGateway;

#[async_trait]
impl ProxyHttp for AppGateway {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  /// 选定上游：恒为 app 容器（单 upstream，不做 LB）。
  async fn upstream_peer(
    &self,
    _session: &mut Session,
    _ctx: &mut (),
  ) -> Result<Box<HttpPeer>> {
    Ok(Box::new(HttpPeer::new(UPSTREAM_ADDR, false, UPSTREAM_SNI.into())))
  }

  /// 上游请求改写：补 X-Forwarded-* 让 app 端 cookie/日志看到真实客户端 + scheme。
  async fn upstream_request_filter(
    &self,
    session: &mut Session,
    req: &mut RequestHeader,
    _ctx: &mut (),
  ) -> Result<()> {
    req.insert_header("X-Forwarded-Proto", "https").ok();
    if let Some(addr) = session.client_addr() {
      let ip = addr.to_string();
      req.insert_header("X-Real-IP", ip.clone()).ok();
      req.append_header("X-Forwarded-For", ip).ok();
    }
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
    "rie-gateway: HTTPS={} (TLS={}), HTTP={} → 301 https; upstream={}",
    HTTPS_BIND, cert, HTTP_BIND, UPSTREAM_ADDR
  );
  server.run_forever();
}
