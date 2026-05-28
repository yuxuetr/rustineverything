//! Dioxus 搜索 UI:`SearchButton`(导航栏入口) + `SearchModal`(全屏模态)。
//!
//! 触发方式:
//! - 点击导航栏放大镜按钮
//! - `Cmd+K` / `Ctrl+K` 全局快捷键
//!
//! 状态共享:`use_context_provider::<Signal<bool>>` 用同一个 Signal 表示
//! "是否打开搜索模态"。在 App 根处 provide 一次,Navbar 与 Layout 共享。

use crate::server::{search_query, SearchHit};
use dioxus::prelude::*;

/// 用 wrapper 类型避免与其他全局 `Signal<bool>`(如 auth modal)冲突。
#[derive(Clone, Copy)]
pub struct SearchOpen(pub Signal<bool>);

/// 在 App 根注入搜索 modal 开关并返回 Signal,Navbar 等可以读取或修改它。
pub fn use_search_open_provider() -> Signal<bool> {
  let sig = use_signal(|| false);
  use_context_provider(|| SearchOpen(sig));
  sig
}

/// 子组件读取该 Signal。
pub fn use_search_open() -> Option<Signal<bool>> {
  try_use_context::<SearchOpen>().map(|w| w.0)
}

/// 导航栏右上角的搜索按钮(放大镜 + 「⌘K」提示)。
#[component]
pub fn SearchButton() -> Element {
  let mut open = match use_search_open() {
    Some(o) => o,
    None => return rsx! {},
  };
  rsx! {
      button {
          onclick: move |_| open.set(true),
          class: "inline-flex items-center gap-2 px-3 h-8 rounded-md border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/50 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 text-xs",
          title: "搜索 (⌘K)",
          svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
              path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2",
                  d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              }
          }
          span { class: "hidden sm:inline", "搜索" }
          kbd { class: "hidden sm:inline px-1.5 py-0.5 rounded bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 font-mono text-[10px]",
              "⌘K"
          }
      }
  }
}

/// 全局键盘监听 + 搜索模态框。在 App 根挂一次。
#[component]
pub fn SearchModal() -> Element {
  let mut open = match use_search_open() {
    Some(o) => o,
    None => return rsx! {},
  };
  let mut query = use_signal(String::new);
  let mut kind_filter = use_signal::<Option<String>>(|| None);
  let mut hits = use_signal::<Vec<SearchHit>>(Vec::new);
  let mut loading = use_signal(|| false);
  let mut elapsed = use_signal(|| 0u64);
  let mut error = use_signal::<Option<String>>(|| None);

  // 全局快捷键:Cmd+K / Ctrl+K 切换;Esc 关闭。
  use_effect(move || {
    let script = r#"
            window.__rie_search_listener = window.__rie_search_listener || (function() {
                document.addEventListener('keydown', (e) => {
                    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
                        e.preventDefault();
                        window.dispatchEvent(new CustomEvent('rie-search-toggle'));
                    }
                    if (e.key === 'Escape') {
                        window.dispatchEvent(new CustomEvent('rie-search-close'));
                    }
                });
                return true;
            })();
        "#;
    let _ = dioxus::document::eval(script);
  });

  // 通过 dioxus.recv 拉取浏览器自定义事件
  use_effect(move || {
    spawn(async move {
      let mut e = dioxus::document::eval(
        r#"
                window.addEventListener('rie-search-toggle', () => dioxus.send('toggle'));
                window.addEventListener('rie-search-close', () => dioxus.send('close'));
                "#,
      );
      loop {
        match e.recv::<String>().await {
          Ok(msg) => match msg.as_str() {
            "toggle" => open.set(!open()),
            "close" => open.set(false),
            _ => {}
          },
          Err(_) => break,
        }
      }
    });
  });

  // 输入变化触发搜索(简单 debounce 由前端计数器实现)
  let mut debounce_token = use_signal(|| 0u64);
  let _ = use_effect(move || {
    let q = query();
    let k = kind_filter();
    // 只在打开且非空时查询
    if !open() {
      hits.set(Vec::new());
      return;
    }
    if q.trim().is_empty() {
      hits.set(Vec::new());
      elapsed.set(0);
      return;
    }
    debounce_token.with_mut(|n| *n = n.wrapping_add(1));
    let token = debounce_token();
    spawn(async move {
      // 简单 debounce:延迟 200ms,期间若 token 改变则放弃。
      sleep_ms(200).await;
      if debounce_token() != token {
        return;
      }
      loading.set(true);
      error.set(None);
      match search_query(q.clone(), k.clone(), Some(20)).await {
        Ok(resp) => {
          hits.set(resp.hits);
          elapsed.set(resp.elapsed_ms);
        }
        Err(e) => {
          hits.set(Vec::new());
          error.set(Some(format!("搜索失败: {}", e)));
        }
      }
      loading.set(false);
    });
  });

  if !open() {
    return rsx! {};
  }

  rsx! {
      div {
          class: "fixed inset-0 z-[100] flex items-start justify-center pt-24 px-4 bg-slate-900/50 backdrop-blur-sm",
          onclick: move |_| open.set(false),
          div {
              class: "w-full max-w-2xl rounded-xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-2xl overflow-hidden",
              onclick: move |e| e.stop_propagation(),
              // 输入栏
              div { class: "flex items-center gap-2 px-4 py-3 border-b border-slate-200 dark:border-slate-700",
                  svg { class: "w-5 h-5 text-slate-400", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2",
                          d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                      }
                  }
                  input {
                      r#type: "text",
                      value: "{query}",
                      autofocus: true,
                      placeholder: "搜索博客、文档、论坛、案例...",
                      class: "flex-1 bg-transparent border-0 focus:ring-0 outline-none text-base text-slate-900 dark:text-slate-100 placeholder-slate-400",
                      oninput: move |e| query.set(e.value()),
                  }
                  button {
                      onclick: move |_| open.set(false),
                      class: "shrink-0 px-2 py-1 text-xs rounded text-slate-400 hover:text-slate-600 dark:hover:text-slate-200",
                      "Esc"
                  }
              }
              // kind 过滤栏
              div { class: "flex items-center gap-1 px-4 py-2 border-b border-slate-100 dark:border-slate-800 text-xs",
                  KindChip { label: "全部".to_string(), value: None, current: kind_filter(), on_select: move |v: Option<String>| kind_filter.set(v) }
                  KindChip { label: "博客".to_string(), value: Some("blog".to_string()), current: kind_filter(), on_select: move |v: Option<String>| kind_filter.set(v) }
                  KindChip { label: "文档".to_string(), value: Some("doc".to_string()), current: kind_filter(), on_select: move |v: Option<String>| kind_filter.set(v) }
                  KindChip { label: "话题".to_string(), value: Some("topic".to_string()), current: kind_filter(), on_select: move |v: Option<String>| kind_filter.set(v) }
                  KindChip { label: "案例".to_string(), value: Some("case".to_string()), current: kind_filter(), on_select: move |v: Option<String>| kind_filter.set(v) }
                  if loading() {
                      span { class: "ml-auto text-slate-400", "搜索中..." }
                  } else if !hits().is_empty() {
                      span { class: "ml-auto text-slate-400", "{hits().len()} 条结果 · {elapsed} ms" }
                  }
              }
              // 错误
              if let Some(err) = error() {
                  div { class: "px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400",
                      "{err}"
                  }
              }
              // 结果
              div { class: "max-h-[60vh] overflow-y-auto",
                  if hits().is_empty() && !query().trim().is_empty() && !loading() {
                      div { class: "px-4 py-8 text-center text-slate-500", "没有匹配的结果" }
                  } else if hits().is_empty() {
                      div { class: "px-4 py-8 text-center text-slate-500",
                          "输入关键词开始搜索 · 支持中英文 · 按 ⌘K 随时打开"
                      }
                  } else {
                      for h in hits().iter() {
                          HitRow { hit: h.clone() }
                      }
                  }
              }
          }
      }
  }
}

