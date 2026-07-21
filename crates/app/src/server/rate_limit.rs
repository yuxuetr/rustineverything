//! S2（风险 R4）：应用层 per-IP 限流中间件。
//!
//! gateway（Pingora + governor）是第一道防线，但它是**可选**部署组件；
//! app 裸跑公网时写接口（评论 / 话题 / 上传）与昂贵接口（搜索 / moderation）
//! 需要自带基础防护。本模块提供轻量 token-bucket 限流：
//!
//! - **作用范围**：仅 `/api/*`（server fn、auth、pay 回调都在该前缀下）。
//!   SSR 页面与静态资源不限流——页面加载会并发拉多个资源，限流误伤大。
//! - **分级**：`/api/auth/*` 与 `/api/pay/*` 用更严的 sensitive 桶（防爆破 /
//!   防回调滥用）；其余 `/api/*` 用宽松的默认桶。
//! - **键**：`x-forwarded-for` 首个 IP（gateway / 反代注入）→ `x-real-ip` →
//!   兜底共享桶 `"global"`。无反代直连时退化为全局限流，仍能防单源打满。
//!
//! ## 运维开关（env）
//! - `RATE_LIMIT_DISABLED=1`：整体禁用。
//! - `RATE_LIMIT_API_RPS` / `RATE_LIMIT_API_BURST`：默认桶（默认 20 rps / 60）。
//! - `RATE_LIMIT_SENSITIVE_RPS` / `RATE_LIMIT_SENSITIVE_BURST`：敏感桶
//!   （默认 5 rps / 15）。
//!
//! 实现为手写 token bucket（无新增依赖）；桶表有容量上限 + 过期剪枝，
//! 防止恶意伪造海量 XFF 值撑爆内存。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// 桶表条目数上限：超过即触发过期剪枝，剪完仍超限则拒绝新键（fail-closed
/// 到共享 global 桶），防御伪造海量 IP 的内存放大。
const MAX_TRACKED_KEYS: usize = 50_000;
/// 剪枝阈值：超过该时长无活动的桶视为过期。
const STALE_AFTER: Duration = Duration::from_secs(600);

/// 单 IP 的 token bucket。
struct Bucket {
  tokens: f64,
  last: Instant,
}

/// 一组按 key 分桶的限流器（纯逻辑，可注入时钟单测）。
pub struct TokenBucketMap {
  rps: f64,
  burst: f64,
  inner: Mutex<HashMap<String, Bucket>>,
}

impl TokenBucketMap {
  pub fn new(rps: f64, burst: f64) -> Self {
    // 防御非法配置：rps/burst 至少为 1，避免除零或永远拒绝。
    Self { rps: rps.max(0.01), burst: burst.max(1.0), inner: Mutex::new(HashMap::new()) }
  }

  /// 尝试取 1 个 token。`true` = 放行。
  pub fn try_acquire(&self, key: &str) -> bool {
    self.try_acquire_at(key, Instant::now())
  }

  /// 时钟可注入版本（单测用）。
  fn try_acquire_at(&self, key: &str, now: Instant) -> bool {
    // 锁中毒时直接取内层数据继续（限流器状态轻微失真优于整体拒绝服务）。
    let mut map = match self.inner.lock() {
      Ok(g) => g,
      Err(poisoned) => poisoned.into_inner(),
    };

    // 容量防御：先剪过期，再判断是否还能接纳新键。
    if !map.contains_key(key) && map.len() >= MAX_TRACKED_KEYS {
      map.retain(|_, b| now.duration_since(b.last) < STALE_AFTER);
      if map.len() >= MAX_TRACKED_KEYS {
        // 表仍然满：把该请求折叠进共享桶，避免无限增长。
        return self.acquire_in(&mut map, "__overflow__", now);
      }
    }

    self.acquire_in(&mut map, key, now)
  }

  fn acquire_in(&self, map: &mut HashMap<String, Bucket>, key: &str, now: Instant) -> bool {
    let bucket = map.entry(key.to_string()).or_insert(Bucket { tokens: self.burst, last: now });
    let dt = now.duration_since(bucket.last).as_secs_f64();
    bucket.tokens = (bucket.tokens + dt * self.rps).min(self.burst);
    bucket.last = now;
    if bucket.tokens >= 1.0 {
      bucket.tokens -= 1.0;
      true
    } else {
      false
    }
  }
}

/// 两级限流器：默认 `/api/*` 桶 + 敏感（auth/pay）桶。
pub struct RateLimiters {
  api: TokenBucketMap,
  sensitive: TokenBucketMap,
}

