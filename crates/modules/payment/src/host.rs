//! 宿主侧执行器（server-only，PM4）：发送 Provider 构造的 [`PayRequest`]
//! 并交回 Provider 解析。HTTP IO 留在宿主（Provider 纯构造 / 解析），
//! 与「签名在宿主」同属未来 WASM 化的边界约束。

use std::collections::HashMap;

use app_core::error::with_sources;

use crate::{
  OrderCall, OrderRequest, PayAction, PayError, PayRequest, PaymentProvider, RefundRequest,
  RefundResult,
};

/// 网关中文展示名（错误提示用），与旧 course 内嵌实现的文案一致。
fn gateway_label(name: &str) -> &str {
  match name {
    "alipay" => "支付宝",
    "wechat" => "微信支付",
    "stripe" => "Stripe",
    other => other,
  }
}

/// 发送一个已签名的网关请求，返回响应 body（HTTP 状态不在此判定——
/// 业务成败统一由 Provider 的 `parse_*` 从 body 判定）。
async fn send(pr: &PayRequest, label: &str) -> Result<String, PayError> {
  // 必须有超时：对账循环逐单 await，一个挂住的连接会让对账永久停住（Y5）。
  let client = reqwest::Client::builder()
    .connect_timeout(std::time::Duration::from_secs(5))
    .timeout(std::time::Duration::from_secs(15))
    .build()
    .map_err(|e| PayError::Gateway(format!("创建 HTTP 客户端失败: {e}")))?;
  let mut builder = match pr.method.as_str() {
    "GET" => client.get(&pr.url),
    _ => client.post(&pr.url),
  };
  for (k, v) in &pr.headers {
    builder = builder.header(k, v);
  }
  if pr.body_is_form {
    let form: HashMap<String, String> = pr.form.iter().cloned().collect();
    builder = builder.form(&form);
  } else if !pr.body.is_empty() {
    builder = builder.body(pr.body.clone());
  }
  let resp = builder
    .send()
    .await
    .map_err(|e| PayError::Gateway(format!("请求{label}失败: {}", with_sources(&e))))?;
  resp
    .text()
    .await
    .map_err(|e| PayError::Gateway(format!("读取{label}响应失败: {}", with_sources(&e))))
}

/// 下单：`build_order` →（Direct 直接返回 / Http 发送后 `parse_order_response`）。
pub async fn execute_order<P: PaymentProvider + Sync + ?Sized>(
  provider: &P,
  req: &OrderRequest,
) -> Result<PayAction, PayError> {
  match provider.build_order(req)? {
    OrderCall::Direct(action) => Ok(action),
    OrderCall::Http(pr) => {
      let body = send(&pr, gateway_label(provider.name())).await?;
      provider.parse_order_response(req, &body)
    }
  }
}

/// 查单（M5e 对账）：`build_query` → 发送 → `parse_query`。
pub async fn execute_query<P: PaymentProvider + Sync + ?Sized>(
  provider: &P,
  out_trade_no: &str,
) -> Result<crate::PaymentEvent, PayError> {
  let pr = provider.build_query(out_trade_no)?;
  let body = send(&pr, gateway_label(provider.name())).await?;
  provider.parse_query(&body)
}

/// 退款（M5e）：`build_refund` → 发送 → `parse_refund`。
pub async fn execute_refund<P: PaymentProvider + Sync + ?Sized>(
  provider: &P,
  req: &RefundRequest,
) -> Result<RefundResult, PayError> {
  let pr = provider.build_refund(req)?;
  let body = send(&pr, gateway_label(provider.name())).await?;
  provider.parse_refund(&body)
}

/// 按名称加载已配置的网关（订单行里存的 `provider`）。网关未编译进来或环境变量
/// 不全 → `Unconfigured`（文案给运维看缺哪组变量）。
pub fn from_env(name: &str) -> Result<Box<dyn PaymentProvider + Send + Sync>, PayError> {
  let unconfigured = |prefix: &str| {
    PayError::Unconfigured(format!("{}未配置（缺 {prefix}_* 环境变量）", gateway_label(name)))
  };
  match name {
    #[cfg(feature = "alipay")]
    "alipay" => crate::alipay::AlipayProvider::from_env()
      .map(|p| Box::new(p) as Box<dyn PaymentProvider + Send + Sync>)
      .map_err(|_| unconfigured("ALIPAY")),
    #[cfg(feature = "wechat")]
    "wechat" => crate::wechat::WechatProvider::from_env()
      .map(|p| Box::new(p) as Box<dyn PaymentProvider + Send + Sync>)
      .map_err(|_| unconfigured("WECHAT")),
    #[cfg(feature = "stripe")]
    "stripe" => crate::stripe::StripeProvider::from_env()
      .map(|p| Box::new(p) as Box<dyn PaymentProvider + Send + Sync>)
      .map_err(|_| unconfigured("STRIPE")),
    other => Err(PayError::Unsupported(format!("不支持的支付方式: {other}"))),
  }
}

/// 服务端已配置的网关（界面只列这些，DI4）。
pub fn configured() -> Vec<String> {
  ["alipay", "wechat", "stripe"]
    .into_iter()
    .filter(|name| from_env(name).is_ok())
    .map(String::from)
    .collect()
}