#[component]
fn KindChip(
  label: String,
  value: Option<String>,
  current: Option<String>,
  on_select: EventHandler<Option<String>>,
) -> Element {
  let is_active = value == current;
  let class = if is_active {
    "px-2 py-0.5 rounded-full bg-blue-600 text-white"
  } else {
    "px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700"
  };
  rsx! {
      button {
          class: "{class}",
          onclick: move |_| on_select.call(value.clone()),
          "{label}"
      }
  }
}

#[component]
fn HitRow(hit: SearchHit) -> Element {
  let badge = match hit.kind.as_str() {
    "blog" => ("BLOG", "bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300"),
    "doc" => {
      ("DOC", "bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300")
    }
    "topic" => {
      ("TOPIC", "bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300")
    }
    "case" => ("CASE", "bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300"),
    _ => ("?", "bg-slate-100 dark:bg-slate-800 text-slate-500"),
  };
  rsx! {
      a {
          href: "{hit.url}",
          class: "block px-4 py-3 border-b border-slate-100 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50 transition-colors",
          div { class: "flex items-center gap-2 mb-1",
              span { class: "text-[10px] px-1.5 py-0.5 rounded font-medium uppercase tracking-wide {badge.1}",
                  "{badge.0}"
              }
              span { class: "text-xs text-slate-400 truncate", "{hit.url}" }
              if !hit.created_at.is_empty() {
                  span { class: "text-xs text-slate-400", "· {hit.created_at}" }
              }
          }
          div { class: "text-sm font-semibold text-slate-900 dark:text-white truncate", "{hit.title}" }
          if !hit.snippet.is_empty() {
              p { class: "mt-1 text-xs text-slate-600 dark:text-slate-400 line-clamp-2",
                  "{hit.snippet}"
              }
          }
      }
  }
}

/// 简单 sleep helper for debouncing(基于 setTimeout)。
async fn sleep_ms(ms: u32) {
  let script = format!("setTimeout(() => dioxus.send(true), {});", ms);
  let mut e = dioxus::document::eval(&script);
  let _ = e.recv::<bool>().await;
}
