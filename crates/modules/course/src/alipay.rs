//! 支付宝集成（M5b，server-only）。
//!
//! 覆盖：电脑网站 `alipay.trade.page.pay` / 手机网站 `alipay.trade.wap.pay`
//! （均为「签名后跳转」，下单时无需访问支付宝服务器）+ 扫码
//! `alipay.trade.precreate`（需 HTTP）+ 异步回调验签。详见 docs/PAYMENT_SPEC.md §4。
//!
//! 签名 RSA2（SHA256withRSA，PKCS#1 v1.5）。密钥从 .env 读取（base64-DER 或 PEM）。
//! **未配置时返回 None / 错误，不影响其它功能。**
//!
//! ⚠️ 上线前需用支付宝沙箱 + 真实商户密钥端到端验证（验签 / 金额 / 回调发货）。

use std::collections::{BTreeMap, HashMap};

use base64::Engine;

/// 支付宝配置（从环境变量加载；缺任一关键项即视为未配置）。
pub struct AlipayConfig {
  pub app_id: String,
  app_private_key_der: Vec<u8>,
  alipay_public_key_der: Vec<u8>,
  pub gateway: String,
  pub notify_url: String,
  pub return_url: String,
}

fn env_nonempty(key: &str) -> Option<String> {
  std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 把 .env 里的密钥（PEM 或裸 base64）解码为 DER 字节。支付宝与微信 v3 共用。
pub(crate) fn decode_key(raw: &str) -> Option<Vec<u8>> {
  let body: String = raw
    .lines()
    .filter(|l| !l.starts_with("-----"))
    .collect::<Vec<_>>()
    .join("")
    .split_whitespace()
    .collect();
  base64::engine::general_purpose::STANDARD.decode(body.as_bytes()).ok()
}

/// 加载配置；任一关键项缺失/无法解析 → None（支付宝下单将报「未配置」）。
pub fn config() -> Option<AlipayConfig> {
  let app_id = env_nonempty("ALIPAY_APP_ID")?;
  let app_private_key_der = decode_key(&env_nonempty("ALIPAY_APP_PRIVATE_KEY")?)?;
  let alipay_public_key_der = decode_key(&env_nonempty("ALIPAY_PUBLIC_KEY")?)?;
  let gateway = env_nonempty("ALIPAY_GATEWAY")
    .unwrap_or_else(|| "https://openapi.alipay.com/gateway.do".to_string());
  let base = env_nonempty("PAY_NOTIFY_BASE")?;
  let base = base.trim_end_matches('/');
  Some(AlipayConfig {
    app_id,
    app_private_key_der,
    alipay_public_key_der,
    gateway,
    notify_url: format!("{base}/api/pay/alipay/notify"),
    return_url: format!("{base}/course"),
  })
}

/// 排序后拼接非空参数：`k1=v1&k2=v2`（待签名串）。
fn canonical(params: &BTreeMap<String, String>) -> String {
  params
    .iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(k, v)| format!("{k}={v}"))
    .collect::<Vec<_>>()
    .join("&")
}

/// RSA2 签名 → base64。支付宝与微信 v3 共用。
pub(crate) fn rsa2_sign(private_key_der: &[u8], content: &str) -> Result<String, String> {
  use rsa::pkcs1::DecodeRsaPrivateKey;
  use rsa::pkcs1v15::SigningKey;
  use rsa::pkcs8::DecodePrivateKey;
  use rsa::sha2::Sha256;
  use rsa::signature::{SignatureEncoding, Signer};
  use rsa::RsaPrivateKey;

  let key = RsaPrivateKey::from_pkcs8_der(private_key_der)
    .or_else(|_| RsaPrivateKey::from_pkcs1_der(private_key_der))
    .map_err(|e| format!("应用私钥解析失败: {e}"))?;
  let signing_key = SigningKey::<Sha256>::new(key);
  let sig = signing_key.try_sign(content.as_bytes()).map_err(|e| format!("签名失败: {e}"))?;
  Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
}