/// 启动自检（D1）里一个网关的配置状态。只带变量名，不带值。
#[derive(Debug, PartialEq)]
pub enum ConfigStatus {
  Enabled,
  /// 网关自己的变量一个都没设（`PAY_NOTIFY_BASE` 是共用的，不算）。
  Off,
  /// 设了一部分，缺这些。
  Missing(Vec<&'static str>),
  /// 都设了仍加载失败：某个值格式不对（密钥不是 base64、APIv3 密钥不是 32 字节等）。
  Invalid,
}

fn classify(
  loaded: bool,
  required: &[&'static str],
  is_set: impl Fn(&str) -> bool,
) -> ConfigStatus {
  if loaded {
    return ConfigStatus::Enabled;
  }
  if !required.iter().any(|v| *v != "PAY_NOTIFY_BASE" && is_set(v)) {
    return ConfigStatus::Off;
  }
  let missing: Vec<_> = required.iter().copied().filter(|v| !is_set(v)).collect();
  if missing.is_empty() {
    ConfigStatus::Invalid
  } else {
    ConfigStatus::Missing(missing)
  }
}

/// 已编译进来的网关各自的配置状态。
pub fn config_status() -> Vec<(&'static str, ConfigStatus)> {
  type Gateway = (&'static str, fn() -> bool, &'static [&'static str]);
  let gateways: &[Gateway] = &[
    #[cfg(feature = "alipay")]
    ("alipay", || crate::alipay::config().is_some(), crate::alipay::REQUIRED_ENV),
    #[cfg(feature = "wechat")]
    ("wechat", || crate::wechat::config().is_some(), crate::wechat::REQUIRED_ENV),
    #[cfg(feature = "stripe")]
    ("stripe", || crate::stripe::config().is_some(), crate::stripe::REQUIRED_ENV),
  ];
  let is_set = |v: &str| crate::crypto::env_nonempty(v).is_some();
  gateways
    .iter()
    .map(|&(name, loaded, required)| (name, classify(loaded(), required, is_set)))
    .collect()
}

/// 启动时每个网关打一行日志：配错的用 warn，免得只在用户下单时才看到「未配置」。
pub fn log_config_status() {
  for (name, status) in config_status() {
    match status {
      ConfigStatus::Enabled => tracing::info!(gateway = name, "payment gateway enabled"),
      ConfigStatus::Off => tracing::info!(gateway = name, "payment gateway not configured"),
      ConfigStatus::Missing(vars) => tracing::warn!(
        gateway = name,
        missing = vars.join(","),
        "payment gateway partly configured; disabled"
      ),
      ConfigStatus::Invalid => tracing::warn!(
        gateway = name,
        "payment gateway variables set but a value is malformed; disabled"
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn classify_states() {
    let req = &["A_KEY", "A_SECRET", "PAY_NOTIFY_BASE"];
    let set = |names: &'static [&'static str]| move |v: &str| names.contains(&v);
    assert_eq!(classify(true, req, set(&[])), ConfigStatus::Enabled);
    assert_eq!(classify(false, req, set(&[])), ConfigStatus::Off);
    // 只有共用的回调地址，不算配了这个网关
    assert_eq!(classify(false, req, set(&["PAY_NOTIFY_BASE"])), ConfigStatus::Off);
    assert_eq!(
      classify(false, req, set(&["A_KEY"])),
      ConfigStatus::Missing(vec!["A_SECRET", "PAY_NOTIFY_BASE"])
    );
    assert_eq!(
      classify(false, req, set(&["A_KEY", "A_SECRET", "PAY_NOTIFY_BASE"])),
      ConfigStatus::Invalid
    );
  }

  /// 自检列表必须和 `config()` 实际读的变量一致，否则会报错缺项或漏报。
  #[cfg(all(feature = "alipay", feature = "wechat", feature = "stripe"))]
  #[test]
  fn required_env_matches_config_readers() {
    fn read_by_config(src: &str) -> Vec<&str> {
      let body = src.split("pub fn config()").nth(1).unwrap_or("");
      let body = body.split("\n}\n").next().unwrap_or("");
      body
        .split("env_nonempty(\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .filter(|v| *v != "ALIPAY_GATEWAY") // 有默认值
        .collect()
    }
    let cases: [(&str, &[&str]); 3] = [
      (include_str!("alipay.rs"), crate::alipay::REQUIRED_ENV),
      (include_str!("wechat.rs"), crate::wechat::REQUIRED_ENV),
      (include_str!("stripe.rs"), crate::stripe::REQUIRED_ENV),
    ];
    for (src, required) in cases {
      let mut read = read_by_config(src);
      let mut required = required.to_vec();
      read.sort_unstable();
      required.sort_unstable();
      assert!(!read.is_empty());
      assert_eq!(read, required);
    }
  }

  #[test]
  fn unknown_gateway_is_unsupported() {
    assert!(matches!(from_env("paypal"), Err(PayError::Unsupported(_))));
    // 不读写环境变量（并行测试共享进程环境）；只确认列表不超出已知网关
    assert!(configured().iter().all(|n| ["alipay", "wechat", "stripe"].contains(&n.as_str())));
  }
}
