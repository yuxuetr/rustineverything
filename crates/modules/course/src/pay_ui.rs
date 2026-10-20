//! 购买 UI（M5d）：购买按钮 + 弹窗；打赏卡片复用同一个弹窗（FEATURE_TIERS.md §6）。
//!
//! 流程：选网关 → `create_order` / `create_tip_order` →
//!   - redirect/h5（支付宝 page/wap、微信 h5）：浏览器跳转收银台；
//!   - qrcode（支付宝扫码、微信 native）：渲染二维码，用户扫码后点「刷新状态」
//!     → `query_order`，已支付则刷新页面解锁。
//!
//! 详见 docs/PAYMENT_SPEC.md 第 7 节。

use dioxus::prelude::*;
use dioxus::router::Link;
use dioxus_shadcn::{
  button_class, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Dialog, DialogClose,
  DialogContent, DialogOverlay, DialogTitle, DialogTrigger, Spinner, SpinnerSize, Table, TableBody,
  TableCell, TableHead, TableHeader, TableRow, ToggleGroup, ToggleGroupItem, UiDensity,
};

use app_core::i18n::t;

use crate::pricing::{
  channel_key, methods_for, region_key, set_region, switch_key, use_region, Price, Pricing, Region,
};
use crate::product::{tip_pricing, ProductKind, TIP_TIERS};
use crate::server::{
  create_order, create_tip_order, list_my_orders, my_membership, payment_providers, query_order,
  OrderInfo,
};

/// 由 fast_qr 把支付链接渲染成 SVG 二维码字符串。
fn qr_svg(data: &str) -> String {
  use fast_qr::convert::svg::SvgBuilder;
  use fast_qr::qr::QRBuilder;
  match QRBuilder::new(data).build() {
    Ok(qr) => SvgBuilder::default().to_str(&qr),
    Err(_) => String::new(),
  }
}

fn current_lang() -> app_core::i18n::Language {
  try_consume_context::<Signal<app_core::i18n::Language>>().map(|s| s()).unwrap_or_default()
}

/// 清洗 server fn 错误信息（去掉框架前缀）。下单 / 查单的用户错误是 i18n key，
/// 调用方再过一遍 `t()`；其余（网关原文）`t()` 原样返回。
fn clean_err(e: &ServerFnError) -> String {
  let s = e.to_string();
  let s = s.rsplit("error running server function: ").next().unwrap_or(&s);
  s.split(" (details:").next().unwrap_or(s).trim().to_string()
}

/// 弹窗里买的东西。
#[derive(Debug, Clone, PartialEq)]
enum Item {
  Course {
    slug: String,
  },
  /// `tier` 是 [`TIP_TIERS`] 的序号，`source` 是打赏发生的页面。
  Tip {
    source: String,
    tier: u32,
  },
}

/// 购买按钮：点击弹出购买弹窗。放在 Paywall / 课程详情。按钮上的价格随地区变化。
#[component]
pub fn PurchaseButton(course_slug: String, pricing: Pricing) -> Element {
  let lang = current_lang();
  let mut open = use_signal(|| false);
  let price = pricing.list_price(use_region()).label();
  rsx! {
      Dialog { open: open(), on_open_change: move |v| open.set(v),
          DialogTrigger {
              class: button_class(ButtonVariant::Primary, ButtonSize::Md, UiDensity::Comfortable, "px-6 font-semibold"),
              "{t(lang, \"pay.buy\")} {price}"
          }
          DialogOverlay { class: "z-[60]" }
          DialogContent { class: "z-[60] max-w-sm gap-0 rounded-2xl",
              DialogTitle { class: "mb-4 pr-8", "{t(lang, \"pay.dialog_title\")}" }
              DialogClose { class: "text-xl leading-none text-muted-foreground",
                  span { "aria-hidden": "true", "×" }
                  span { class: "sr-only", "{t(lang, \"pay.close\")}" }
              }
              // 内容只在打开时挂载：每次重新打开都从选择支付方式开始。
              if open() {
                  PurchaseForm { item: Item::Course { slug: course_slug.clone() }, pricing: pricing.clone() }
              }
          }
      }
  }
}

