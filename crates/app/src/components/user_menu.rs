//! 导航栏用户菜单：头像按钮 + 下拉（我的话题 / 标注 / 订单、管理后台、退出）。
//!
//! Classic 与 Minimal 两个布局共用；`compact` 切换 Minimal 的小头像、不显示昵称。

use app_core::session::SessionUser;
use dioxus::prelude::*;
use dioxus_shadcn::{
  Dropdown, DropdownContent, DropdownItem, DropdownLabel, DropdownSeparator, DropdownTrigger,
};

use crate::i18n::{t, use_i18n};
use crate::routes::Route;

#[component]
pub fn UserMenu(user: SessionUser, compact: bool, show_my_topics: bool) -> Element {
  let lang = use_i18n();
  let nav = navigator();
  let avatar_size = if compact { "w-6 h-6" } else { "w-7 h-7" };
  let avatar_px = if compact { "24" } else { "28" };
  let initial = user.nickname.chars().next().unwrap_or('U');

  rsx! {
      Dropdown {
          DropdownTrigger {
              class: if compact { "flex items-center gap-1.5 px-1.5 py-1 rounded-lg hover:bg-accent transition-colors" } else { "flex items-center gap-2 px-2 py-1 rounded-lg hover:bg-accent transition-colors" },
              "aria-label": "{user.nickname}",
              if let Some(ref avatar) = user.avatar_url {
                  img {
                      src: "{avatar}",
                      class: "{avatar_size} shrink-0 rounded-full object-cover",
                      width: avatar_px,
                      height: avatar_px,
                      alt: ""
                  }
              } else {
                  div { class: "{avatar_size} shrink-0 rounded-full bg-primary flex items-center justify-center text-primary-foreground text-xs font-bold",
                      "aria-hidden": "true",
                      "{initial}"
                  }
              }
              if !compact {
                  span { class: "hidden sm:inline text-sm font-medium text-foreground", "{user.nickname}" }
              }
          }
          DropdownContent { class: "w-44",
              DropdownLabel { class: "text-xs font-normal", "{user.nickname}" }
              DropdownSeparator {}
              if show_my_topics {
                  DropdownItem { onclick: move |_| { nav.push(Route::MyTopics {}); }, "{t(lang(), \"user.my_topics\")}" }
              }
              DropdownItem { onclick: move |_| { nav.push(Route::MyAnnotations {}); }, "{t(lang(), \"user.my_annotations\")}" }
              // PM4：在线支付关闭时隐藏「我的订单」入口（路由保留）。
              if cfg!(feature = "payments") {
                  DropdownItem { onclick: move |_| { nav.push(Route::MyOrders {}); }, "{t(lang(), \"user.my_orders\")}" }
              }
              if user.is_admin() {
                  DropdownSeparator {}
                  DropdownItem { class: "font-semibold text-primary", onclick: move |_| { nav.push(Route::AdminDashboard {}); },
                      "{t(lang(), \"nav.admin\")}"
                  }
              }
              DropdownSeparator {}
              // 退出走服务端路由清 cookie，需要整页跳转；该路由只收 POST（SEC-09）。
              DropdownItem { onclick: move |_| widgets::browser::post_navigate("/api/auth/logout"), "{t(lang(), \"auth.logout\")}" }
          }
      }
  }
}
