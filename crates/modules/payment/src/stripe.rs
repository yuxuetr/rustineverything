//! Stripe 集成（server-only，feature = "stripe"；阶段 I，docs/PAYMENT_SPEC.md §15）。
//!
//! 用托管收银台 Checkout Session（DI1）：卡、Apple Pay、Google Pay 与各国本地
//! 支付方式由 Stripe 后台配置决定，本站不接触卡号。
//!
//! - 下单：`POST /v1/checkout/sessions`（form）→ 跳转 `url`；单号同时写进
//!   `client_reference_id`、session 与 PaymentIntent 的 metadata。
//! - 回调：`Stripe-Signature` 验签（[`verify_signature`]，宿主侧）后交
//!   [`StripeProvider::parse_notify`]。
//! - 查单：按 metadata 搜索 PaymentIntent（DI5）。退款：按 PaymentIntent id。
//!
//! ⚠️ 上线前需用测试模式密钥端到端验证（Todos I6）。

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::crypto::env_nonempty;
use crate::{
  NotifyPayload, OrderCall, OrderRequest, PayAction, PayError, PayRequest, PaymentEvent,
  PaymentProvider, RefundRequest, RefundResult, EVENT_STATUS_SUCCESS,
};

const API_BASE: &str = "https://api.stripe.com";
/// 固定 API 版本，响应结构不随账户默认版本漂移（DI8）。
const API_VERSION: &str = "2024-06-20";
/// 回调时间戳允许的偏差（秒），与微信一致。
pub const SIGNATURE_TOLERANCE_SECS: i64 = 300;

pub struct StripeConfig {
  secret_key: String,
  webhook_secret: String,
  /// 站点公网根地址（`PAY_NOTIFY_BASE`），拼 success / cancel URL。
  pub return_base: String,
}

