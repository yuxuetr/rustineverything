//! 微信支付 v3 Provider（feature = "wechat" + "server"）。
//!
//! PM2：从 `module-course/src/wechat.rs` 纯搬移 + 适配为
//! [`crate::PaymentProvider`] 实现（native/h5 下单、回调验签 + AES-GCM 解密）。
