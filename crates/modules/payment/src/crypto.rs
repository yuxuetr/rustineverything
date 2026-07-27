//! 宿主侧 crypto 工具（server-only）：RSA2 签名/验签、AES-256-GCM 解密、
//! 密钥解码。**不入 [`crate::PaymentProvider`] trait**——密钥持有与运算留在
//! 宿主，是未来 WASM 化「密钥不进沙箱 / 验签在宿主」的安全红线。
//!
//! PM2：从 `module-course` 的 alipay.rs / wechat.rs 迁入实现。
