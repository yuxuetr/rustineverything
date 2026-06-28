//! 购买 UI（M5d）：购买按钮 + 弹窗。
//!
//! 流程：选网关 → `create_order` →
//!   - redirect/h5（支付宝 page/wap、微信 h5）：浏览器跳转收银台；
//!   - qrcode（支付宝扫码、微信 native）：渲染二维码，用户扫码后点「刷新状态」
//!     → `query_order`，已支付则刷新页面解锁。
//!
//! 详见 docs/PAYMENT_SPEC.md 第 7 节。

use dioxus::document::eval;
use dioxus::prelude::*;
use dioxus::router::Link;

use crate::server::{create_order, list_my_orders, my_membership, query_order, OrderInfo};

/// 由 fast_qr 把支付链接渲染成 SVG 二维码字符串。
fn qr_svg(data: &str) -> String {
  use fast_qr::convert::svg::SvgBuilder;
  use fast_qr::qr::QRBuilder;
  match QRBuilder::new(data).build() {
    Ok(qr) => SvgBuilder::default().to_str(&qr),
    Err(_) => String::new(),
  }
}

/// 清洗 server fn 错误信息（去掉框架前缀，仅留中文提示）。
fn clean_err(e: &ServerFnError) -> String {
  let s = e.to_string();
  let s = s.rsplit("error running server function: ").next().unwrap_or(&s);
  s.split(" (details:").next().unwrap_or(s).trim().to_string()
}

/// 购买按钮：点击弹出购买弹窗。放在 Paywall / 课程详情。
#[component]
pub fn PurchaseButton(course_slug: String, price: i64) -> Element {
  let mut open = use_signal(|| false);
  let yuan = price / 100;
  rsx! {
      button {
          class: "inline-flex items-center justify-center rounded-md bg-[var(--color-primary)] px-6 py-2.5 text-sm font-semibold text-white hover:opacity-90 transition",
          onclick: move |_| open.set(true),
          "购买 ¥{yuan}"
      }
      if open() {
          PurchaseModal { course_slug: course_slug.clone(), price, open }
      }
  }
}

