//! 微信支付 v3 集成（server-only，feature = "wechat"）。
//!
//! PM2：从 `module-course/src/wechat.rs` 纯搬移 + 适配为
//! [`WechatProvider`]（[`crate::PaymentProvider`] 实现），协议逻辑不变。
//!
//! 覆盖：Native（PC 扫码 → code_url）、H5（移动浏览器 → h5_url）、异步回调
//! 验签 + AEAD_AES_256_GCM 解密。详见 docs/PAYMENT_SPEC.md §5。
//!
//! - 请求签名：商户 API 私钥 RSA2 over `{method}\n{url}\n{ts}\n{nonce}\n{body}\n`。
//! - 回调验签：**公钥模式**——用配置的微信支付公钥校验 `{ts}\n{nonce}\n{body}\n`
//!   （省去平台证书下载/轮换；若用证书模式可后续扩展 GET /v3/certificates）。
//! - 回调解密：APIv3Key + resource.nonce + associated_data 解 ciphertext。
//!
//! ⚠️ 上线前需用真实商户号端到端验证（下单 / 验签 / 解密 / 发货）。

use crate::crypto::{aes256_gcm_decrypt, decode_key, env_nonempty, rsa2_sign, rsa2_verify};
use crate::{
  NotifyPayload, OrderCall, OrderRequest, PayAction, PayError, PayRequest, PaymentEvent,
  PaymentProvider, EVENT_STATUS_SUCCESS,
};

const API_BASE: &str = "https://api.mch.weixin.qq.com";

pub struct WechatConfig {
  pub mchid: String,
  pub appid: String,
  apiv3_key: Vec<u8>, // 32 字节
  mch_private_key_der: Vec<u8>,
  mch_serial: String,
  platform_public_key_der: Vec<u8>, // 公钥模式：微信支付公钥
  pub notify_url: String,
}

/// 加载配置；任一关键项缺失/非法 → None（微信下单将报「未配置」）。
pub fn config() -> Option<WechatConfig> {
  let mchid = env_nonempty("WECHAT_MCHID")?;
  let appid = env_nonempty("WECHAT_APP_ID")?;
  let apiv3_key = env_nonempty("WECHAT_API_V3_KEY")?.into_bytes();
  if apiv3_key.len() != 32 {
    return None; // APIv3Key 必须 32 字节
  }
  let mch_private_key_der = decode_key(&env_nonempty("WECHAT_MCH_PRIVATE_KEY")?)?;
  let mch_serial = env_nonempty("WECHAT_MCH_SERIAL_NO")?;
  let platform_public_key_der = decode_key(&env_nonempty("WECHAT_PLATFORM_PUBLIC_KEY")?)?;
  let base = env_nonempty("PAY_NOTIFY_BASE")?;
  let notify_url = format!("{}/api/pay/wechat/notify", base.trim_end_matches('/'));
  Some(WechatConfig {
    mchid,
    appid,
    apiv3_key,
    mch_private_key_der,
    mch_serial,
    platform_public_key_der,
    notify_url,
  })
}

fn nonce_str() -> String {
  use rand::Rng;
  let mut rng = rand::rng();
  (0..32).map(|_| char::from(b'a' + rng.random_range(0..26))).collect()
}

/// 构造 v3 `Authorization` 头：商户私钥 RSA2 签名。返回 (头值, 被签名原文)。
fn build_auth(
  cfg: &WechatConfig,
  method: &str,
  url_path: &str,
  body: &str,
) -> Result<(String, String), String> {
  let ts = chrono::Utc::now().timestamp().to_string();
  let nonce = nonce_str();
  let message = format!("{method}\n{url_path}\n{ts}\n{nonce}\n{body}\n");
  let signature = rsa2_sign(&cfg.mch_private_key_der, &message)?;
  let header = format!(
    "WECHATPAY2-SHA256-RSA2048 mchid=\"{}\",nonce_str=\"{}\",signature=\"{}\",timestamp=\"{}\",serial_no=\"{}\"",
    cfg.mchid, nonce, signature, ts, cfg.mch_serial
  );
  Ok((header, message))
}