/// RSA2 验签。支付宝与微信 v3 共用。
pub(crate) fn rsa2_verify(public_key_der: &[u8], content: &str, sign_b64: &str) -> bool {
  use rsa::pkcs1v15::{Signature, VerifyingKey};
  use rsa::pkcs8::DecodePublicKey;
  use rsa::sha2::Sha256;
  use rsa::signature::Verifier;
  use rsa::RsaPublicKey;

  let Ok(key) = RsaPublicKey::from_public_key_der(public_key_der) else {
    return false;
  };
  let vk = VerifyingKey::<Sha256>::new(key);
  let Ok(sig_bytes) = base64::engine::general_purpose::STANDARD.decode(sign_b64.as_bytes()) else {
    return false;
  };
  let Ok(sig) = Signature::try_from(sig_bytes.as_slice()) else {
    return false;
  };
  vk.verify(content.as_bytes(), &sig).is_ok()
}

/// 金额（分）→ 支付宝要求的「元.角分」字符串，如 9900 → "99.00"。
fn yuan(amount_cents: i64) -> String {
  format!("{}.{:02}", amount_cents / 100, amount_cents % 100)
}

fn now_ts() -> String {
  // 支付宝 timestamp 用北京时间（UTC+8）。
  (chrono::Utc::now() + chrono::Duration::hours(8)).format("%Y-%m-%d %H:%M:%S").to_string()
}

fn common_params(
  cfg: &AlipayConfig,
  method: &str,
  biz_content: String,
) -> BTreeMap<String, String> {
  let mut p = BTreeMap::new();
  p.insert("app_id".into(), cfg.app_id.clone());
  p.insert("method".into(), method.to_string());
  p.insert("format".into(), "JSON".into());
  p.insert("charset".into(), "utf-8".into());
  p.insert("sign_type".into(), "RSA2".into());
  p.insert("timestamp".into(), now_ts());
  p.insert("version".into(), "1.0".into());
  p.insert("notify_url".into(), cfg.notify_url.clone());
  p.insert("biz_content".into(), biz_content);
  p
}

/// 构造「电脑网站 / 手机网站」支付跳转 URL（签名后浏览器直接跳转，无需服务端调支付宝）。
pub fn build_pay_url(
  cfg: &AlipayConfig,
  scene: &str,
  out_trade_no: &str,
  subject: &str,
  amount_cents: i64,
) -> Result<String, String> {
  let (method, product_code) = match scene {
    "wap" => ("alipay.trade.wap.pay", "QUICK_WAP_WAY"),
    _ => ("alipay.trade.page.pay", "FAST_INSTANT_TRADE_PAY"),
  };
  let biz = serde_json::json!({
    "out_trade_no": out_trade_no,
    "total_amount": yuan(amount_cents),
    "subject": subject,
    "product_code": product_code,
  })
  .to_string();
  let mut params = common_params(cfg, method, biz);
  params.insert("return_url".into(), cfg.return_url.clone());

  let sign = rsa2_sign(&cfg.app_private_key_der, &canonical(&params))?;

  let mut url = url::Url::parse(&cfg.gateway).map_err(|e| format!("网关 URL 非法: {e}"))?;
  {
    let mut qp = url.query_pairs_mut();
    for (k, v) in &params {
      qp.append_pair(k, v);
    }
    qp.append_pair("sign", &sign);
  }
  Ok(url.to_string())
}

/// 扫码下单（`alipay.trade.precreate`）：需访问支付宝服务器，返回 `qr_code`。
pub async fn precreate(
  cfg: &AlipayConfig,
  out_trade_no: &str,
  subject: &str,
  amount_cents: i64,
) -> Result<String, String> {
  let biz = serde_json::json!({
    "out_trade_no": out_trade_no,
    "total_amount": yuan(amount_cents),
    "subject": subject,
  })
  .to_string();
  let mut params = common_params(cfg, "alipay.trade.precreate", biz);
  let sign = rsa2_sign(&cfg.app_private_key_der, &canonical(&params))?;
  params.insert("sign".into(), sign);

  let client = reqwest::Client::new();
  let resp = client
    .post(&cfg.gateway)
    .form(&params)
    .send()
    .await
    .map_err(|e| format!("请求支付宝失败: {e}"))?;
  let text = resp.text().await.map_err(|e| format!("读取支付宝响应失败: {e}"))?;
  let json: serde_json::Value =
    serde_json::from_str(&text).map_err(|e| format!("解析支付宝响应失败: {e}"))?;
  let node = &json["alipay_trade_precreate_response"];
  if node["code"].as_str() != Some("10000") {
    return Err(format!("支付宝下单失败: {}", node["sub_msg"].as_str().unwrap_or("未知")));
  }
  node["qr_code"].as_str().map(|s| s.to_string()).ok_or_else(|| "未返回二维码".to_string())
}