/// 加载配置；任一项缺失 → None（Stripe 下单将报「未配置」）。
pub fn config() -> Option<StripeConfig> {
  Some(StripeConfig {
    secret_key: env_nonempty("STRIPE_SECRET_KEY")?,
    webhook_secret: env_nonempty("STRIPE_WEBHOOK_SECRET")?,
    return_base: env_nonempty("PAY_NOTIFY_BASE")?.trim_end_matches('/').to_string(),
  })
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
  if !s.len().is_multiple_of(2) {
    return None;
  }
  (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// 校验 `Stripe-Signature` 头（`t=…,v1=…[,v1=…]`）：HMAC-SHA256(`{t}.{body}`)，
/// 常数时间比较；任一 `v1` 匹配即通过（密钥轮换期 Stripe 会给多个）。
/// `now_secs` 由调用方传入，便于单测。
pub fn verify_signature(webhook_secret: &str, header: &str, body: &str, now_secs: i64) -> bool {
  let mut timestamp = None;
  let mut signatures = Vec::new();
  for part in header.split(',') {
    match part.trim().split_once('=') {
      Some(("t", v)) => timestamp = v.parse::<i64>().ok(),
      Some(("v1", v)) => signatures.push(v),
      _ => {}
    }
  }
  let Some(ts) = timestamp else {
    return false;
  };
  if (now_secs - ts).abs() > SIGNATURE_TOLERANCE_SECS {
    return false;
  }
  signatures.into_iter().filter_map(hex_decode).any(|sig| {
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes()) else {
      return false;
    };
    mac.update(format!("{ts}.{body}").as_bytes());
    mac.verify_slice(&sig).is_ok()
  })
}

/// 用配置里的 webhook secret 校验回调（与 alipay / wechat 的 `verify_notify` 同形）。
pub fn verify_notify(cfg: &StripeConfig, header: &str, body: &str, now_secs: i64) -> bool {
  verify_signature(&cfg.webhook_secret, header, body, now_secs)
}

pub struct StripeProvider {
  cfg: StripeConfig,
}

impl StripeProvider {
  pub fn new(cfg: StripeConfig) -> Self {
    Self { cfg }
  }

  /// 从环境变量加载；未配置 → `PayError::Unconfigured`。
  pub fn from_env() -> Result<Self, PayError> {
    config()
      .map(Self::new)
      .ok_or_else(|| PayError::Unconfigured("缺 STRIPE_* 环境变量".to_string()))
  }

  pub fn config(&self) -> &StripeConfig {
    &self.cfg
  }

  fn request(&self, method: &str, url: String, idempotency_key: Option<&str>) -> PayRequest {
    let mut headers = vec![
      ("Authorization".to_string(), format!("Bearer {}", self.cfg.secret_key)),
      ("Stripe-Version".to_string(), API_VERSION.to_string()),
    ];
    if let Some(key) = idempotency_key {
      headers.push(("Idempotency-Key".to_string(), key.to_string()));
    }
    PayRequest {
      url,
      method: method.to_string(),
      headers,
      body: String::new(),
      body_is_form: method == "POST",
      form: vec![],
      sign_payload: None,
    }
  }
}

/// 解析响应 JSON；Stripe 的错误响应是 `{"error":{"message":…}}`。
fn parse_body(body: &str, action: &str) -> Result<serde_json::Value, PayError> {
  let json: serde_json::Value = serde_json::from_str(body)
    .map_err(|e| PayError::Parse(format!("解析 Stripe 响应失败: {e}")))?;
  if let Some(err) = json.get("error") {
    let msg = err["message"].as_str().unwrap_or("未知错误");
    return Err(PayError::Gateway(format!("Stripe {action}失败: {msg}")));
  }
  Ok(json)
}

fn str_field(v: &serde_json::Value) -> Option<String> {
  v.as_str().filter(|s| !s.is_empty()).map(|s| s.to_string())
}

impl PaymentProvider for StripeProvider {
  fn name(&self) -> &'static str {
    "stripe"
  }

  fn build_order(&self, req: &OrderRequest) -> Result<OrderCall, PayError> {
    let base = &self.cfg.return_base;
    let otn = &req.out_trade_no;
    let mut pr =
      self.request("POST", format!("{API_BASE}/v1/checkout/sessions"), Some(otn.as_str()));
    pr.form = vec![
      ("mode".into(), "payment".into()),
      ("locale".into(), "auto".into()),
      ("client_reference_id".into(), otn.clone()),
      ("metadata[out_trade_no]".into(), otn.clone()),
      ("payment_intent_data[metadata][out_trade_no]".into(), otn.clone()),
      ("line_items[0][quantity]".into(), "1".into()),
      ("line_items[0][price_data][currency]".into(), req.currency.to_lowercase()),
      ("line_items[0][price_data][unit_amount]".into(), req.amount_cents.to_string()),
      ("line_items[0][price_data][product_data][name]".into(), req.subject.clone()),
      ("success_url".into(), format!("{base}/me/orders")),
      ("cancel_url".into(), format!("{base}/course")),
    ];
    Ok(OrderCall::Http(pr))
  }

  fn parse_order_response(&self, _req: &OrderRequest, body: &str) -> Result<PayAction, PayError> {
    let json = parse_body(body, "下单")?;
    str_field(&json["url"])
      .map(PayAction::PayUrl)
      .ok_or_else(|| PayError::Gateway("Stripe 未返回收银台 url".to_string()))
  }

  /// 已验签的 webhook 事件 → 中立事件。只有 `checkout.session.completed` /
  /// `checkout.session.async_payment_succeeded` 且 `payment_status == "paid"`
  /// 算成功（银行转账等异步方式 completed 时还是 unpaid）。
  fn parse_notify(&self, payload: &NotifyPayload) -> Result<PaymentEvent, PayError> {
    let NotifyPayload::Json(event) = payload else {
      return Err(PayError::Parse("stripe 回调应为 JSON".to_string()));
    };
    let kind = event["type"].as_str().unwrap_or("");
    let session = &event["data"]["object"];
    let out_trade_no = str_field(&session["client_reference_id"])
      .or_else(|| str_field(&session["metadata"]["out_trade_no"]))
      .ok_or_else(|| PayError::Parse("回调缺 client_reference_id".to_string()))?;
    let payment_status = session["payment_status"].as_str().unwrap_or("");
    let paid =
      matches!(kind, "checkout.session.completed" | "checkout.session.async_payment_succeeded")
        && payment_status == "paid";
    Ok(PaymentEvent {
      provider: "stripe".to_string(),
      out_trade_no,
      amount_cents: session["amount_total"].as_i64().unwrap_or(-1),
      status: if paid {
        EVENT_STATUS_SUCCESS.to_string()
      } else {
        format!("{kind}:{payment_status}")
      },
      txn_id: str_field(&session["payment_intent"]),
    })
  }

  /// 按 metadata 搜索 PaymentIntent（DI5）。搜索约有 1 分钟延迟，对账最小单龄
  /// 300s，不受影响。
  fn build_query(&self, out_trade_no: &str) -> Result<PayRequest, PayError> {
    let query = format!("metadata['out_trade_no']:'{out_trade_no}'");
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    Ok(self.request("GET", format!("{API_BASE}/v1/payment_intents/search?query={encoded}"), None))
  }

  /// 有 `succeeded` 的 PaymentIntent → 成功；没有（用户没完成支付）→ 非成功
  /// 事件，超出支付窗口后由对账关单。
  fn parse_query(&self, body: &str) -> Result<PaymentEvent, PayError> {
    let json = parse_body(body, "查单")?;
    let intents = json["data"].as_array().cloned().unwrap_or_default();
    let Some(intent) = intents.iter().find(|pi| pi["status"] == "succeeded").or(intents.first())
    else {
      return Ok(PaymentEvent {
        provider: "stripe".to_string(),
        out_trade_no: String::new(),
        amount_cents: -1,
        status: "not_found".to_string(),
        txn_id: None,
      });
    };
    let raw = intent["status"].as_str().unwrap_or("");
    Ok(PaymentEvent {
      provider: "stripe".to_string(),
      out_trade_no: str_field(&intent["metadata"]["out_trade_no"]).unwrap_or_default(),
      amount_cents: intent["amount_received"].as_i64().unwrap_or(-1),
      status: if raw == "succeeded" { EVENT_STATUS_SUCCESS.to_string() } else { raw.to_string() },
      txn_id: str_field(&intent["id"]),
    })
  }

  /// 按 PaymentIntent id 退款（回调时存进 `orders.provider_txn`）；
  /// 我方退款单号作 `Idempotency-Key`。
  fn build_refund(&self, req: &RefundRequest) -> Result<PayRequest, PayError> {
    let intent = req.txn_id.as_deref().filter(|s| !s.is_empty()).ok_or_else(|| {
      PayError::Gateway(format!("订单 {} 缺 PaymentIntent id，无法退款", req.out_trade_no))
    })?;
    let mut pr = self.request("POST", format!("{API_BASE}/v1/refunds"), Some(&req.refund_no));
    pr.form = vec![
      ("payment_intent".into(), intent.to_string()),
      ("amount".into(), req.amount_cents.to_string()),
      ("metadata[out_trade_no]".into(), req.out_trade_no.clone()),
    ];
    Ok(pr)
  }

  fn parse_refund(&self, body: &str) -> Result<RefundResult, PayError> {
    let json = parse_body(body, "退款")?;
    let status = match json["status"].as_str().unwrap_or("") {
      "succeeded" => "success".to_string(),
      "pending" => "processing".to_string(),
      other => other.to_string(),
    };
    Ok(RefundResult { status, refund_id: str_field(&json["id"]) })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const SECRET: &str = "whsec_test_secret";
  const BODY: &str = r#"{"id":"evt_1","type":"checkout.session.completed"}"#;
  /// Python 独立算出：`hmac.new(SECRET, b"1700000000." + BODY, sha256).hexdigest()`。
  const SIG: &str = "a13d4b1f9003bc2fb6c06224e878b6056185eaa50d40e1e7dc05517f8af41d7b";
  const TS: i64 = 1_700_000_000;

  fn provider() -> StripeProvider {
    StripeProvider::new(StripeConfig {
      secret_key: "sk_test_x".into(),
      webhook_secret: SECRET.into(),
      return_base: "https://example.com".into(),
    })
  }

  fn form_get<'a>(pr: &'a PayRequest, key: &str) -> Option<&'a str> {
    pr.form.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
  }

  fn header_get<'a>(pr: &'a PayRequest, key: &str) -> Option<&'a str> {
    pr.headers.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
  }

  #[test]
  fn signature_accepts_valid_and_rotated() {
    let header = format!("t={TS},v1={SIG}");
    assert!(verify_signature(SECRET, &header, BODY, TS + 10));
    // 密钥轮换：多个 v1，任一匹配即可；v0 被忽略
    let rotated = format!("t={TS},v1={},v1={SIG},v0=ff", "00".repeat(32));
    assert!(verify_signature(SECRET, &rotated, BODY, TS));
  }

  #[test]
  fn signature_rejects_tampering_staleness_and_garbage() {
    let header = format!("t={TS},v1={SIG}");
    assert!(!verify_signature(SECRET, &header, &BODY.replace("evt_1", "evt_2"), TS));
    assert!(!verify_signature("whsec_other", &header, BODY, TS));
    assert!(!verify_signature(SECRET, &header, BODY, TS + SIGNATURE_TOLERANCE_SECS + 1));
    assert!(!verify_signature(SECRET, &header, BODY, TS - SIGNATURE_TOLERANCE_SECS - 1));
    assert!(!verify_signature(SECRET, &format!("t={},v1={SIG}", TS + 1), BODY, TS));
    assert!(!verify_signature(SECRET, &format!("v1={SIG}"), BODY, TS));
    assert!(!verify_signature(SECRET, &format!("t={TS},v1=zz"), BODY, TS));
    assert!(!verify_signature(SECRET, "", BODY, TS));
  }

  #[test]
  fn build_order_creates_checkout_session() {
    let req = OrderRequest {
      out_trade_no: "RIE1".into(),
      subject: "Rust 基础".into(),
      amount_cents: 1499,
      currency: "USD".into(),
      scene: "page".into(),
      client_ip: None,
    };
    let OrderCall::Http(pr) = provider().build_order(&req).unwrap() else {
      panic!("Stripe 下单需要 HTTP");
    };
    assert_eq!(pr.url, "https://api.stripe.com/v1/checkout/sessions");
    assert!(pr.body_is_form);
    assert_eq!(header_get(&pr, "Authorization"), Some("Bearer sk_test_x"));
    assert_eq!(header_get(&pr, "Idempotency-Key"), Some("RIE1"));
    assert_eq!(header_get(&pr, "Stripe-Version"), Some(API_VERSION));
    assert_eq!(form_get(&pr, "line_items[0][price_data][currency]"), Some("usd"));
    assert_eq!(form_get(&pr, "line_items[0][price_data][unit_amount]"), Some("1499"));
    assert_eq!(form_get(&pr, "client_reference_id"), Some("RIE1"));
    assert_eq!(form_get(&pr, "payment_intent_data[metadata][out_trade_no]"), Some("RIE1"));
    assert_eq!(form_get(&pr, "success_url"), Some("https://example.com/me/orders"));
  }

  #[test]
  fn parse_order_response_url_or_error() {
    let req = OrderRequest {
      out_trade_no: "RIE1".into(),
      subject: "s".into(),
      amount_cents: 1,
      currency: "USD".into(),
      scene: "page".into(),
      client_ip: None,
    };
    let p = provider();
    assert_eq!(
      p.parse_order_response(
        &req,
        r#"{"id":"cs_1","url":"https://checkout.stripe.com/c/pay/cs_1"}"#
      ),
      Ok(PayAction::PayUrl("https://checkout.stripe.com/c/pay/cs_1".into()))
    );
    let err = p.parse_order_response(&req, r#"{"error":{"message":"Invalid API Key"}}"#);
    assert!(matches!(err, Err(PayError::Gateway(m)) if m.contains("Invalid API Key")));
  }

  fn session_event(kind: &str, payment_status: &str) -> NotifyPayload {
    NotifyPayload::Json(serde_json::json!({
      "type": kind,
      "data": { "object": {
        "object": "checkout.session",
        "client_reference_id": "RIE1",
        "amount_total": 1499,
        "currency": "usd",
        "payment_status": payment_status,
        "payment_intent": "pi_1",
      }},
    }))
  }

  #[test]
  fn parse_notify_success_only_when_paid() {
    let p = provider();
    let ev = p.parse_notify(&session_event("checkout.session.completed", "paid")).unwrap();
    assert!(ev.is_success());
    assert_eq!((ev.out_trade_no.as_str(), ev.amount_cents), ("RIE1", 1499));
    assert_eq!(ev.txn_id.as_deref(), Some("pi_1"));
    assert!(p
      .parse_notify(&session_event("checkout.session.async_payment_succeeded", "paid"))
      .unwrap()
      .is_success());
    // 异步方式 completed 时还没到账
    assert!(!p
      .parse_notify(&session_event("checkout.session.completed", "unpaid"))
      .unwrap()
      .is_success());
    assert!(!p
      .parse_notify(&session_event("checkout.session.expired", "unpaid"))
      .unwrap()
      .is_success());
    assert!(p.parse_notify(&NotifyPayload::Json(serde_json::json!({"type": "x"}))).is_err());
  }

  #[test]
  fn query_searches_by_metadata() {
    let pr = provider().build_query("RIE1").unwrap();
    assert_eq!(pr.method, "GET");
    assert!(!pr.body_is_form);
    assert_eq!(
      pr.url,
      "https://api.stripe.com/v1/payment_intents/search?query=metadata%5B%27out_trade_no%27%5D%3A%27RIE1%27"
    );
  }

  #[test]
  fn parse_query_found_pending_and_missing() {
    let p = provider();
    let found = r#"{"data":[
      {"id":"pi_0","status":"canceled","amount_received":0,"metadata":{"out_trade_no":"RIE1"}},
      {"id":"pi_1","status":"succeeded","amount_received":1499,"metadata":{"out_trade_no":"RIE1"}}
    ]}"#;
    let ev = p.parse_query(found).unwrap();
    assert!(ev.is_success());
    assert_eq!((ev.amount_cents, ev.txn_id.as_deref()), (1499, Some("pi_1")));
    let pending = r#"{"data":[{"id":"pi_2","status":"requires_payment_method","metadata":{}}]}"#;
    assert_eq!(p.parse_query(pending).unwrap().status, "requires_payment_method");
    let missing = p.parse_query(r#"{"data":[]}"#).unwrap();
    assert!(!missing.is_success());
    assert_eq!(missing.status, "not_found");
    assert!(matches!(p.parse_query(r#"{"error":{"message":"x"}}"#), Err(PayError::Gateway(_))));
  }

  #[test]
  fn refund_by_payment_intent() {
    let p = provider();
    let mut req = RefundRequest {
      out_trade_no: "RIE1".into(),
      refund_no: "RIE1R1".into(),
      amount_cents: 1499,
      total_cents: 1499,
      reason: "r".into(),
      txn_id: Some("pi_1".into()),
    };
    let pr = p.build_refund(&req).unwrap();
    assert_eq!(pr.url, "https://api.stripe.com/v1/refunds");
    assert_eq!(form_get(&pr, "payment_intent"), Some("pi_1"));
    assert_eq!(header_get(&pr, "Idempotency-Key"), Some("RIE1R1"));
    req.txn_id = None;
    assert!(p.build_refund(&req).is_err());

    assert_eq!(p.parse_refund(r#"{"id":"re_1","status":"succeeded"}"#).unwrap().status, "success");
    let pending = p.parse_refund(r#"{"id":"re_2","status":"pending"}"#).unwrap();
    assert!(pending.is_accepted());
    assert!(!p.parse_refund(r#"{"id":"re_3","status":"failed"}"#).unwrap().is_accepted());
  }
}