/// 构造一个已签名的 v3 POST 请求（宿主发送）。
fn build_post_v3(cfg: &WechatConfig, url_path: &str, body: String) -> Result<PayRequest, String> {
  let (auth, sign_payload) = build_auth(cfg, "POST", url_path, &body)?;
  Ok(PayRequest {
    url: format!("{API_BASE}{url_path}"),
    method: "POST".into(),
    headers: vec![
      ("Authorization".into(), auth),
      ("Accept".into(), "application/json".into()),
      ("Content-Type".into(), "application/json".into()),
      ("User-Agent".into(), "rustineverything/1.0".into()),
    ],
    body,
    body_is_form: false,
    form: vec![],
    sign_payload: Some(sign_payload),
  })
}

/// 发送已构造的 v3 请求并解析 JSON（HTTP 层错误与业务错误统一为 String）。
async fn send_v3(req: &PayRequest) -> Result<serde_json::Value, String> {
  let client = reqwest::Client::new();
  let mut builder = client.post(&req.url);
  for (k, v) in &req.headers {
    builder = builder.header(k, v);
  }
  let resp =
    builder.body(req.body.clone()).send().await.map_err(|e| format!("请求微信支付失败: {e}"))?;
  let status = resp.status();
  let text = resp.text().await.map_err(|e| format!("读取微信响应失败: {e}"))?;
  let json: serde_json::Value =
    serde_json::from_str(&text).map_err(|e| format!("解析微信响应失败: {e}"))?;
  if !status.is_success() {
    return Err(format!("微信支付下单失败: {}", json["message"].as_str().unwrap_or("未知")));
  }
  Ok(json)
}

fn native_body(cfg: &WechatConfig, out_trade_no: &str, description: &str, amount: i64) -> String {
  serde_json::json!({
    "appid": cfg.appid,
    "mchid": cfg.mchid,
    "description": description,
    "out_trade_no": out_trade_no,
    "notify_url": cfg.notify_url,
    "amount": { "total": amount, "currency": "CNY" },
  })
  .to_string()
}

fn h5_body(
  cfg: &WechatConfig,
  out_trade_no: &str,
  description: &str,
  amount: i64,
  client_ip: &str,
) -> String {
  serde_json::json!({
    "appid": cfg.appid,
    "mchid": cfg.mchid,
    "description": description,
    "out_trade_no": out_trade_no,
    "notify_url": cfg.notify_url,
    "amount": { "total": amount, "currency": "CNY" },
    "scene_info": { "payer_client_ip": client_ip, "h5_info": { "type": "Wap" } },
  })
  .to_string()
}

/// Native 下单（PC 扫码）→ `code_url`（前端渲染二维码）。
pub async fn create_native(
  cfg: &WechatConfig,
  out_trade_no: &str,
  description: &str,
  amount_cents: i64,
) -> Result<String, String> {
  let body = native_body(cfg, out_trade_no, description, amount_cents);
  let req = build_post_v3(cfg, "/v3/pay/transactions/native", body)?;
  let json = send_v3(&req).await?;
  json["code_url"].as_str().map(|s| s.to_string()).ok_or_else(|| "未返回 code_url".to_string())
}

/// H5 下单（移动浏览器）→ `h5_url`（跳转）。
pub async fn create_h5(
  cfg: &WechatConfig,
  out_trade_no: &str,
  description: &str,
  amount_cents: i64,
  client_ip: &str,
) -> Result<String, String> {
  let body = h5_body(cfg, out_trade_no, description, amount_cents, client_ip);
  let req = build_post_v3(cfg, "/v3/pay/transactions/h5", body)?;
  let json = send_v3(&req).await?;
  json["h5_url"].as_str().map(|s| s.to_string()).ok_or_else(|| "未返回 h5_url".to_string())
}

