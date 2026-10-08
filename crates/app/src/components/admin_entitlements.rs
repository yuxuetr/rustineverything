//! Admin：课程权益手动授权页（M4d）。
//!
//! 放在 app 组合根（而非 admin 模块）：复用 admin 模块的公共外壳 `AdminShell`，
//! 同时调用 course 模块的权益 server fn —— 避免 admin 反向依赖 course。
//! 支付网关接入前（M5），运营可在此手动为用户开通课程（线下售卖后开通）。

use dioxus::prelude::*;
use dioxus_shadcn::{
  Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Input, Spinner, SpinnerSize, Table,
  TableBody, TableCell, TableHead, TableHeader, TableRow,
};

use module_admin::admin::{is_current_user_admin, AdminShell, ForbiddenPanel};
use module_course::server::{
  grant_entitlement, grant_membership, list_entitlements, list_memberships, revoke_entitlement,
  revoke_membership, EntitlementInfo, MembershipAdminInfo,
};

/// `/admin/entitlements`：列出全部权益 + 手动授予 / 撤销。
#[component]
pub fn AdminEntitlementsPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  // admin 后台一律 use_resource（鉴权 + 强交互，非 SEO）。
  let mut rows_res = use_resource(|| async move { list_entitlements().await.unwrap_or_default() });
  let rows: Vec<EntitlementInfo> = rows_res.read().clone().unwrap_or_default();
  let loaded = rows_res.read().is_some();

  let mut user_id = use_signal(String::new);
  let mut course_slug = use_signal(String::new);
  let mut msg = use_signal(String::new);

  let do_grant = move |_| {
    let parsed = user_id().trim().parse::<i32>();
    let slug = course_slug().trim().to_string();
    match parsed {
      Ok(id) if !slug.is_empty() => {
        spawn(async move {
          match grant_entitlement(id, slug).await {
            Ok(_) => {
              msg.set("已授予".to_string());
              user_id.set(String::new());
              course_slug.set(String::new());
              rows_res.restart();
            }
            Err(e) => msg.set(format!("失败：{e}")),
          }
        });
      }
      _ => msg.set("请输入有效的用户 ID 与课程 slug".to_string()),
    }
  };

  rsx! {
      AdminShell { active: "entitlements".to_string(),
          h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-2", "课程权益" }
          p { class: "text-sm text-slate-500 dark:text-slate-400 mb-6",
              "为用户手动开通课程访问权益（线下售卖 / 优惠后开通）。在线支付接入后将自动写入。"
          }

          // 授予表单
          div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5 mb-8",
              h2 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200 mb-3", "授予权益" }
              div { class: "flex flex-col sm:flex-row gap-3",
                  Input {
                      class: "sm:w-40",
                      r#type: "number",
                      placeholder: "用户 ID",
                      value: user_id(),
                      on_value_change: move |v: String| user_id.set(v),
                  }
                  Input {
                      class: "flex-1",
                      placeholder: "课程 slug，如 rust-basics",
                      value: course_slug(),
                      on_value_change: move |v: String| course_slug.set(v),
                  }
                  Button {
                      class: "whitespace-nowrap",
                      onclick: do_grant,
                      "授予"
                  }
              }
              if !msg().is_empty() {
                  p { class: "mt-3 text-sm text-slate-600 dark:text-slate-300", "{msg}" }
              }
          }

          // 权益列表
          if !loaded {
              div { class: "flex items-center justify-center py-16",
                  Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
              }
          } else if rows.is_empty() {
              p { class: "text-center text-slate-400 py-10", "暂无任何权益记录。" }
          } else {
              div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                  Table {
                      TableHeader { class: "bg-muted/50",
                          TableRow {
                              TableHead { class: "h-10 px-4", "用户" }
                              TableHead { class: "h-10 px-4", "课程" }
                              TableHead { class: "h-10 px-4", "来源" }
                              TableHead { class: "h-10 px-4", "授予时间" }
                              TableHead { class: "h-10 px-4" }
                          }
                      }
                      TableBody {
                          for r in rows.into_iter() {
                              {
                                  let uid = r.user_id;
                                  let slug = r.course_slug.clone();
                                  rsx! {
                                      TableRow { key: "{r.user_id}-{r.course_slug}",
                                          TableCell { class: "px-4 py-2", "{r.nickname} #{r.user_id}" }
                                          TableCell { class: "px-4 py-2 font-mono text-xs", "{r.course_slug}" }
                                          TableCell { class: "px-4 py-2 text-slate-400", "{r.source}" }
                                          TableCell { class: "px-4 py-2 text-slate-400 text-xs", "{r.granted_at}" }
                                          TableCell { class: "px-4 py-2 text-right",
                                              Button {
                                                  variant: ButtonVariant::Ghost,
                                                  size: ButtonSize::Sm,
                                                  class: "text-xs text-destructive hover:bg-destructive/10",
                                                  onclick: move |_| {
                                                      let slug = slug.clone();
                                                      spawn(async move {
                                                          let _ = revoke_entitlement(uid, slug).await;
                                                          rows_res.restart();
                                                      });
                                                  },
                                                  "撤销"
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

          MembershipSection {}

          OrdersSection {}
      }
  }
}

/// 订单管理区块（M5e：最近订单 + 已支付订单全额退款）。
/// `payments` feature 关闭时不渲染（订单 server fns 不存在）。
#[component]
fn OrdersSection() -> Element {
  #[cfg(not(feature = "payments"))]
  {
    rsx! {}
  }
  #[cfg(feature = "payments")]
  {
    use module_course::server::{admin_list_orders, admin_refund_order, AdminOrderInfo};

    let mut rows_res =
      use_resource(|| async move { admin_list_orders().await.unwrap_or_default() });
    let rows: Vec<AdminOrderInfo> = rows_res.read().clone().unwrap_or_default();
    let loaded = rows_res.read().is_some();
    let mut msg = use_signal(String::new);

    rsx! {
        div { class: "mt-12",
            h2 { class: "text-lg font-bold text-slate-900 dark:text-white mb-2", "订单 / 退款" }
            p { class: "text-sm text-slate-500 dark:text-slate-400 mb-4",
                "最近 100 笔订单。已支付订单可全额退款：网关受理后订单置为已退款并撤销购买来源的课程权益（微信为异步到账）。"
            }
            if !msg().is_empty() {
                p { class: "mb-3 text-sm text-slate-600 dark:text-slate-300", "{msg}" }
            }
            if !loaded {
                div { class: "flex items-center justify-center py-8",
                    Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                }
            } else if rows.is_empty() {
                p { class: "text-center text-slate-400 py-6", "暂无订单。" }
            } else {
                div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                    Table {
                        TableHeader { class: "bg-muted/50",
                            TableRow {
                                TableHead { class: "h-10 px-4", "订单号" }
                                TableHead { class: "h-10 px-4", "用户" }
                                TableHead { class: "h-10 px-4", "课程" }
                                TableHead { class: "h-10 px-4", "金额" }
                                TableHead { class: "h-10 px-4", "渠道" }
                                TableHead { class: "h-10 px-4", "状态" }
                                TableHead { class: "h-10 px-4" }
                            }
                        }
                        TableBody {
                            for o in rows.into_iter() {
                                {
                                    let otn = o.out_trade_no.clone();
                                    let yuan = o.amount / 100;
                                    let chan = if o.provider == "alipay" { "支付宝" } else { "微信" };
                                    let paid = o.status == "paid";
                                    rsx! {
                                        TableRow { key: "{o.out_trade_no}",
                                            TableCell { class: "px-4 py-2 font-mono text-xs", "{o.out_trade_no}" }
                                            TableCell { class: "px-4 py-2", "{o.nickname} #{o.user_id}" }
                                            TableCell { class: "px-4 py-2 font-mono text-xs", "{o.course_slug}" }
                                            TableCell { class: "px-4 py-2 font-medium", "¥{yuan}" }
                                            TableCell { class: "px-4 py-2 text-slate-400", "{chan}" }
                                            TableCell { class: "px-4 py-2 text-xs", "{o.status}" }
                                            TableCell { class: "px-4 py-2 text-right",
                                                if paid {
                                                    Button {
                                                        variant: ButtonVariant::Ghost,
                                                        size: ButtonSize::Sm,
                                                        class: "text-xs text-destructive hover:bg-destructive/10",
                                                        onclick: move |_| {
                                                            let otn = otn.clone();
                                                            spawn(async move {
                                                                match admin_refund_order(otn.clone()).await {
                                                                    Ok(_) => {
                                                                        msg.set(format!("订单 {otn} 已退款"));
                                                                        rows_res.restart();
                                                                    }
                                                                    Err(e) => msg.set(format!("退款失败：{e}")),
                                                                }
                                                            });
                                                        },
                                                        "全额退款"
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
    }
  }
}

/// Pro 会员管理区块（授予/续期 + 列表 + 撤销）。
#[component]
fn MembershipSection() -> Element {
  let mut rows_res = use_resource(|| async move { list_memberships().await.unwrap_or_default() });
  let rows: Vec<MembershipAdminInfo> = rows_res.read().clone().unwrap_or_default();
  let loaded = rows_res.read().is_some();

  let mut user_id = use_signal(String::new);
  let mut days = use_signal(|| "30".to_string());
  let mut msg = use_signal(String::new);

  let do_grant = move |_| {
    let uid = user_id().trim().parse::<i32>();
    let d = days().trim().parse::<i64>();
    match (uid, d) {
      (Ok(id), Ok(n)) if n > 0 => {
        spawn(async move {
          match grant_membership(id, n).await {
            Ok(_) => {
              msg.set("已开通/续期".to_string());
              user_id.set(String::new());
              rows_res.restart();
            }
            Err(e) => msg.set(format!("失败：{e}")),
          }
        });
      }
      _ => msg.set("请输入有效的用户 ID 与天数".to_string()),
    }
  };

  rsx! {
      div { class: "mt-12",
          h2 { class: "text-lg font-bold text-slate-900 dark:text-white mb-2", "Pro 会员" }
          p { class: "text-sm text-slate-500 dark:text-slate-400 mb-4",
              "为用户开通 / 续期 Pro 会员（解锁全部 pro 课程）。在已有有效期或当前时间上叠加天数。"
          }
          div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5 mb-6",
              div { class: "flex flex-col sm:flex-row gap-3",
                  Input {
                      class: "sm:w-40",
                      r#type: "number",
                      placeholder: "用户 ID",
                      value: user_id(),
                      on_value_change: move |v: String| user_id.set(v),
                  }
                  Input {
                      class: "sm:w-32",
                      r#type: "number",
                      placeholder: "天数",
                      value: days(),
                      on_value_change: move |v: String| days.set(v),
                  }
                  Button {
                      class: "whitespace-nowrap",
                      onclick: do_grant,
                      "开通 / 续期"
                  }
              }
              if !msg().is_empty() {
                  p { class: "mt-3 text-sm text-slate-600 dark:text-slate-300", "{msg}" }
              }
          }

          if !loaded {
              div { class: "flex items-center justify-center py-8",
                  Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
              }
          } else if rows.is_empty() {
              p { class: "text-center text-slate-400 py-6", "暂无会员记录。" }
          } else {
              div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                  Table {
                      TableHeader { class: "bg-muted/50",
                          TableRow {
                              TableHead { class: "h-10 px-4", "用户" }
                              TableHead { class: "h-10 px-4", "层级" }
                              TableHead { class: "h-10 px-4", "到期" }
                              TableHead { class: "h-10 px-4", "状态" }
                              TableHead { class: "h-10 px-4" }
                          }
                      }
                      TableBody {
                          for m in rows.into_iter() {
                              {
                                  let uid = m.user_id;
                                  let date = m.expires_at.split('T').next().unwrap_or(&m.expires_at).to_string();
                                  rsx! {
                                      TableRow { key: "{m.user_id}",
                                          TableCell { class: "px-4 py-2", "{m.nickname} #{m.user_id}" }
                                          TableCell { class: "px-4 py-2 uppercase text-xs font-semibold", "{m.tier}" }
                                          TableCell { class: "px-4 py-2 text-slate-400 text-xs", "{date}" }
                                          TableCell { class: "px-4 py-2",
                                              if m.active {
                                                  Badge { variant: BadgeVariant::Success, "有效" }
                                              } else {
                                                  Badge { variant: BadgeVariant::Secondary, "已过期" }
                                              }
                                          }
                                          TableCell { class: "px-4 py-2 text-right",
                                              Button {
                                                  variant: ButtonVariant::Ghost,
                                                  size: ButtonSize::Sm,
                                                  class: "text-xs text-destructive hover:bg-destructive/10",
                                                  onclick: move |_| {
                                                      spawn(async move {
                                                          let _ = revoke_membership(uid).await;
                                                          rows_res.restart();
                                                      });
                                                  },
                                                  "撤销"
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
}
