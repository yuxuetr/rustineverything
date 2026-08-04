//! 宿主侧执行器（server-only，PM4）：发送 Provider 构造的 [`PayRequest`]
//! 并交回 Provider 解析。HTTP IO 留在宿主（Provider 纯构造 / 解析），
//! 与「签名在宿主」同属未来 WASM 化的边界约束。

use std::collections::HashMap;

use crate::{OrderCall, OrderRequest, PayAction, PayError, PayRequest, PaymentProvider};

/// 网关中文展示名（错误提示用），与旧 course 内嵌实现的文案一致。
fn gateway_label(name: &str) -> &str {
  match name {
    "alipay" => "支付宝",
    "wechat" => "微信支付",
    other => other,
  }
}

/// 发送一个已签名的网关请求，返回响应 body（HTTP 状态不在此判定——
/// 业务成败统一由 Provider 的 `parse_*` 从 body 判定）。
async fn send(pr: &PayRequest, label: &str) -> Result<String, PayError> {
  let client = reqwest::Client::new();
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
  let resp =
    builder.send().await.map_err(|e| PayError::Gateway(format!("请求{label}失败: {e}")))?;
  resp.text().await.map_err(|e| PayError::Gateway(format!("读取{label}响应失败: {e}")))
}

/// 下单：`build_order` →（Direct 直接返回 / Http 发送后 `parse_order_response`）。
pub async fn execute_order<P: PaymentProvider + Sync>(
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
pub async fn execute_query<P: PaymentProvider + Sync>(
  provider: &P,
  out_trade_no: &str,
) -> Result<crate::PaymentEvent, PayError> {
  let pr = provider.build_query(out_trade_no)?;
  let body = send(&pr, gateway_label(provider.name())).await?;
  provider.parse_query(&body)
}
