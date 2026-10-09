//! 内容页的「找不到 / 加载失败」状态：服务端渲染时同时设置 HTTP 状态码，
//! 让爬虫与反代看到 404 / 500，而不是 200 的空页面。

use dioxus::prelude::*;

/// 内容不存在：SSR 回 404，并显示 `message`。
#[component]
pub fn NotFound(message: String) -> Element {
  #[cfg(feature = "server")]
  dioxus::fullstack::FullstackContext::commit_http_status(
    dioxus::fullstack::StatusCode::NOT_FOUND,
    None,
  );
  rsx! {
      div { class: "py-20 text-center",
          h1 { class: "text-2xl font-bold text-slate-900 dark:text-white", "{message}" }
      }
  }
}

/// 读取出错：SSR 回 500，只显示通用提示。具体错误在服务端日志里，
/// 不展示给访客（原先会把 server fn 的原始报错直接渲染到页面上）。
#[component]
pub fn LoadFailed() -> Element {
  #[cfg(feature = "server")]
  dioxus::fullstack::FullstackContext::commit_http_status(
    dioxus::fullstack::StatusCode::INTERNAL_SERVER_ERROR,
    None,
  );
  rsx! {
      div { class: "py-20 text-center text-slate-500 dark:text-slate-400", "加载失败，请稍后再试" }
  }
}