/// 验证异步回调签名：用支付宝公钥校验除 `sign` / `sign_type` 外的全部非空参数。
pub fn verify_notify(cfg: &AlipayConfig, params: &HashMap<String, String>) -> bool {
  let Some(sign) = params.get("sign") else {
    return false;
  };
  let mut sorted: BTreeMap<String, String> = BTreeMap::new();
  for (k, v) in params {
    if k != "sign" && k != "sign_type" && !v.is_empty() {
      sorted.insert(k.clone(), v.clone());
    }
  }
  rsa2_verify(&cfg.alipay_public_key_der, &canonical(&sorted), sign)
}

/// 回调金额（元字符串）是否与订单金额（分）一致。
pub fn amount_matches(total_amount: &str, order_cents: i64) -> bool {
  total_amount == yuan(order_cents)
}

#[cfg(test)]
mod tests {
  use super::*;

  // 固定测试密钥（仅供单测；非生产）。priv 为 PKCS#1 DER、pub 为 SPKI DER 的 base64。
  const TEST_PRIV_B64: &str = "MIIEowIBAAKCAQEAiOBBRxQpGkL87a/p3i3ZGs5Xyk0b8AYSrkIjpldOrdmgZm/xMDgfASts2ARMRmRMaLUgYGIaEr5xTW7kXgwRL8YAKKH/yl+rB0+SxPgUakiDshDSjnuqWr3jYpBGDQUacyQlNvvEZ4ogo2z+2km9SdgfwfQfulIfCeusfi+19osFLEb7hpfoeo3YoONPbFhdXsIxdaiQGlwBphjX+DwOJ6PPuK1qytirxBFC2VGqy4JmAK2H92FDCT4fBDniDDbbl/zPVoy+SGS+43LtexUa9Lyy0gDbChJ5HxexsFE4uw8HIMywOtalt//dRlYcKS5ttW+fzbTwbB8XZghz21pBYQIDAQABAoIBAA5LPQXrOQ+hB0DbKhUlvJJsEgbyXoSGXdUM2yQ34eON4o5QCmP6uGIq4sb8S+rd9ozIvYTTOd3TPYnUlsyrfe/7QXD82fWMYBP3X2Bqd9dRk085KoPuri+jvOdCIc6iRczYbXp8eFpHtnjanRK2uKnJhCeBEv8mLE+g6PaUjPAeFWWXk+HOL/L72jxgxjjjNQi3rosIlJ1LscJegiM54Izmr6GcjDC/4/uLyg+QOIhnzmSvVCRwmCx6bUvXlf8qXYaEFsH/okZtV7we9lnyY5P4DhOCSOk3QxzI9rTDKiMHSYJiN+uLUBSPJMJuiHIt/AUIM3z06uegYp3Vujj/L/ECgYEAvsGgMfwd4u++7ASnVCHB0ymX3FGRUVklPEZdWWDGccyaH3mHys6VOoTb7u1KdprpiS80lokTilZPK7/oUNfI8RTfJG56OM+Q010B/v5+uchUozRD0ABPuz0aQNR8OWG0N21F25ykl5BVjQzd0Jg/N3bjYuiDMh0ciiOmewDVvh8CgYEAt7DvvypX17b0IlC2kqUOA7O3mgOn8OHR0BHFM2rfQDw+olIkXE8AFf/tczeAPjDiOKqoCt/8wQmdo+IihIim5T4UpIteOAqSdIW95Lo0BarZZYOw6H6bh2tWois118Rt8U2lRFF4F2+pXcfvSP72wp/xNkTJUFFH5EFSjx6xEH8CgYEAnB0UwLOnteklpDzuwGDcIrfgi7PJrPy7B4hCr3oPDmU3IVkxs927rWe8It7aWRTQ2a/jZuuKLWYTZyeotjjTP9IoCMXNix78VK7CinC3P85ezi5g7SLEHeWUzcfYXpHCjrYEPQYGge/ixAvqoONooTjQQUsuy92dVMR2ZCY7x1sCgYAWyBrz2oyKdGZS2y/JgC78xo0+zLVHarpa09lhRx/pF4+tEgLwb9vS3qrUX03IaMelv4SX1K/EQS0L5j/hsBEC3XAx+Bb3XFhNm0ix1WYeTdIohOyr6QfhA6767eD/oZ0BEGAu2OvL/E1FFEbZBsYT3UJNOLq++1WvOWrD1UqggQKBgEFggCpLFh4ASFIiDDDTMi+re6Ay5x8hjmX/l51D+jF2+SG7SPO/2X+OM8gEtbS4le5I6FMhC7t3+KxWyeD7ig37pHUb0/U/k/Nc+TlxlFfrkleJ6nGRcc0j1wIhyo9/lSd0qjN9cJXPupb56WJBLynbl3SAhQGbaV8MZn2RAiQ+";
  const TEST_PUB_B64: &str = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAiOBBRxQpGkL87a/p3i3ZGs5Xyk0b8AYSrkIjpldOrdmgZm/xMDgfASts2ARMRmRMaLUgYGIaEr5xTW7kXgwRL8YAKKH/yl+rB0+SxPgUakiDshDSjnuqWr3jYpBGDQUacyQlNvvEZ4ogo2z+2km9SdgfwfQfulIfCeusfi+19osFLEb7hpfoeo3YoONPbFhdXsIxdaiQGlwBphjX+DwOJ6PPuK1qytirxBFC2VGqy4JmAK2H92FDCT4fBDniDDbbl/zPVoy+SGS+43LtexUa9Lyy0gDbChJ5HxexsFE4uw8HIMywOtalt//dRlYcKS5ttW+fzbTwbB8XZghz21pBYQIDAQAB";

