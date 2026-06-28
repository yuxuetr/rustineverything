//! 微信支付 v3 集成（M5c，server-only）。
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

use base64::Engine;

use crate::alipay::{decode_key, rsa2_sign, rsa2_verify};

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

fn env_nonempty(key: &str) -> Option<String> {
  std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
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

/// 构造 v3 `Authorization` 头：商户私钥 RSA2 签名。
fn build_auth(
  cfg: &WechatConfig,
  method: &str,
  url_path: &str,
  body: &str,
) -> Result<String, String> {
  let ts = chrono::Utc::now().timestamp().to_string();
  let nonce = nonce_str();
  let message = format!("{method}\n{url_path}\n{ts}\n{nonce}\n{body}\n");
  let signature = rsa2_sign(&cfg.mch_private_key_der, &message)?;
  Ok(format!(
    "WECHATPAY2-SHA256-RSA2048 mchid=\"{}\",nonce_str=\"{}\",signature=\"{}\",timestamp=\"{}\",serial_no=\"{}\"",
    cfg.mchid, nonce, signature, ts, cfg.mch_serial
  ))
}

async fn post_v3(
  cfg: &WechatConfig,
  url_path: &str,
  body: String,
) -> Result<serde_json::Value, String> {
  let auth = build_auth(cfg, "POST", url_path, &body)?;
  let client = reqwest::Client::new();
  let resp = client
    .post(format!("{API_BASE}{url_path}"))
    .header("Authorization", auth)
    .header("Accept", "application/json")
    .header("Content-Type", "application/json")
    .header("User-Agent", "rustineverything/1.0")
    .body(body)
    .send()
    .await
    .map_err(|e| format!("请求微信支付失败: {e}"))?;
  let status = resp.status();
  let text = resp.text().await.map_err(|e| format!("读取微信响应失败: {e}"))?;
  let json: serde_json::Value =
    serde_json::from_str(&text).map_err(|e| format!("解析微信响应失败: {e}"))?;
  if !status.is_success() {
    return Err(format!("微信支付下单失败: {}", json["message"].as_str().unwrap_or("未知")));
  }
  Ok(json)
}

/// Native 下单（PC 扫码）→ `code_url`（前端渲染二维码）。
pub async fn create_native(
  cfg: &WechatConfig,
  out_trade_no: &str,
  description: &str,
  amount_cents: i64,
) -> Result<String, String> {
  let body = serde_json::json!({
    "appid": cfg.appid,
    "mchid": cfg.mchid,
    "description": description,
    "out_trade_no": out_trade_no,
    "notify_url": cfg.notify_url,
    "amount": { "total": amount_cents, "currency": "CNY" },
  })
  .to_string();
  let json = post_v3(cfg, "/v3/pay/transactions/native", body).await?;
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
  let body = serde_json::json!({
    "appid": cfg.appid,
    "mchid": cfg.mchid,
    "description": description,
    "out_trade_no": out_trade_no,
    "notify_url": cfg.notify_url,
    "amount": { "total": amount_cents, "currency": "CNY" },
    "scene_info": { "payer_client_ip": client_ip, "h5_info": { "type": "Wap" } },
  })
  .to_string();
  let json = post_v3(cfg, "/v3/pay/transactions/h5", body).await?;
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
  use aes_gcm::aead::{Aead, KeyInit, Payload};
  use aes_gcm::{Aes256Gcm, Nonce};

  let ct = base64::engine::general_purpose::STANDARD
    .decode(ciphertext_b64.as_bytes())
    .map_err(|e| format!("ciphertext base64 解码失败: {e}"))?;
  let cipher =
    Aes256Gcm::new_from_slice(&cfg.apiv3_key).map_err(|e| format!("APIv3Key 非法: {e}"))?;
  if nonce.len() != 12 {
    return Err("nonce 长度非法".to_string());
  }
  let plaintext = cipher
    .decrypt(
      Nonce::from_slice(nonce.as_bytes()),
      Payload { msg: &ct, aad: associated_data.as_bytes() },
    )
    .map_err(|_| "回调解密失败（APIv3Key 不匹配或数据被篡改）".to_string())?;
  String::from_utf8(plaintext).map_err(|e| format!("明文非 UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
  use super::*;

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
}
