//! module-payment：支付网关抽象（可选 crate，PM1–PM3）。
//!
//! 2026-07-27 评审结论落地：支付代码从 `module-course` 抽出为**可选 crate**
//! （feature 门控），trait 边界提供编译期安全，与 `module-moderation` 可选
//! 基建模式同构（MODULE_SPEC §11.3）。trait 边界 = 未来 WASM ABI 边界，
//! 两条路线不冲突。
//!
//! 分层：
//! - 本文件：中立类型 + [`PaymentProvider`] trait —— client / server 双目标
//!   均可编译（web 目标只带 serde，不带任何网关/crypto 重依赖）；
//! - [`crypto`]（server-only）：RSA2 签名/验签、AES-256-GCM 解密、密钥解码。
//!   **签名、验签、解密不入 trait**——由宿主侧 crypto 工具承担，为未来
//!   WASM 化保留“密钥不进沙箱 / 验签在宿主”的安全红线；
//! - [`alipay`] / [`wechat`]（feature 门控）：两个 Provider 实现（PM2 迁入）；
//! - [`pipeline`]：统一 notify 流水线——金额核验 → 原子认领 → 发货回调 →
//!   pay_audit 审计日志（PM3 迁入，S6 加固成果收敛于此）。

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================
// 中立类型（不含任何网关协议字段命名）
// =============================================================

/// 支付层错误。语义分类便于调用方决定「配置缺失 → 提示运维」/「网关拒绝 →
/// 提示用户」/「解析失败 → 记日志排查」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayError {
  /// 网关未配置（缺环境变量 / 密钥无法解析）。
  Unconfigured(String),
  /// 签名 / 密钥处理失败。
  Sign(String),
  /// 网关请求失败或网关返回业务错误。
  Gateway(String),
  /// 响应 / 回调解析失败。
  Parse(String),
  /// 该 Provider 不支持所请求的操作（如查询未实现）。
  Unsupported(String),
}

impl std::fmt::Display for PayError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PayError::Unconfigured(m) => write!(f, "支付网关未配置: {m}"),
      PayError::Sign(m) => write!(f, "签名失败: {m}"),
      PayError::Gateway(m) => write!(f, "网关错误: {m}"),
      PayError::Parse(m) => write!(f, "解析失败: {m}"),
      PayError::Unsupported(m) => write!(f, "不支持的操作: {m}"),
    }
  }
}

impl std::error::Error for PayError {}

/// 下单入参（由调用方从订单快照构造；金额一律用「分」）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderRequest {
  /// 我方订单号（幂等键）。
  pub out_trade_no: String,
  /// 商品标题（网关收银台展示）。
  pub subject: String,
  /// 金额（分）。
  pub amount_cents: i64,
  /// 货币代码（当前仅 CNY）。
  pub currency: String,
  /// 支付场景：`page` | `wap` | `qr`（支付宝）/ `native` | `h5`（微信）。
  pub scene: String,
  /// 付款用户 IP（微信 h5 必填；其余场景可空）。
  #[serde(default)]
  pub client_ip: Option<String>,
}

/// 需要宿主发出的网关 HTTP 请求（Provider 只构造，不发送）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PayRequest {
  pub url: String,
  /// `POST` / `GET`。
  pub method: String,
  /// 额外请求头（如微信 v3 `Authorization`）。
  pub headers: Vec<(String, String)>,
  /// 请求体：`form` 序列化由 `body_form` 表达，JSON 由 `body` 表达。
  pub body: String,
  /// body 是否为 form-urlencoded 键值对（true：支付宝网关表单；false：JSON）。
  #[serde(default)]
  pub body_is_form: bool,
  /// form 模式下的键值对（`body_is_form = true` 时使用）。
  #[serde(default)]
  pub form: Vec<(String, String)>,
  /// 被签名的原文（审计 / 单测比对用；密钥与签名过程在宿主 crypto 工具中）。
  #[serde(default)]
  pub sign_payload: Option<String>,
}

/// 下单产物：把用户引导到支付的动作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PayAction {
  /// 跳转收银台（支付宝 page/wap、微信 h5）。
  PayUrl(String),
  /// 展示二维码（支付宝 precreate、微信 native 的 code_url）。
  QrCode(String),
}

/// 下单调用形态：部分场景「签名后直接跳转」，无需访问网关。
#[derive(Debug, Clone, PartialEq)]
pub enum OrderCall {
  /// 无需 HTTP：签名拼 URL 即得支付动作（支付宝 page/wap）。
  Direct(PayAction),
  /// 需要 HTTP：宿主发送 [`PayRequest`]，响应交 [`PaymentProvider::parse_order_response`]。
  Http(PayRequest),
}

/// 已验签（及解密）后的回调载荷——验签 / 解密由宿主侧完成后再交给
/// [`PaymentProvider::parse_notify`]，Provider 只做字段中立化。
#[derive(Debug, Clone, PartialEq)]
pub enum NotifyPayload {
  /// form-urlencoded 键值对（支付宝）。
  Form(HashMap<String, String>),
  /// JSON 明文（微信 v3 解密后的 resource）。
  Json(serde_json::Value),
}

/// `PaymentEvent::status` 中「支付成功」的规范值；其余保留网关原始状态串。
pub const EVENT_STATUS_SUCCESS: &str = "success";

