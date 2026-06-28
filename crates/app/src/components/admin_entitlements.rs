//! Admin：课程权益手动授权页（M4d）。
//!
//! 放在 app 组合根（而非 admin 模块）：复用 admin 模块的公共外壳 `AdminShell`，
//! 同时调用 course 模块的权益 server fn —— 避免 admin 反向依赖 course。
//! 支付网关接入前（M5），运营可在此手动为用户开通课程（线下售卖后开通）。

use dioxus::prelude::*;

use module_admin::admin::{is_current_user_admin, AdminShell, ForbiddenPanel};
use module_course::server::{
  grant_entitlement, list_entitlements, revoke_entitlement, EntitlementInfo,
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

  let input_class = "rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-2 text-sm text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500";

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
                  input {
                      class: "{input_class} sm:w-40",
                      r#type: "number",
                      placeholder: "用户 ID",
                      value: "{user_id}",
                      oninput: move |e| user_id.set(e.value()),
                  }
                  input {
                      class: "{input_class} flex-1",
                      placeholder: "课程 slug，如 rust-basics",
                      value: "{course_slug}",
                      oninput: move |e| course_slug.set(e.value()),
                  }
                  button {
                      class: "rounded-lg bg-blue-600 hover:bg-blue-700 px-4 py-2 text-sm font-semibold text-white whitespace-nowrap transition-colors",
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
                  div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
              }
          } else if rows.is_empty() {
              p { class: "text-center text-slate-400 py-10", "暂无任何权益记录。" }
          } else {
              div { class: "overflow-x-auto rounded-xl border border-slate-200 dark:border-slate-800",
                  table { class: "w-full text-sm",
                      thead { class: "bg-slate-50 dark:bg-slate-900/60 text-slate-500 dark:text-slate-400",
                          tr {
                              th { class: "text-left font-medium px-4 py-2", "用户" }
                              th { class: "text-left font-medium px-4 py-2", "课程" }
                              th { class: "text-left font-medium px-4 py-2", "来源" }
                              th { class: "text-left font-medium px-4 py-2", "授予时间" }
                              th { class: "px-4 py-2" }
                          }
                      }
                      tbody { class: "divide-y divide-slate-100 dark:divide-slate-800",
                          for r in rows.into_iter() {
                              {
                                  let uid = r.user_id;
                                  let slug = r.course_slug.clone();
                                  rsx! {
                                      tr { key: "{r.user_id}-{r.course_slug}", class: "text-slate-700 dark:text-slate-200",
                                          td { class: "px-4 py-2", "{r.nickname} #{r.user_id}" }
                                          td { class: "px-4 py-2 font-mono text-xs", "{r.course_slug}" }
                                          td { class: "px-4 py-2 text-slate-400", "{r.source}" }
                                          td { class: "px-4 py-2 text-slate-400 text-xs", "{r.granted_at}" }
                                          td { class: "px-4 py-2 text-right",
                                              button {
                                                  class: "text-xs font-medium text-rose-600 hover:text-rose-700",
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
      }
  }
}