#[component]
fn PurchaseForm(item: Item, pricing: Pricing) -> Element {
  let lang = current_lang();
  let region = use_region();
  // 服务端已配置的网关；与地区取交集（DI4）。
  let configured = use_resource(|| async move { payment_providers().await.unwrap_or_default() });
  let methods = methods_for(region, &configured.read().clone().unwrap_or_default());
  let mut chosen = use_signal(|| None::<String>);
  // 用户选过且仍在本地区列表里就用它，否则用列表第一个（切换地区后自动换）。
  let provider = chosen().filter(|p| methods.contains(p)).or_else(|| methods.first().cloned());
  let price = provider
    .as_deref()
    .and_then(|p| pricing.quote(p, region).ok())
    .unwrap_or_else(|| pricing.list_price(region))
    .label();
  let mut status = use_signal(|| "idle".to_string()); // idle | loading | qr | paid | error
  let mut message = use_signal(String::new);
  let mut qr = use_signal(String::new);
  let mut out_trade_no = use_signal(String::new);

  // 课程付款后刷新页面解锁；打赏没有要解锁的，停在感谢页。
  let is_tip = matches!(item, Item::Tip { .. });
  let pay_with = provider.clone();
  let start_pay = move |_| {
    let Some(prov) = pay_with.clone() else {
      return;
    };
    let item = item.clone();
    // PC 默认：支付宝跳转收银台(page)、微信 Native 扫码；Stripe 跳转托管收银台。
    let scene = if prov == "wechat" { "native" } else { "page" }.to_string();
    let region = region.as_str().to_string();
    status.set("loading".to_string());
    message.set(String::new());
    spawn(async move {
      let created = match item {
        Item::Course { slug } => create_order(slug, prov, scene, region).await,
        Item::Tip { source, tier } => create_tip_order(tier, source, prov, scene, region).await,
      };
      match created {
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
            message.set(t(lang, "pay.bad_credential"));
          }
        },
        Err(e) => {
          status.set("error".to_string());
          message.set(t(lang, &clean_err(&e)));
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
          if !is_tip {
            widgets::browser::sleep_ms(800).await;
            widgets::browser::reload();
          }
        }
        Ok(_) => message.set(t(lang, "pay.not_yet")),
        Err(e) => message.set(t(lang, &clean_err(&e))),
      }
    });
  };

  let provider_label = provider.as_deref().map(|p| t(lang, channel_key(p))).unwrap_or_default();
  let selected = provider.clone();
  let option_class = move |val: &str| {
    if selected.as_deref() == Some(val) {
      "flex-1 border border-primary bg-primary/5 text-primary hover:bg-primary/10"
    } else {
      "flex-1 border border-input"
    }
  };
  let other_region = match region {
    Region::Mainland => Region::Overseas,
    Region::Overseas => Region::Mainland,
  };

  rsx! {
      match status().as_str() {
          "qr" => rsx! {
              p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "{t(lang, \"pay.scan\")} · {provider_label} · {price}" }
              div {
                  class: "mx-auto w-48 [&>svg]:w-48 [&>svg]:h-48",
                  dangerous_inner_html: "{qr}"
              }
              Button {
                  class: "mt-4 w-full font-semibold",
                  onclick: check_status,
                  "{t(lang, \"pay.i_paid\")}"
              }
              if !message().is_empty() {
                  p { class: "mt-2 text-xs text-amber-600", "{message}" }
              }
          },
          "paid" => rsx! {
              div { class: "py-8 text-center",
                  div { class: "text-4xl mb-3", "✅" }
                  p { class: "font-semibold text-slate-900 dark:text-white",
                      if is_tip { "{t(lang, \"pay.tip.thanks\")}" } else { "{t(lang, \"pay.success\")}" }
                  }
              }
          },
          _ => rsx! {
              p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "{t(lang, \"pay.choose\")} {price}" }
              if configured.read().is_none() {
                  div { class: "flex justify-center py-4", Spinner { size: SpinnerSize::Md } }
              } else if methods.is_empty() {
                  p { class: "mb-4 text-sm text-amber-600", "{t(lang, \"pay.no_methods\")}" }
              } else {
                  // 受控单选：再点已选项时 ToggleGroup 会报空值，忽略它，保证总有一个网关被选中。
                  ToggleGroup {
                      class: "flex w-full gap-3 mb-4",
                      "aria-label": t(lang, "pay.methods"),
                      value: provider.clone().unwrap_or_default(),
                      on_value_change: move |v: String| if !v.is_empty() { chosen.set(Some(v)) },
                      for m in methods.iter() {
                          ToggleGroupItem { key: "{m}", value: "{m}", class: option_class(m), "{t(lang, channel_key(m))}" }
                      }
                  }
                  Button {
                      class: "w-full font-semibold",
                      disabled: status() == "loading",
                      onclick: start_pay,
                      if status() == "loading" { "{t(lang, \"pay.processing\")}" } else { "{t(lang, \"pay.pay_now\")} {price}" }
                  }
              }
              if status() == "error" && !message().is_empty() {
                  p { class: "mt-3 text-sm text-rose-600", "{message}" }
              }
              // 地区按时区判断，可能不准（如出差、改过系统时区）：允许手动切换。
              p { class: "mt-4 text-center text-xs text-slate-500 dark:text-slate-400",
                  span { "🌐 {t(lang, region_key(region))} · " }
                  button {
                      r#type: "button",
                      class: "font-medium text-primary hover:underline",
                      onclick: move |_| set_region(other_region),
                      "{t(lang, switch_key(other_region))}"
                  }
              }
          },
      }
  }
}