/// 中立支付事件（回调 / 查询响应统一形态）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentEvent {
  /// 网关标识：`alipay` | `wechat`。
  pub provider: String,
  pub out_trade_no: String,
  /// 网关侧回报金额（分）；解析失败 / 缺失时为负值，流水线金额核验必拒。
  pub amount_cents: i64,
  /// 规范化状态：成功一律 [`EVENT_STATUS_SUCCESS`]，其余保留原始状态串。
  pub status: String,
  /// 网关流水号。
  pub txn_id: Option<String>,
}

impl PaymentEvent {
  pub fn is_success(&self) -> bool {
    self.status == EVENT_STATUS_SUCCESS
  }
}

// =============================================================
// PaymentProvider trait
// =============================================================

/// 支付网关能力边界（编译期安全的第一方扩展点；未来 WASM ABI 的雏形）。
///
/// 设计约束：
/// - **纯构造 / 解析**：方法不做 IO（HTTP 由宿主发送、DB 由流水线操作）；
/// - **签名、验签、解密不入 trait**：密钥持有与 crypto 运算留在宿主工具
///   （[`crypto`] 模块），Provider 实现内部调用——WASM 化时该边界即
///   「密钥不进沙箱」红线；
/// - `build_query` / `parse_query` 为 M5e 余项（对账 / 关单）预留，默认
///   [`PayError::Unsupported`]。
pub trait PaymentProvider {
  /// 网关标识：`alipay` | `wechat`。
  fn name(&self) -> &'static str;

  /// 构造下单调用（含签名后的参数 / 请求头）。
  fn build_order(&self, req: &OrderRequest) -> Result<OrderCall, PayError>;

  /// 解析网关下单响应 → 支付动作（仅 [`OrderCall::Http`] 场景需要）。
  fn parse_order_response(&self, req: &OrderRequest, body: &str) -> Result<PayAction, PayError>;

  /// 解析（已验签 / 解密的）回调载荷 → 中立支付事件。
  fn parse_notify(&self, payload: &NotifyPayload) -> Result<PaymentEvent, PayError>;

  /// 构造订单查询请求（对账预留；M5e 余项实现时填充）。
  fn build_query(&self, out_trade_no: &str) -> Result<PayRequest, PayError> {
    Err(PayError::Unsupported(format!(
      "{}: 订单查询未实现（out_trade_no={out_trade_no}）",
      self.name()
    )))
  }

  /// 解析订单查询响应（对账预留）。
  fn parse_query(&self, body: &str) -> Result<PaymentEvent, PayError> {
    let _ = body;
    Err(PayError::Unsupported(format!("{}: 订单查询未实现", self.name())))
  }
}

// =============================================================
// server-only 子模块（PM2/PM3 迁入实现）
// =============================================================

#[cfg(feature = "server")]
pub mod crypto;

#[cfg(feature = "server")]
pub mod host;

#[cfg(all(feature = "server", feature = "alipay"))]
pub mod alipay;

#[cfg(all(feature = "server", feature = "wechat"))]
pub mod wechat;

#[cfg(feature = "server")]
pub mod pipeline;

// =============================================================
// Tests
// =============================================================

#[cfg(test)]
mod tests {
  use super::*;

  struct DummyProvider;

  impl PaymentProvider for DummyProvider {
    fn name(&self) -> &'static str {
      "dummy"
    }
    fn build_order(&self, _req: &OrderRequest) -> Result<OrderCall, PayError> {
      Ok(OrderCall::Direct(PayAction::PayUrl("https://pay.example/checkout".into())))
    }
    fn parse_order_response(&self, _req: &OrderRequest, body: &str) -> Result<PayAction, PayError> {
      Ok(PayAction::QrCode(body.to_string()))
    }
    fn parse_notify(&self, payload: &NotifyPayload) -> Result<PaymentEvent, PayError> {
      match payload {
        NotifyPayload::Form(p) => Ok(PaymentEvent {
          provider: "dummy".into(),
          out_trade_no: p.get("out_trade_no").cloned().unwrap_or_default(),
          amount_cents: 100,
          status: EVENT_STATUS_SUCCESS.into(),
          txn_id: None,
        }),
        NotifyPayload::Json(_) => Err(PayError::Parse("dummy 只收 form".into())),
      }
    }
  }

  #[test]
  fn query_defaults_to_unsupported() {
    let p = DummyProvider;
    assert!(matches!(p.build_query("RIE1"), Err(PayError::Unsupported(_))));
    assert!(matches!(p.parse_query("{}"), Err(PayError::Unsupported(_))));
  }

  #[test]
  fn event_success_normalization() {
    let ok = PaymentEvent {
      provider: "dummy".into(),
      out_trade_no: "RIE1".into(),
      amount_cents: 9900,
      status: EVENT_STATUS_SUCCESS.into(),
      txn_id: Some("t1".into()),
    };
    assert!(ok.is_success());
    let other = PaymentEvent { status: "WAIT_BUYER_PAY".into(), ..ok };
    assert!(!other.is_success());
  }

  #[test]
  fn pay_error_display_is_categorized() {
    assert!(PayError::Unconfigured("缺 ALIPAY_APP_ID".into()).to_string().contains("未配置"));
    assert!(PayError::Gateway("insufficient balance".into()).to_string().contains("网关错误"));
  }
}