/// 验证回调签名（公钥模式）：校验 `{timestamp}\n{nonce}\n{body}\n`。
pub fn verify_notify(
  cfg: &WechatConfig,
  timestamp: &str,
  nonce: &str,
  body: &str,
  sign_b64: &str,
) -> bool {
  let message = format!("{timestamp}\n{nonce}\n{body}\n");
  rsa2_verify(&cfg.platform_public_key_der, &message, sign_b64)
}

/// 解密回调 resource（AEAD_AES_256_GCM）→ 明文 JSON 字符串。
pub fn decrypt_resource(
  cfg: &WechatConfig,
  nonce: &str,
  associated_data: &str,
  ciphertext_b64: &str,
) -> Result<String, String> {
  aes256_gcm_decrypt(&cfg.apiv3_key, nonce, associated_data, ciphertext_b64)
}

// =============================================================
// PaymentProvider 适配
// =============================================================

/// 微信 Provider：持有已加载的配置；纯构造 / 解析，不做 IO。
pub struct WechatProvider {
  cfg: WechatConfig,
}

impl WechatProvider {
  pub fn new(cfg: WechatConfig) -> Self {
    Self { cfg }
  }

  /// 从环境变量加载；未配置 → `PayError::Unconfigured`。
  pub fn from_env() -> Result<Self, PayError> {
    config()
      .map(Self::new)
      .ok_or_else(|| PayError::Unconfigured("缺 WECHAT_* 环境变量".to_string()))
  }

  pub fn config(&self) -> &WechatConfig {
    &self.cfg
  }
}

