//! AES-256-GCM 加密工具，用于保护敏感字段（当前唯一调用点：OAuth PKCE
//! cookie，见 `auth/mod.rs::PkceCookiePayload`）。
//!
//! ## S5（风险 R2）：密钥治理
//! - **v2（当前）**：密钥优先取独立的 `DATA_ENCRYPTION_KEY` env（与 JWT
//!   签名密钥解耦，可独立轮换）；未配置时回退到 `JWT_SECRET` 派生并
//!   warn。密文带 `v2:` 前缀（key-id），为未来 v3/多密钥轮换预留格式。
//! - **v1（历史）**：`SHA256("…token-encryption-v1" ‖ JWT_SECRET)`，密文为
//!   无前缀裸 base64。解密保留 v1 回退路径（部署切换窗口内的在途
//!   cookie 仍可解）；**新密文不再以 v1 格式产出**。
//!
//! - 编码格式：`v2:` + `base64-url-no-pad(nonce(12) || ciphertext || tag(16))`。
//! - 加密/解密失败均不暴露内部细节，调用方通常应在解密失败时要求重新登录。

use aes_gcm::{
  aead::{Aead, AeadCore, KeyInit, OsRng},
  Aes256Gcm, Key, Nonce,
};
use base64::Engine;
use sha2::{Digest, Sha256};

/// v2 密文前缀（key-id）。未来密钥轮换时新增 `v3:` 等前缀，解密侧按
/// 前缀选密钥，实现多密钥共存的渐进轮换。
const V2_PREFIX: &str = "v2:";

/// 域分隔派生：`SHA256(tag ‖ secret)` → 32 字节。
fn derive_32(tag: &[u8], secret: &str) -> [u8; 32] {
  let mut hasher = Sha256::new();
  hasher.update(tag);
  hasher.update(secret.as_bytes());
  let digest = hasher.finalize();
  let mut out = [0u8; 32];
  out.copy_from_slice(&digest);
  out
}

/// v1（历史）：从 `JWT_SECRET` 派生。仅用于解密无前缀的存量密文。
fn derive_key_v1() -> [u8; 32] {
  derive_32(b"rustineverything::token-encryption-v1", &crate::session::get_jwt_secret())
}

/// v2（当前）：优先 `DATA_ENCRYPTION_KEY`，缺省回退 JWT_SECRET 派生并 warn。
///
/// 回退语义：未配置独立密钥时加密仍可用（不阻断登录流程），但 v2 与
/// JWT 密钥仍同源——warn 提醒运维尽早配置。域 tag 不同，v2 密钥不等于 v1。
fn derive_key_v2() -> [u8; 32] {
  match std::env::var("DATA_ENCRYPTION_KEY") {
    Ok(k) if !k.is_empty() => {
      crate::session::assert_not_placeholder("DATA_ENCRYPTION_KEY", &k);
      derive_32(b"rustineverything::data-encryption-v2", &k)
    }
    _ => {
      // 只在首次回退时 warn 一次，避免日志刷屏。
      static WARNED: std::sync::Once = std::sync::Once::new();
      WARNED.call_once(|| {
        tracing::warn!(
          "crypto: DATA_ENCRYPTION_KEY 未配置，回退到 JWT_SECRET 派生。建议配置独立密钥以支持分开轮换"
        );
      });
      derive_32(b"rustineverything::data-encryption-v2", &crate::session::get_jwt_secret())
    }
  }
}

fn encrypt_with_key(key_bytes: &[u8; 32], plaintext: &str) -> Result<Vec<u8>, String> {
  let key = Key::<Aes256Gcm>::from_slice(key_bytes);
  let cipher = Aes256Gcm::new(key);
  let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
  let ciphertext =
    cipher.encrypt(&nonce, plaintext.as_bytes()).map_err(|_| "加密失败".to_string())?;
  let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
  out.extend_from_slice(nonce.as_slice());
  out.extend_from_slice(&ciphertext);
  Ok(out)
}

fn decrypt_with_key(key_bytes: &[u8; 32], encoded: &str) -> Result<String, String> {
  let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
    .decode(encoded)
    .map_err(|_| "密文 base64 解码失败".to_string())?;

  if bytes.len() < 12 + 16 {
    return Err("密文过短".to_string());
  }

  let key = Key::<Aes256Gcm>::from_slice(key_bytes);
  let cipher = Aes256Gcm::new(key);
  let (nonce_bytes, ciphertext) = bytes.split_at(12);
  let nonce = Nonce::from_slice(nonce_bytes);

  let plaintext = cipher
    .decrypt(nonce, ciphertext)
    .map_err(|_| "解密失败（可能密钥变更或密文被篡改）".to_string())?;

  String::from_utf8(plaintext).map_err(|_| "明文不是有效 UTF-8".to_string())
}

/// 加密 token 字符串。返回 `v2:<base64url-no-pad>`，可安全存入数据库 / cookie。
///
/// 失败场景：cipher 实例创建失败 / encrypt 调用失败。失败信息以字符串形式返回，
/// 不暴露密钥相关细节。
pub fn encrypt_token(plaintext: &str) -> Result<String, String> {
  let out = encrypt_with_key(&derive_key_v2(), plaintext)?;
  Ok(format!("{}{}", V2_PREFIX, base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(out)))
}