  fn test_keys() -> (Vec<u8>, Vec<u8>) {
    (decode_key(TEST_PRIV_B64).unwrap(), decode_key(TEST_PUB_B64).unwrap())
  }

  #[test]
  fn sign_verify_roundtrip() {
    let (priv_der, pub_der) = test_keys();
    let content = "app_id=2021000000000000&method=alipay.trade.page.pay&out_trade_no=RIE123";
    let sig = rsa2_sign(&priv_der, content).expect("sign");
    assert!(rsa2_verify(&pub_der, content, &sig), "valid signature must verify");
    assert!(!rsa2_verify(&pub_der, "tampered", &sig), "tampered content must fail");
  }

  #[test]
  fn verify_notify_roundtrip_and_reject() {
    let (priv_der, pub_der) = test_keys();
    let cfg = AlipayConfig {
      app_id: "x".into(),
      app_private_key_der: priv_der.clone(),
      alipay_public_key_der: pub_der,
      gateway: "https://openapi.alipay.com/gateway.do".into(),
      notify_url: "https://e/n".into(),
      return_url: "https://e/r".into(),
    };
    // 构造一条「支付宝」回调：对非空参数排序拼接后签名。
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("out_trade_no".into(), "RIE123".into());
    params.insert("trade_status".into(), "TRADE_SUCCESS".into());
    params.insert("total_amount".into(), "99.00".into());
    params.insert("sign_type".into(), "RSA2".into());
    let mut sorted: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in &params {
      if k != "sign" && k != "sign_type" {
        sorted.insert(k.clone(), v.clone());
      }
    }
    let sign = rsa2_sign(&cfg.app_private_key_der, &canonical(&sorted)).unwrap();
    params.insert("sign".into(), sign);
    assert!(verify_notify(&cfg, &params), "genuine notify must verify");

    // 篡改金额 → 验签失败
    let mut tampered = params.clone();
    tampered.insert("total_amount".into(), "0.01".into());
    assert!(!verify_notify(&cfg, &tampered), "tampered notify must be rejected");
  }

  #[test]
  fn amount_formatting() {
    assert_eq!(yuan(9900), "99.00");
    assert_eq!(yuan(100), "1.00");
    assert_eq!(yuan(9), "0.09");
    assert!(amount_matches("99.00", 9900));
    assert!(!amount_matches("99.0", 9900));
  }
}