/// 打赏卡片（博客文章末尾、播客页底部）。服务端没配网关时不显示（DT4）。
/// 登录状态与登录弹窗在 app 层，由调用方传入：未登录点按钮打开登录弹窗。
#[component]
pub fn TipJar(source: String, signed_in: bool, on_sign_in: EventHandler<()>) -> Element {
  let lang = current_lang();
  let mut open = use_signal(|| false);
  let configured = use_resource(|| async move { payment_providers().await.unwrap_or_default() });
  if configured.read().as_ref().is_none_or(|c| c.is_empty()) {
    return rsx! {};
  }
  rsx! {
      div { class: "mt-12 flex flex-col items-center gap-3 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 px-6 py-8 text-center",
          p { class: "font-semibold text-slate-900 dark:text-white", "{t(lang, \"pay.tip.title\")}" }
          p { class: "text-sm text-slate-500 dark:text-slate-400", "{t(lang, \"pay.tip.body\")}" }
          if signed_in {
              Dialog { open: open(), on_open_change: move |v| open.set(v),
                  DialogTrigger {
                      class: button_class(ButtonVariant::Primary, ButtonSize::Md, UiDensity::Comfortable, "mt-2 px-6 font-semibold"),
                      "{t(lang, \"pay.tip.button\")}"
                  }
                  DialogOverlay { class: "z-[60]" }
                  DialogContent { class: "z-[60] max-w-sm gap-0 rounded-2xl",
                      DialogTitle { class: "mb-4 pr-8", "{t(lang, \"pay.tip.dialog_title\")}" }
                      DialogClose { class: "text-xl leading-none text-muted-foreground",
                          span { "aria-hidden": "true", "×" }
                          span { class: "sr-only", "{t(lang, \"pay.close\")}" }
                      }
                      if open() {
                          TipForm { source: source.clone() }
                      }
                  }
              }
          } else {
              Button {
                  class: "mt-2 px-6 font-semibold",
                  onclick: move |_| on_sign_in.call(()),
                  "{t(lang, \"pay.tip.button\")}"
              }
              p { class: "text-xs text-slate-400", "{t(lang, \"pay.tip.sign_in\")}" }
          }
      }
  }
}

/// 档位选择 + 购买表单。换档位时表单重新挂载（`key`），从选支付方式开始。
#[component]
fn TipForm(source: String) -> Element {
  let lang = current_lang();
  let region = use_region();
  let mut tier = use_signal(|| 1u32);
  let item_class = move |i: u32| {
    if tier() == i {
      "flex-1 border border-primary bg-primary/5 text-primary hover:bg-primary/10"
    } else {
      "flex-1 border border-input"
    }
  };
  // 档位在常量表内，取不到只可能是表被改空了；那样就不渲染表单。
  let Some(pricing) = tip_pricing(tier() as usize) else {
    return rsx! {};
  };
  rsx! {
      p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "{t(lang, \"pay.tip.amount\")}" }
      ToggleGroup {
          class: "flex w-full gap-2 mb-5",
          "aria-label": t(lang, "pay.tip.amount"),
          value: tier().to_string(),
          on_value_change: move |v: String| if let Ok(i) = v.parse() { tier.set(i) },
          for i in 0..TIP_TIERS.len() as u32 {
              {
                  let label = tip_pricing(i as usize).map(|p| p.list_price(region).label()).unwrap_or_default();
                  rsx! { ToggleGroupItem { key: "{i}", value: "{i}", class: item_class(i), "{label}" } }
              }
          }
      }
      {
          let tier = tier();
          rsx! { PurchaseForm { key: "{tier}", item: Item::Tip { source: source.clone(), tier }, pricing } }
      }
  }
}