/// 解密 token 字符串。按 key-id 前缀选密钥：
/// - `v2:` 前缀 → v2 密钥（DATA_ENCRYPTION_KEY 或回退派生）
/// - 无前缀 → v1 历史密钥（JWT_SECRET 派生），仅为平滑升级保留
///
/// 非法密文（被篡改 / 截断 / 错误密钥）返回 Err。
pub fn decrypt_token(encoded: &str) -> Result<String, String> {
  if let Some(rest) = encoded.strip_prefix(V2_PREFIX) {
    decrypt_with_key(&derive_key_v2(), rest)
  } else {
    decrypt_with_key(&derive_key_v1(), encoded)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 单测前确保 JWT_SECRET 存在（测试环境固定使用一个常量）。
  fn ensure_secret() {
    if std::env::var("JWT_SECRET").is_err() {
      // SAFETY: 测试环境单线程下设置环境变量
      unsafe {
        std::env::set_var("JWT_SECRET", "test-secret-for-crypto-tests-1234");
      }
    }
  }

  #[test]
  fn test_encrypt_decrypt_round_trip() {
    ensure_secret();
    let plaintext = "ghp_secret_token_xyz_1234567890";
    let cipher = encrypt_token(plaintext).expect("encrypt");
    assert_ne!(cipher, plaintext, "密文不能等于明文");
    assert!(cipher.starts_with("v2:"), "新密文应带 v2: 前缀（key-id）");
    let decoded = decrypt_token(&cipher).expect("decrypt");
    assert_eq!(decoded, plaintext);
  }

  /// S5：v1（无前缀，JWT_SECRET 派生）存量密文仍可解——平滑升级兼容。
  #[test]
  fn test_legacy_v1_ciphertext_still_decrypts() {
    ensure_secret();
    let plaintext = "legacy-pkce-cookie-payload";
    // 手工构造 v1 密文（复刻旧实现：v1 密钥 + 裸 base64）
    let v1 = encrypt_with_key(&derive_key_v1(), plaintext).expect("v1 encrypt");
    let legacy = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v1);
    assert!(!legacy.starts_with("v2:"));
    let decoded = decrypt_token(&legacy).expect("v1 密文应可解");
    assert_eq!(decoded, plaintext);
  }

  /// S5：配置独立 DATA_ENCRYPTION_KEY 后，v2 密钥与 JWT_SECRET 解耦：
  /// 用独立密钥加密的密文，在密钥变更后无法解密（轮换语义）。
  #[test]
  fn test_independent_key_rotation_semantics() {
    ensure_secret();
    // SAFETY: 单测序列化运行（--test-threads=1）
    unsafe {
      std::env::set_var("DATA_ENCRYPTION_KEY", "independent-data-key-A-0123456789ab");
    }
    let cipher = encrypt_token("rotate-me").expect("encrypt with key A");
    assert!(decrypt_token(&cipher).is_ok());

    // 轮换到密钥 B → 旧密文应拒绝（而非静默解出错误明文）
    unsafe {
      std::env::set_var("DATA_ENCRYPTION_KEY", "independent-data-key-B-0123456789ab");
    }
    assert!(decrypt_token(&cipher).is_err(), "密钥轮换后旧 v2 密文应解密失败");

    // 清理：恢复回退路径，不影响其它测试
    unsafe {
      std::env::remove_var("DATA_ENCRYPTION_KEY");
    }
  }

  #[test]
  fn test_each_encryption_uses_unique_nonce() {
    ensure_secret();
    let plaintext = "same-token";
    let a = encrypt_token(plaintext).expect("encrypt a");
    let b = encrypt_token(plaintext).expect("encrypt b");
    // nonce 随机，两次加密结果必须不同（极低概率冲突）
    assert_ne!(a, b, "相同明文两次加密应产生不同密文");
  }

  #[test]
  fn test_tampered_ciphertext_fails_decryption() {
    ensure_secret();
    let plaintext = "tamper-me";
    let mut cipher = encrypt_token(plaintext).expect("encrypt");
    // 翻转一个字符（在 base64 字符表内）
    let bytes: Vec<char> = cipher.chars().collect();
    let pos = bytes.len() / 2;
    let new_char = if bytes[pos] == 'a' { 'b' } else { 'a' };
    cipher.replace_range(
      cipher.char_indices().nth(pos).unwrap().0..cipher.char_indices().nth(pos + 1).unwrap().0,
      &new_char.to_string(),
    );
    assert!(decrypt_token(&cipher).is_err());
  }

  #[test]
  fn test_short_ciphertext_rejected() {
    ensure_secret();
    let result = decrypt_token("AAAAAAA");
    assert!(result.is_err());
  }

  #[test]
  fn test_invalid_base64_rejected() {
    ensure_secret();
    let result = decrypt_token("@@@not base64@@@");
    assert!(result.is_err());
  }
}