impl PaymentProvider for WechatProvider {
  fn name(&self) -> &'static str {
    "wechat"
  }

  /// `h5` → H5 下单；其它 → Native 扫码（与旧 `create_order` 的 scene 分派
  /// 一致；H5 的付款 IP 缺省 `0.0.0.0` 占位同旧实现）。
  fn build_order(&self, req: &OrderRequest) -> Result<OrderCall, PayError> {
    let (path, body) = match req.scene.as_str() {
      "h5" => {
        let ip = req.client_ip.as_deref().unwrap_or("0.0.0.0");
        (
          "/v3/pay/transactions/h5",
          h5_body(&self.cfg, &req.out_trade_no, &req.subject, req.amount_cents, ip),
        )
      }
      _ => (
        "/v3/pay/transactions/native",
        native_body(&self.cfg, &req.out_trade_no, &req.subject, req.amount_cents),
      ),
    };
    build_post_v3(&self.cfg, path, body).map(OrderCall::Http).map_err(PayError::Sign)
  }

  fn parse_order_response(&self, req: &OrderRequest, body: &str) -> Result<PayAction, PayError> {
    let json: serde_json::Value =
      serde_json::from_str(body).map_err(|e| PayError::Parse(format!("解析微信响应失败: {e}")))?;
    // 业务错误响应含 code + message（成功响应只有 code_url / h5_url）——
    // 透传网关 message，与旧 post_v3 的错误文案一致。
    if let (Some(msg), true) = (json["message"].as_str(), json["code"].is_string()) {
      return Err(PayError::Gateway(format!("微信支付下单失败: {msg}")));
    }
    match req.scene.as_str() {
      "h5" => json["h5_url"]
        .as_str()
        .map(|s| PayAction::PayUrl(s.to_string()))
        .ok_or_else(|| PayError::Gateway("未返回 h5_url".to_string())),
      _ => json["code_url"]
        .as_str()
        .map(|s| PayAction::QrCode(s.to_string()))
        .ok_or_else(|| PayError::Gateway("未返回 code_url".to_string())),
    }
  }

  /// 已验签 + 解密的回调明文（JSON）→ 中立事件。`SUCCESS` 规范化为
  /// success；金额缺失时 `amount_cents = -1`（流水线金额核验必拒）。
  fn parse_notify(&self, payload: &NotifyPayload) -> Result<PaymentEvent, PayError> {
    let NotifyPayload::Json(tx) = payload else {
      return Err(PayError::Parse("wechat 回调应为解密后的 JSON".to_string()));
    };
    let out_trade_no = tx["out_trade_no"]
      .as_str()
      .filter(|s| !s.is_empty())
      .ok_or_else(|| PayError::Parse("回调缺 out_trade_no".to_string()))?
      .to_string();
    let raw_state = tx["trade_state"].as_str().unwrap_or("");
    let status =
      if raw_state == "SUCCESS" { EVENT_STATUS_SUCCESS.to_string() } else { raw_state.to_string() };
    Ok(PaymentEvent {
      provider: "wechat".to_string(),
      out_trade_no,
      amount_cents: tx["amount"]["total"].as_i64().unwrap_or(-1),
      status,
      txn_id: tx["transaction_id"].as_str().map(|s| s.to_string()),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use base64::Engine;

  // 与 alipay 测试同一对 RSA 测试密钥（priv PKCS#1 DER / pub SPKI DER 的 base64）。
  const TEST_PRIV_B64: &str = "MIIEowIBAAKCAQEAiOBBRxQpGkL87a/p3i3ZGs5Xyk0b8AYSrkIjpldOrdmgZm/xMDgfASts2ARMRmRMaLUgYGIaEr5xTW7kXgwRL8YAKKH/yl+rB0+SxPgUakiDshDSjnuqWr3jYpBGDQUacyQlNvvEZ4ogo2z+2km9SdgfwfQfulIfCeusfi+19osFLEb7hpfoeo3YoONPbFhdXsIxdaiQGlwBphjX+DwOJ6PPuK1qytirxBFC2VGqy4JmAK2H92FDCT4fBDniDDbbl/zPVoy+SGS+43LtexUa9Lyy0gDbChJ5HxexsFE4uw8HIMywOtalt//dRlYcKS5ttW+fzbTwbB8XZghz21pBYQIDAQABAoIBAA5LPQXrOQ+hB0DbKhUlvJJsEgbyXoSGXdUM2yQ34eON4o5QCmP6uGIq4sb8S+rd9ozIvYTTOd3TPYnUlsyrfe/7QXD82fWMYBP3X2Bqd9dRk085KoPuri+jvOdCIc6iRczYbXp8eFpHtnjanRK2uKnJhCeBEv8mLE+g6PaUjPAeFWWXk+HOL/L72jxgxjjjNQi3rosIlJ1LscJegiM54Izmr6GcjDC/4/uLyg+QOIhnzmSvVCRwmCx6bUvXlf8qXYaEFsH/okZtV7we9lnyY5P4DhOCSOk3QxzI9rTDKiMHSYJiN+uLUBSPJMJuiHIt/AUIM3z06uegYp3Vujj/L/ECgYEAvsGgMfwd4u++7ASnVCHB0ymX3FGRUVklPEZdWWDGccyaH3mHys6VOoTb7u1KdprpiS80lokTilZPK7/oUNfI8RTfJG56OM+Q010B/v5+uchUozRD0ABPuz0aQNR8OWG0N21F25ykl5BVjQzd0Jg/N3bjYuiDMh0ciiOmewDVvh8CgYEAt7DvvypX17b0IlC2kqUOA7O3mgOn8OHR0BHFM2rfQDw+olIkXE8AFf/tczeAPjDiOKqoCt/8wQmdo+IihIim5T4UpIteOAqSdIW95Lo0BarZZYOw6H6bh2tWois118Rt8U2lRFF4F2+pXcfvSP72wp/xNkTJUFFH5EFSjx6xEH8CgYEAnB0UwLOnteklpDzuwGDcIrfgi7PJrPy7B4hCr3oPDmU3IVkxs927rWe8It7aWRTQ2a/jZuuKLWYTZyeotjjTP9IoCMXNix78VK7CinC3P85ezi5g7SLEHeWUzcfYXpHCjrYEPQYGge/ixAvqoONooTjQQUsuy92dVMR2ZCY7x1sCgYAWyBrz2oyKdGZS2y/JgC78xo0+zLVHarpa09lhRx/pF4+tEgLwb9vS3qrUX03IaMelv4SX1K/EQS0L5j/hsBEC3XAx+Bb3XFhNm0ix1WYeTdIohOyr6QfhA6767eD/oZ0BEGAu2OvL/E1FFEbZBsYT3UJNOLq++1WvOWrD1UqggQKBgEFggCpLFh4ASFIiDDDTMi+re6Ay5x8hjmX/l51D+jF2+SG7SPO/2X+OM8gEtbS4le5I6FMhC7t3+KxWyeD7ig37pHUb0/U/k/Nc+TlxlFfrkleJ6nGRcc0j1wIhyo9/lSd0qjN9cJXPupb56WJBLynbl3SAhQGbaV8MZn2RAiQ+";
  const TEST_PUB_B64: &str = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAiOBBRxQpGkL87a/p3i3ZGs5Xyk0b8AYSrkIjpldOrdmgZm/xMDgfASts2ARMRmRMaLUgYGIaEr5xTW7kXgwRL8YAKKH/yl+rB0+SxPgUakiDshDSjnuqWr3jYpBGDQUacyQlNvvEZ4ogo2z+2km9SdgfwfQfulIfCeusfi+19osFLEb7hpfoeo3YoONPbFhdXsIxdaiQGlwBphjX+DwOJ6PPuK1qytirxBFC2VGqy4JmAK2H92FDCT4fBDniDDbbl/zPVoy+SGS+43LtexUa9Lyy0gDbChJ5HxexsFE4uw8HIMywOtalt//dRlYcKS5ttW+fzbTwbB8XZghz21pBYQIDAQAB";

  fn test_cfg(apiv3: Vec<u8>) -> WechatConfig {
    WechatConfig {
      mchid: "1900000000".into(),
      appid: "wxtest".into(),
      apiv3_key: apiv3,
      mch_private_key_der: decode_key(TEST_PRIV_B64).unwrap(),
      mch_serial: "ABCDEF".into(),
      platform_public_key_der: decode_key(TEST_PUB_B64).unwrap(),
      notify_url: "https://e/api/pay/wechat/notify".into(),
    }
  }

  #[test]
  fn notify_signature_roundtrip_and_reject() {
    let cfg = test_cfg(vec![0u8; 32]);
    let ts = "1700000000";
    let nonce = "abc123";
    let body = r#"{"id":"evt","resource":{}}"#;
    // 用「商户私钥」对回调串签名，再用「平台公钥」验——本测试两者同一对密钥。
    let message = format!("{ts}\n{nonce}\n{body}\n");
    let sign = rsa2_sign(&cfg.mch_private_key_der, &message).unwrap();
    assert!(verify_notify(&cfg, ts, nonce, body, &sign), "genuine notify must verify");
    assert!(!verify_notify(&cfg, ts, "tampered-nonce", body, &sign), "tampered must be rejected");
  }

  #[test]
  fn aes_gcm_decrypt_roundtrip() {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes256Gcm, Nonce};
    let key = b"0123456789abcdef0123456789abcdef".to_vec(); // 32B
    let cfg = test_cfg(key.clone());
    let nonce = "abcdefghijkl"; // 12B
    let aad = "transaction";
    let plaintext = r#"{"out_trade_no":"RIE1","trade_state":"SUCCESS","amount":{"total":9900}}"#;
    // 加密生成回调 ciphertext（含 GCM tag）。
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let ct = cipher
      .encrypt(
        Nonce::from_slice(nonce.as_bytes()),
        Payload { msg: plaintext.as_bytes(), aad: aad.as_bytes() },
      )
      .unwrap();
    let ct_b64 = base64::engine::general_purpose::STANDARD.encode(ct);
    let got = decrypt_resource(&cfg, nonce, aad, &ct_b64).unwrap();
    assert_eq!(got, plaintext);
    // 错误 AAD → 解密失败
    assert!(decrypt_resource(&cfg, nonce, "wrong", &ct_b64).is_err());
  }

  #[test]
  fn provider_build_order_scenes() {
    let p = WechatProvider::new(test_cfg(vec![0u8; 32]));
    let mut req = OrderRequest {
      out_trade_no: "RIE1".into(),
      subject: "课程".into(),
      amount_cents: 9900,
      currency: "CNY".into(),
      scene: "native".into(),
      client_ip: None,
    };
    match p.build_order(&req) {
      Ok(OrderCall::Http(pr)) => {
        assert!(pr.url.ends_with("/v3/pay/transactions/native"));
        assert!(pr
          .headers
          .iter()
          .any(|(k, v)| k == "Authorization" && v.starts_with("WECHATPAY2-SHA256-RSA2048")));
        assert!(pr.body.contains("\"out_trade_no\":\"RIE1\""));
        assert!(pr.sign_payload.is_some());
      }
      other => panic!("expected http native order, got {other:?}"),
    }
    req.scene = "h5".into();
    req.client_ip = Some("1.2.3.4".into());
    match p.build_order(&req) {
      Ok(OrderCall::Http(pr)) => {
        assert!(pr.url.ends_with("/v3/pay/transactions/h5"));
        assert!(pr.body.contains("\"payer_client_ip\":\"1.2.3.4\""));
      }
      other => panic!("expected http h5 order, got {other:?}"),
    }
  }

  #[test]
  fn provider_parse_order_response() {
    let p = WechatProvider::new(test_cfg(vec![0u8; 32]));
    let mut req = OrderRequest {
      out_trade_no: "RIE1".into(),
      subject: "课程".into(),
      amount_cents: 9900,
      currency: "CNY".into(),
      scene: "native".into(),
      client_ip: None,
    };
    assert_eq!(
      p.parse_order_response(&req, r#"{"code_url":"weixin://wxpay/x"}"#).unwrap(),
      PayAction::QrCode("weixin://wxpay/x".into())
    );
    req.scene = "h5".into();
    assert_eq!(
      p.parse_order_response(&req, r#"{"h5_url":"https://wx.gd/h5"}"#).unwrap(),
      PayAction::PayUrl("https://wx.gd/h5".into())
    );
    assert!(matches!(p.parse_order_response(&req, "{}"), Err(PayError::Gateway(_))));
  }

  #[test]
  fn provider_parse_notify_normalizes() {
    let p = WechatProvider::new(test_cfg(vec![0u8; 32]));
    let tx: serde_json::Value = serde_json::from_str(
      r#"{"out_trade_no":"RIE1","trade_state":"SUCCESS","amount":{"total":9900},"transaction_id":"wx001"}"#,
    )
    .unwrap();
    let ev = p.parse_notify(&NotifyPayload::Json(tx)).unwrap();
    assert!(ev.is_success());
    assert_eq!(ev.amount_cents, 9900);
    assert_eq!(ev.txn_id.as_deref(), Some("wx001"));

    // 非成功状态保留原始串；金额缺失 → -1。
    let tx: serde_json::Value =
      serde_json::from_str(r#"{"out_trade_no":"RIE1","trade_state":"NOTPAY"}"#).unwrap();
    let ev = p.parse_notify(&NotifyPayload::Json(tx)).unwrap();
    assert!(!ev.is_success());
    assert_eq!(ev.status, "NOTPAY");
    assert_eq!(ev.amount_cents, -1);

    // 缺 out_trade_no → Parse 错误。
    let tx: serde_json::Value = serde_json::from_str(r#"{"trade_state":"SUCCESS"}"#).unwrap();
    assert!(matches!(p.parse_notify(&NotifyPayload::Json(tx)), Err(PayError::Parse(_))));
  }
}