#[component]
fn PurchaseModal(course_slug: String, price: i64, open: Signal<bool>) -> Element {
  let mut open = open;
  let mut provider = use_signal(|| "alipay".to_string());
  let mut status = use_signal(|| "idle".to_string()); // idle | loading | qr | paid | error
  let mut message = use_signal(String::new);
  let mut qr = use_signal(String::new);
  let mut out_trade_no = use_signal(String::new);
  let yuan = price / 100;

  let start_pay = move |_| {
    let slug = course_slug.clone();
    let prov = provider();
    // PC 默认：支付宝跳转收银台(page)、微信 Native 扫码。
    let scene = if prov == "alipay" { "page" } else { "native" };
    status.set("loading".to_string());
    message.set(String::new());
    spawn(async move {
      match create_order(slug, prov, scene.to_string()).await {
        Ok(init) => match init.kind.as_str() {
          "redirect" | "h5" => {
            let js = format!(
              "window.location.href = {};",
              serde_json::to_string(&init.payload).unwrap_or_default()
            );
            let _ = eval(&js);
          }
          "qrcode" => {
            out_trade_no.set(init.out_trade_no.clone());
            qr.set(qr_svg(&init.payload));
            status.set("qr".to_string());
          }
          _ => {
            status.set("error".to_string());
            message.set("不支持的支付凭据".to_string());
          }
        },
        Err(e) => {
          status.set("error".to_string());
          message.set(clean_err(&e));
        }
      }
    });
  };

  let check_status = move |_| {
    let otn = out_trade_no();
    if otn.is_empty() {
      return;
    }
    spawn(async move {
      match query_order(otn).await {
        Ok(s) if s.paid => {
          status.set("paid".to_string());
          let _ = eval("setTimeout(function(){ window.location.reload(); }, 800);");
        }
        Ok(_) => message.set("尚未到账；完成支付后再点刷新".to_string()),
        Err(e) => message.set(clean_err(&e)),
      }
    });
  };

  let provider_label = if provider() == "alipay" { "支付宝" } else { "微信" };
  let radio = |val: &str, label: &str, cur: &str| {
    let active = val == cur;
    let class = if active {
      "flex-1 rounded-lg border-2 border-[var(--color-primary)] bg-[var(--color-primary)]/5 px-4 py-2 text-sm font-medium text-[var(--color-primary)]"
    } else {
      "flex-1 rounded-lg border border-slate-200 dark:border-slate-700 px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-300"
    };
    (class.to_string(), label.to_string(), val.to_string())
  };
  let (ali_c, ali_l, ali_v) = radio("alipay", "支付宝", &provider());
  let (wx_c, wx_l, wx_v) = radio("wechat", "微信支付", &provider());

  rsx! {
      div {
          class: "fixed inset-0 z-[60] flex items-center justify-center bg-black/50 p-4",
          onclick: move |_| open.set(false),
          div {
              class: "w-full max-w-sm rounded-2xl bg-white dark:bg-slate-900 p-6 shadow-xl",
              onclick: move |e| e.stop_propagation(),
              div { class: "flex items-center justify-between mb-4",
                  h3 { class: "text-lg font-bold text-slate-900 dark:text-white", "购买课程" }
                  button {
                      class: "text-slate-400 hover:text-slate-600 text-xl leading-none",
                      onclick: move |_| open.set(false),
                      "×"
                  }
              }

              match status().as_str() {
                  "qr" => rsx! {
                      p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "请使用{provider_label}扫码支付 ¥{yuan}" }
                      div {
                          class: "mx-auto w-48 [&>svg]:w-48 [&>svg]:h-48",
                          dangerous_inner_html: "{qr}"
                      }
                      button {
                          class: "mt-4 w-full rounded-md bg-[var(--color-primary)] px-4 py-2 text-sm font-semibold text-white hover:opacity-90",
                          onclick: check_status,
                          "我已支付，刷新状态"
                      }
                      if !message().is_empty() {
                          p { class: "mt-2 text-xs text-amber-600", "{message}" }
                      }
                  },
                  "paid" => rsx! {
                      div { class: "py-8 text-center",
                          div { class: "text-4xl mb-3", "✅" }
                          p { class: "font-semibold text-slate-900 dark:text-white", "支付成功，正在解锁…" }
                      }
                  },
                  _ => rsx! {
                      p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "选择支付方式，金额 ¥{yuan}" }
                      div { class: "flex gap-3 mb-4",
                          button { class: "{ali_c}", onclick: move |_| provider.set(ali_v.clone()), "{ali_l}" }
                          button { class: "{wx_c}", onclick: move |_| provider.set(wx_v.clone()), "{wx_l}" }
                      }
                      button {
                          class: "w-full rounded-md bg-[var(--color-primary)] px-4 py-2.5 text-sm font-semibold text-white hover:opacity-90 disabled:opacity-60",
                          disabled: status() == "loading",
                          onclick: start_pay,
                          if status() == "loading" { "处理中…" } else { "立即支付 ¥{yuan}" }
                      }
                      if status() == "error" && !message().is_empty() {
                          p { class: "mt-3 text-sm text-rose-600", "{message}" }
                      }
                  },
              }
          }
      }
  }
}

/// 订单状态 → (中文, 样式 class)。
fn status_badge(status: &str) -> (&'static str, &'static str) {
  match status {
    "paid" => {
      ("已支付", "bg-emerald-100 dark:bg-emerald-900/40 text-emerald-600 dark:text-emerald-400")
    }
    "pending" => ("待支付", "bg-amber-100 dark:bg-amber-900/40 text-amber-600 dark:text-amber-400"),
    "closed" => ("已关闭", "bg-slate-100 dark:bg-slate-800 text-slate-500"),
    "refunded" => ("已退款", "bg-sky-100 dark:bg-sky-900/40 text-sky-600 dark:text-sky-400"),
    _ => ("失败", "bg-rose-100 dark:bg-rose-900/40 text-rose-600 dark:text-rose-400"),
  }
}

