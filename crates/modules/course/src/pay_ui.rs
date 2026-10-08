//! 购买 UI（M5d）：购买按钮 + 弹窗。
//!
//! 流程：选网关 → `create_order` →
//!   - redirect/h5（支付宝 page/wap、微信 h5）：浏览器跳转收银台；
//!   - qrcode（支付宝扫码、微信 native）：渲染二维码，用户扫码后点「刷新状态」
//!     → `query_order`，已支付则刷新页面解锁。
//!
//! 详见 docs/PAYMENT_SPEC.md 第 7 节。

use dioxus::prelude::*;
use dioxus::router::Link;
use dioxus_shadcn::{
  button_class, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant,
  Dialog, DialogClose, DialogContent, DialogOverlay, DialogTitle, DialogTrigger, Spinner,
  SpinnerSize, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, ToggleGroup,
  ToggleGroupItem, UiDensity,
};

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
      Dialog { open: open(), on_open_change: move |v| open.set(v),
          DialogTrigger {
              class: button_class(ButtonVariant::Primary, ButtonSize::Md, UiDensity::Comfortable, "px-6 font-semibold"),
              "购买 ¥{yuan}"
          }
          DialogOverlay { class: "z-[60]" }
          DialogContent { class: "z-[60] max-w-sm gap-0 rounded-2xl",
              DialogTitle { class: "mb-4 pr-8", "购买课程" }
              DialogClose { class: "text-xl leading-none text-muted-foreground",
                  span { "aria-hidden": "true", "×" }
                  span { class: "sr-only", "关闭" }
              }
              // 内容只在打开时挂载：每次重新打开都从选择支付方式开始。
              if open() {
                  PurchaseForm { course_slug: course_slug.clone(), price }
              }
          }
      }
  }
}

#[component]
fn PurchaseForm(course_slug: String, price: i64) -> Element {
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
            widgets::browser::navigate(&init.payload);
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
          widgets::browser::sleep_ms(800).await;
          widgets::browser::reload();
        }
        Ok(_) => message.set("尚未到账；完成支付后再点刷新".to_string()),
        Err(e) => message.set(clean_err(&e)),
      }
    });
  };

  let provider_label = if provider() == "alipay" { "支付宝" } else { "微信" };
  let option_class = move |val: &str| {
    if provider() == val {
      "flex-1 border border-primary bg-primary/5 text-primary hover:bg-primary/10"
    } else {
      "flex-1 border border-input"
    }
  };

  rsx! {
      match status().as_str() {
          "qr" => rsx! {
              p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "请使用{provider_label}扫码支付 ¥{yuan}" }
              div {
                  class: "mx-auto w-48 [&>svg]:w-48 [&>svg]:h-48",
                  dangerous_inner_html: "{qr}"
              }
              Button {
                  class: "mt-4 w-full font-semibold",
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
              // 受控单选：再点已选项时 ToggleGroup 会报空值，忽略它，保证总有一个网关被选中。
              ToggleGroup {
                  class: "flex w-full gap-3 mb-4",
                  "aria-label": "支付方式",
                  value: provider(),
                  on_value_change: move |v: String| if !v.is_empty() { provider.set(v) },
                  ToggleGroupItem { value: "alipay", class: option_class("alipay"), "支付宝" }
                  ToggleGroupItem { value: "wechat", class: option_class("wechat"), "微信支付" }
              }
              Button {
                  class: "w-full font-semibold",
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

/// 订单状态 → (中文, 徽章样式)。
fn status_badge(status: &str) -> (&'static str, BadgeVariant) {
  match status {
    "paid" => ("已支付", BadgeVariant::Success),
    "pending" => ("待支付", BadgeVariant::Warning),
    "closed" => ("已关闭", BadgeVariant::Secondary),
    "refunded" => ("已退款", BadgeVariant::Info),
    _ => ("失败", BadgeVariant::Destructive),
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
                              div { class: "mb-6 flex items-center justify-between gap-4 rounded-xl border border-primary/30 bg-primary/5 px-5 py-4",
                                  div {
                                      span { class: "font-bold text-primary", "Pro 会员" }
                                      span { class: "ml-2 text-sm text-slate-500 dark:text-slate-400", "有效期至 {date}" }
                                  }
                                  Badge { variant: BadgeVariant::Success, "有效" }
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
                      Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                  }
              } else if orders.is_empty() {
                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 p-12 text-center",
                      p { class: "text-slate-400", "还没有订单。" }
                      Link { to: "/course", class: "inline-block mt-4 text-sm font-medium text-primary hover:underline", "去看看课程 →" }
                  }
              } else {
                  div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                      Table {
                          TableHeader { class: "bg-muted/50",
                              TableRow {
                                  TableHead { class: "h-10 px-4", "课程" }
                                  TableHead { class: "h-10 px-4", "金额" }
                                  TableHead { class: "h-10 px-4", "渠道" }
                                  TableHead { class: "h-10 px-4", "状态" }
                                  TableHead { class: "h-10 px-4", "下单时间" }
                              }
                          }
                          TableBody {
                              for o in orders.into_iter() {
                                  {
                                      let (label, badge) = status_badge(&o.status);
                                      let yuan = o.amount / 100;
                                      let chan = if o.provider == "alipay" { "支付宝" } else { "微信" };
                                      let date = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                      rsx! {
                                          TableRow { key: "{o.out_trade_no}",
                                              TableCell { class: "px-4 py-2",
                                                  Link { to: format!("/course/{}", o.course_slug), class: "hover:text-primary", "{o.course_slug}" }
                                              }
                                              TableCell { class: "px-4 py-2 font-medium", "¥{yuan}" }
                                              TableCell { class: "px-4 py-2 text-slate-400", "{chan}" }
                                              TableCell { class: "px-4 py-2",
                                                  Badge { variant: badge, "{label}" }
                                              }
                                              TableCell { class: "px-4 py-2 text-slate-400 text-xs", "{date}" }
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