/// 订单状态 → (i18n key, 徽章样式)。
fn status_badge(status: &str) -> (&'static str, BadgeVariant) {
  match status {
    "paid" => ("pay.status.paid", BadgeVariant::Success),
    "pending" => ("pay.status.pending", BadgeVariant::Warning),
    "closed" => ("pay.status.closed", BadgeVariant::Secondary),
    "refunded" => ("pay.status.refunded", BadgeVariant::Info),
    _ => ("pay.status.failed", BadgeVariant::Destructive),
  }
}

/// 个人中心「我的订单」页（`/me/orders`）。
#[component]
pub fn MyOrdersPage() -> Element {
  let lang = current_lang();
  let res = use_resource(|| async move { list_my_orders().await.unwrap_or_default() });
  let orders: Vec<OrderInfo> = res.read().clone().unwrap_or_default();
  let loaded = res.read().is_some();
  // Pro 会员状态
  let mem_res = use_resource(|| async move { my_membership().await.ok().flatten() });
  let membership = mem_res.read().clone().flatten();

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950 min-h-[60vh]",
          div { class: "mx-auto max-w-4xl px-4 sm:px-6 lg:px-8",
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-6", "{t(lang, \"pay.orders.title\")}" }

              // Pro 会员横幅
              if let Some(m) = membership {
                  {
                      let date = m.expires_at.split('T').next().unwrap_or(&m.expires_at).to_string();
                      if m.active {
                          rsx! {
                              div { class: "mb-6 flex items-center justify-between gap-4 rounded-xl border border-primary/30 bg-primary/5 px-5 py-4",
                                  div {
                                      span { class: "font-bold text-primary", "{t(lang, \"pay.pro\")}" }
                                      span { class: "ml-2 text-sm text-slate-500 dark:text-slate-400", "{t(lang, \"pay.pro_until\")} {date}" }
                                  }
                                  Badge { variant: BadgeVariant::Success, "{t(lang, \"pay.pro_active\")}" }
                              }
                          }
                      } else {
                          rsx! {
                              div { class: "mb-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 px-5 py-4 text-sm text-slate-500",
                                  "{t(lang, \"pay.pro_expired\")} {date}"
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
                      p { class: "text-slate-400", "{t(lang, \"pay.orders.empty\")}" }
                      Link { to: "/course", class: "inline-block mt-4 text-sm font-medium text-primary hover:underline", "{t(lang, \"pay.orders.browse\")}" }
                  }
              } else {
                  div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                      Table {
                          TableHeader { class: "bg-muted/50",
                              TableRow {
                                  TableHead { class: "h-10 px-4", "{t(lang, \"pay.orders.item\")}" }
                                  TableHead { class: "h-10 px-4", "{t(lang, \"pay.orders.amount\")}" }
                                  TableHead { class: "h-10 px-4", "{t(lang, \"pay.orders.channel\")}" }
                                  TableHead { class: "h-10 px-4", "{t(lang, \"pay.orders.status\")}" }
                                  TableHead { class: "h-10 px-4", "{t(lang, \"pay.orders.created\")}" }
                              }
                          }
                          TableBody {
                              for o in orders.into_iter() {
                                  {
                                      let (label, badge) = status_badge(&o.status);
                                      let amount = Price { amount: o.amount, currency: o.currency.clone() }.label();
                                      let chan = t(lang, channel_key(&o.provider));
                                      let date = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                      rsx! {
                                          TableRow { key: "{o.out_trade_no}",
                                              TableCell { class: "px-4 py-2",
                                                  match o.product_kind {
                                                      ProductKind::Course => rsx! {
                                                          Link { to: format!("/course/{}", o.product_ref), class: "hover:text-primary", "{o.product_ref}" }
                                                      },
                                                      ProductKind::Tip => rsx! {
                                                          span { "{t(lang, \"pay.orders.tip\")} · " }
                                                          Link { to: o.product_ref.clone(), class: "text-slate-400 hover:text-primary", "{o.product_ref}" }
                                                      },
                                                  }
                                              }
                                              TableCell { class: "px-4 py-2 font-medium", "{amount}" }
                                              TableCell { class: "px-4 py-2 text-slate-400", "{chan}" }
                                              TableCell { class: "px-4 py-2",
                                                  Badge { variant: badge, "{t(lang, label)}" }
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
