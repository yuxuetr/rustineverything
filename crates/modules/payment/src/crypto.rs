//! 宿主侧 crypto 工具（server-only）：RSA2 签名/验签、AES-256-GCM 解密、
//! 密钥解码。**不入 [`crate::PaymentProvider`] trait**——密钥持有与运算留在
//! 宿主，是未来 WASM 化「密钥不进沙箱 / 验签在宿主」的安全红线。
//!
//! PM2：从 `module-course` 的 alipay.rs / wechat.rs 纯搬移（逻辑不变），
//! 支付宝与微信 v3 共用。密钥从 .env 读取（base64-DER 或 PEM），不回显。

use base64::Engine;

/// 读取非空环境变量（trim 后为空视为未配置）。
pub(crate) fn env_nonempty(key: &str) -> Option<String> {
  std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 把 .env 里的密钥（PEM 或裸 base64）解码为 DER 字节。
pub fn decode_key(raw: &str) -> Option<Vec<u8>> {
  let body: String = raw
    .lines()
    .filter(|l| !l.starts_with("-----"))
    .collect::<Vec<_>>()
    .join("")
    .split_whitespace()
    .collect();
  base64::engine::general_purpose::STANDARD.decode(body.as_bytes()).ok()
}

/// RSA2（SHA256withRSA，PKCS#1 v1.5）签名 → base64。
pub fn rsa2_sign(private_key_der: &[u8], content: &str) -> Result<String, String> {
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

/// RSA2 验签。
pub fn rsa2_verify(public_key_der: &[u8], content: &str, sign_b64: &str) -> bool {
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

/// AEAD_AES_256_GCM 解密（微信 v3 回调 resource）→ 明文字符串。
///
/// `nonce` 必须 12 字节；`aad` 为 associated_data；密文为 base64（含 GCM tag）。
pub fn aes256_gcm_decrypt(
  key: &[u8],
  nonce: &str,
  aad: &str,
  ciphertext_b64: &str,
) -> Result<String, String> {
  use aes_gcm::aead::{Aead, KeyInit, Payload};
  use aes_gcm::{Aes256Gcm, Nonce};

  let ct = base64::engine::general_purpose::STANDARD
    .decode(ciphertext_b64.as_bytes())
    .map_err(|e| format!("ciphertext base64 解码失败: {e}"))?;
  let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("APIv3Key 非法: {e}"))?;
  if nonce.len() != 12 {
    return Err("nonce 长度非法".to_string());
  }
  let plaintext = cipher
    .decrypt(Nonce::from_slice(nonce.as_bytes()), Payload { msg: &ct, aad: aad.as_bytes() })
    .map_err(|_| "回调解密失败（APIv3Key 不匹配或数据被篡改）".to_string())?;
  String::from_utf8(plaintext).map_err(|e| format!("明文非 UTF-8: {e}"))
}
