//! S7（风险 R8）：支付异步回调路由。从 `main.rs` 拆出，行为不变。
//!
//! 验签 / 金额核验 / 幂等 / 发货逻辑在 `module_course::server` 内完成
//! （S6 已加固）；这里只做 HTTP 形态适配。反代需放行 `/api/pay/*`。

use axum::http::HeaderMap;
use axum::routing::post;
use axum::Router;

/// 挂载支付回调路由。
pub fn mount(router: Router) -> Router {
  router
    // M5b：支付宝异步回调（form-urlencoded）。返回纯文本 success/failure。
    .route(
      "/api/pay/alipay/notify",
      post(
        |axum::extract::Form(params): axum::extract::Form<
          std::collections::HashMap<String, String>,
        >| async move { module_course::server::handle_alipay_notify(params).await },
      ),
    )
    // M5c：微信支付 v3 异步回调（JSON）。验签需原始 body 逐字节一致 → 用 Bytes。
    .route(
      "/api/pay/wechat/notify",
      post(|headers: HeaderMap, body: axum::body::Bytes| async move {
        let mut map = std::collections::HashMap::new();
        for (k, v) in headers.iter() {
          if let Ok(s) = v.to_str() {
            map.insert(k.as_str().to_ascii_lowercase(), s.to_string());
          }
        }
        let body_str = String::from_utf8_lossy(&body).to_string();
        let (code, resp) = module_course::server::handle_wechat_notify(map, body_str).await;
        let status = axum::http::StatusCode::from_u16(code)
          .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        (status, resp)
      }),
    )
}