fn read_env_f64(key: &str, default: f64) -> f64 {
  std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

impl RateLimiters {
  pub fn from_env() -> Self {
    Self {
      api: TokenBucketMap::new(
        read_env_f64("RATE_LIMIT_API_RPS", 20.0),
        read_env_f64("RATE_LIMIT_API_BURST", 60.0),
      ),
      sensitive: TokenBucketMap::new(
        read_env_f64("RATE_LIMIT_SENSITIVE_RPS", 5.0),
        read_env_f64("RATE_LIMIT_SENSITIVE_BURST", 15.0),
      ),
    }
  }

  pub fn check(&self, sensitive: bool, key: &str) -> bool {
    if sensitive {
      self.sensitive.try_acquire(key)
    } else {
      self.api.try_acquire(key)
    }
  }
}

/// 路径是否在限流范围内；返回 `Some(is_sensitive)`，`None` = 不限流。
pub fn classify_path(path: &str) -> Option<bool> {
  if !path.starts_with("/api/") {
    return None;
  }
  Some(path.starts_with("/api/auth/") || path.starts_with("/api/pay/"))
}

/// 从请求头解析限流键。信任反代注入的 `x-forwarded-for` 首个 IP；
/// 无反代头时退化为共享桶 `"global"`。
pub fn client_key(req: &Request) -> String {
  if let Some(xff) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
    if let Some(first) = xff.split(',').next() {
      let ip = first.trim();
      if !ip.is_empty() {
        return ip.to_string();
      }
    }
  }
  if let Some(real) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
    let ip = real.trim();
    if !ip.is_empty() {
      return ip.to_string();
    }
  }
  "global".to_string()
}

/// 中间件本体。超限返回 `429 Too Many Requests` + `Retry-After: 1`。
pub async fn rate_limit_mw(req: Request, next: Next) -> Response {
  static STATE: std::sync::OnceLock<Option<RateLimiters>> = std::sync::OnceLock::new();
  let limiters = STATE.get_or_init(|| {
    if std::env::var("RATE_LIMIT_DISABLED").map(|v| v == "1").unwrap_or(false) {
      tracing::warn!("rate_limit: disabled via RATE_LIMIT_DISABLED=1");
      None
    } else {
      Some(RateLimiters::from_env())
    }
  });

  let Some(limiters) = limiters else {
    return next.run(req).await;
  };
  let Some(sensitive) = classify_path(req.uri().path()) else {
    return next.run(req).await;
  };

  let key = client_key(&req);
  if !limiters.check(sensitive, &key) {
    tracing::warn!(
      target: "rate_limit",
      key = %key,
      path = %req.uri().path(),
      sensitive,
      "rate limit exceeded"
    );
    let mut resp = (StatusCode::TOO_MANY_REQUESTS, "请求过于频繁，请稍后重试").into_response();
    resp.headers_mut().insert("retry-after", HeaderValue::from_static("1"));
    return resp;
  }
  next.run(req).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn burst_allows_then_blocks() {
    let map = TokenBucketMap::new(1.0, 3.0);
    let now = Instant::now();
    assert!(map.try_acquire_at("ip1", now));
    assert!(map.try_acquire_at("ip1", now));
    assert!(map.try_acquire_at("ip1", now));
    assert!(!map.try_acquire_at("ip1", now), "burst 用尽后同刻应拒绝");
  }

  #[test]
  fn refill_restores_tokens() {
    let map = TokenBucketMap::new(2.0, 2.0);
    let t0 = Instant::now();
    assert!(map.try_acquire_at("ip1", t0));
    assert!(map.try_acquire_at("ip1", t0));
    assert!(!map.try_acquire_at("ip1", t0));
    // 1 秒后按 2 rps 回填 2 个 token
    let t1 = t0 + Duration::from_secs(1);
    assert!(map.try_acquire_at("ip1", t1));
    assert!(map.try_acquire_at("ip1", t1));
    assert!(!map.try_acquire_at("ip1", t1));
  }

  #[test]
  fn keys_are_isolated() {
    let map = TokenBucketMap::new(1.0, 1.0);
    let now = Instant::now();
    assert!(map.try_acquire_at("a", now));
    assert!(!map.try_acquire_at("a", now));
    assert!(map.try_acquire_at("b", now), "不同 key 桶互相独立");
  }

  #[test]
  fn classify_scopes_only_api() {
    assert_eq!(classify_path("/blog/hello"), None);
    assert_eq!(classify_path("/assets/tailwind.css"), None);
    assert_eq!(classify_path("/api/comments/list"), Some(false));
    assert_eq!(classify_path("/api/auth/login/github"), Some(true));
    assert_eq!(classify_path("/api/pay/alipay/notify"), Some(true));
  }

  #[test]
  fn client_key_prefers_xff_first_hop() {
    let req = Request::builder()
      .uri("/api/x")
      .header("x-forwarded-for", "203.0.113.7, 10.0.0.1")
      .header("x-real-ip", "198.51.100.2")
      .body(axum::body::Body::empty())
      .expect("test request");
    assert_eq!(client_key(&req), "203.0.113.7");
  }

  #[test]
  fn client_key_falls_back_to_real_ip_then_global() {
    let req = Request::builder()
      .uri("/api/x")
      .header("x-real-ip", "198.51.100.2")
      .body(axum::body::Body::empty())
      .expect("test request");
    assert_eq!(client_key(&req), "198.51.100.2");

    let bare =
      Request::builder().uri("/api/x").body(axum::body::Body::empty()).expect("test request");
    assert_eq!(client_key(&bare), "global");
  }

  #[test]
  fn invalid_config_clamped() {
    // rps/burst 非法值不 panic、不永久拒绝
    let map = TokenBucketMap::new(0.0, 0.0);
    assert!(map.try_acquire_at("x", Instant::now()), "clamp 后至少允许 1 个 burst");
  }
}