/// 个人中心「我的订单」页（`/me/orders`）。
#[component]
pub fn MyOrdersPage() -> Element {
  let res = use_resource(|| async move { list_my_orders().await.unwrap_or_default() });
  let orders: Vec<OrderInfo> = res.read().clone().unwrap_or_default();
  let loaded = res.read().is_some();
  // Pro 会员状态
  let mem_res = use_resource(|| async move { my_membership().await.ok().flatten() });
  let membership = mem_res.read().clone().flatten();

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950 min-h-[60vh]",
          div { class: "mx-auto max-w-4xl px-4 sm:px-6 lg:px-8",
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-6", "我的订单" }

              // Pro 会员横幅
              if let Some(m) = membership {
                  {
                      let date = m.expires_at.split('T').next().unwrap_or(&m.expires_at).to_string();
                      if m.active {
                          rsx! {
                              div { class: "mb-6 flex items-center justify-between gap-4 rounded-xl border border-[var(--color-primary)]/30 bg-[var(--color-primary)]/5 px-5 py-4",
                                  div {
                                      span { class: "font-bold text-[var(--color-primary)]", "Pro 会员" }
                                      span { class: "ml-2 text-sm text-slate-500 dark:text-slate-400", "有效期至 {date}" }
                                  }
                                  span { class: "text-xs px-2 py-0.5 rounded-full bg-emerald-100 dark:bg-emerald-900/40 text-emerald-600 dark:text-emerald-400", "有效" }
                              }
                          }
                      } else {
                          rsx! {
                              div { class: "mb-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 px-5 py-4 text-sm text-slate-500",
                                  "Pro 会员已于 {date} 到期。"
                              }
                          }
                      }
                  }
              }
              if !loaded {
                  div { class: "flex items-center justify-center py-16",
                      div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-[var(--color-primary)]" }
                  }
              } else if orders.is_empty() {
                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 p-12 text-center",
                      p { class: "text-slate-400", "还没有订单。" }
                      Link { to: "/course", class: "inline-block mt-4 text-sm font-medium text-[var(--color-primary)] hover:underline", "去看看课程 →" }
                  }
              } else {
                  div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                      table { class: "w-full text-sm",
                          thead { class: "bg-slate-50 dark:bg-slate-900/60 text-slate-500 dark:text-slate-400",
                              tr {
                                  th { class: "text-left font-medium px-4 py-2", "课程" }
                                  th { class: "text-left font-medium px-4 py-2", "金额" }
                                  th { class: "text-left font-medium px-4 py-2", "渠道" }
                                  th { class: "text-left font-medium px-4 py-2", "状态" }
                                  th { class: "text-left font-medium px-4 py-2", "下单时间" }
                              }
                          }
                          tbody { class: "divide-y divide-slate-100 dark:divide-slate-800",
                              for o in orders.into_iter() {
                                  {
                                      let (label, badge) = status_badge(&o.status);
                                      let yuan = o.amount / 100;
                                      let chan = if o.provider == "alipay" { "支付宝" } else { "微信" };
                                      let date = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                      rsx! {
                                          tr { key: "{o.out_trade_no}", class: "text-slate-700 dark:text-slate-200",
                                              td { class: "px-4 py-2",
                                                  Link { to: format!("/course/{}", o.course_slug), class: "hover:text-[var(--color-primary)]", "{o.course_slug}" }
                                              }
                                              td { class: "px-4 py-2 font-medium", "¥{yuan}" }
                                              td { class: "px-4 py-2 text-slate-400", "{chan}" }
                                              td { class: "px-4 py-2",
                                                  span { class: "text-xs px-2 py-0.5 rounded-full font-medium {badge}", "{label}" }
                                              }
                                              td { class: "px-4 py-2 text-slate-400 text-xs", "{date}" }
                                          }
                                      }
                                  }
                              }
                          }
                      }
                  }
              }
          }
      }
  }
}
