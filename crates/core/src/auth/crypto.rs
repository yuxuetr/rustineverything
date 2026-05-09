//! AES-256-GCM 加密工具，用于在数据库中保护 OAuth access_token 等敏感字段。
//!
//! - 密钥派生：从 `JWT_SECRET` 经 SHA-256 派生出 32 字节对称密钥。
//! - 编码格式：`base64-url-no-pad(nonce(12) || ciphertext || tag(16))`，方便直接存表。
//! - 加密失败 / 解密失败均不暴露内部细节，调用方通常应在解密失败时强制用户重新登录。

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, AeadCore},
    Aes256Gcm, Key, Nonce,
};
use base64::Engine;
use sha2::{Digest, Sha256};

/// 从 `JWT_SECRET` 派生 32 字节 AES 密钥。
fn derive_key() -> [u8; 32] {
    let secret = crate::session::get_jwt_secret();
    let mut hasher = Sha256::new();
    hasher.update(b"rustineverything::token-encryption-v1");
    hasher.update(secret.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// 加密 token 字符串。返回 base64url(no-pad) 编码，可安全存入数据库。
///
/// 失败场景：cipher 实例创建失败 / encrypt 调用失败。失败信息以字符串形式返回，
/// 不暴露密钥相关细节。
pub fn encrypt_token(plaintext: &str) -> Result<String, String> {
    let key_bytes = derive_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| "加密失败".to_string())?;

    let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
    out.extend_from_slice(nonce.as_slice());
    out.extend_from_slice(&ciphertext);
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(out))
}

/// 解密 token 字符串。如果输入不是合法的密文（被篡改 / 截断 / 错误密钥），返回 Err。
pub fn decrypt_token(encoded: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "密文 base64 解码失败".to_string())?;

    if bytes.len() < 12 + 16 {
        return Err("密文过短".to_string());
    }

    let key_bytes = derive_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let (nonce_bytes, ciphertext) = bytes.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "解密失败（可能密钥变更或密文被篡改）".to_string())?;

    String::from_utf8(plaintext).map_err(|_| "明文不是有效 UTF-8".to_string())
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
        let decoded = decrypt_token(&cipher).expect("decrypt");
        assert_eq!(decoded, plaintext);
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
