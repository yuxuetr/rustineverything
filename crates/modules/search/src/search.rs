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
use dioxus_shadcn::{
  Alert, AlertDescription, AlertVariant, Button, ButtonSize, ButtonVariant, Command, CommandEmpty,
  CommandInput, CommandItem, CommandList, CommandStatus, Dialog, DialogClose, DialogContent,
  DialogOverlay, DialogTitle, ToggleGroup, ToggleGroupItem,
};

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
      Button {
          r#type: "button",
          variant: ButtonVariant::Outline,
          size: ButtonSize::Sm,
          // min-h-8：Comfortable 密度给 Sm 加 min-h-10，导航栏里会撑高（FB-21）
          class: "min-h-8 gap-2 px-3 text-xs font-normal text-muted-foreground",
          onclick: move |_| open.set(true),
          title: "搜索 (⌘K)",
          svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
              path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2",
                  d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              }
          }
          span { class: "hidden sm:inline", "搜索" }
          kbd { class: "hidden sm:inline px-1.5 py-0.5 rounded bg-background border border-border font-mono text-[10px]",
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

  // 全局快捷键:Cmd+K / Ctrl+K 切换(Esc 由 Dialog 处理)。监听随组件卸载移除。
  use_hook(|| {
    std::rc::Rc::new(widgets::browser::on_document_keydown(move |key, ctrl_or_meta| {
      if ctrl_or_meta && key.eq_ignore_ascii_case("k") {
        open.set(!open());
        return true;
      }
      false
    }))
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
      widgets::browser::sleep_ms(200).await;
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

  let status = if loading() {
    "搜索中...".to_string()
  } else if hits().is_empty() {
    String::new()
  } else {
    format!("{} 条结果 · {} ms", hits().len(), elapsed())
  };

  rsx! {
      Dialog { open: open(), on_open_change: move |v| open.set(v),
          DialogOverlay { class: "z-[100] bg-slate-900/50 backdrop-blur-sm" }
          DialogContent { class: "z-[100] top-24 max-w-2xl translate-y-0 gap-0 overflow-hidden rounded-xl p-0 shadow-2xl",
              DialogTitle { class: "sr-only", "搜索" }
              // Command 只在打开时挂载：它的键盘脚本在祖先带 `hidden` 时启动即退出、
              // 之后不再重启，挂在关闭的 Dialog 里会失去方向键高亮（FB-17）。
              if open() {
                  Command {
                      class: "rounded-none",
                      on_select: move |url: String| {
                          if is_site_path(&url) {
                              open.set(false);
                              widgets::browser::navigate(&url);
                          }
                      },
                      // 输入栏
                      div { class: "flex items-center gap-2 px-4 border-b border-border",
                          svg { class: "w-5 h-5 shrink-0 text-muted-foreground", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", "aria-hidden": "true",
                              path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2",
                                  d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                              }
                          }
                          CommandInput {
                              value: query(),
                              placeholder: "搜索博客、文档、论坛、案例、专题...",
                              class: "h-12 px-0 text-base",
                              oninput: move |e: FormEvent| query.set(e.value()),
                          }
                          DialogClose { class: "static shrink-0 px-2 py-1 text-xs text-muted-foreground", "Esc" }
                      }
                      // kind 过滤栏:受控单选,再点已选项时的空值忽略,保证总有一项选中。
                      div { class: "flex items-center gap-1 px-4 py-2 border-b border-border text-xs",
                          ToggleGroup {
                              class: "gap-1",
                              "aria-label": "内容类型",
                              value: kind_filter().unwrap_or_else(|| "all".to_string()),
                              on_value_change: move |v: String| match v.as_str() {
                                  "" => {}
                                  "all" => kind_filter.set(None),
                                  _ => kind_filter.set(Some(v)),
                              },
                              for (value, label) in KINDS {
                                  ToggleGroupItem { value: *value, class: kind_chip_class(kind_filter().as_deref().unwrap_or("all") == *value), "{label}" }
                              }
                          }
                          span { class: "ml-auto text-muted-foreground", "aria-hidden": "true", "{status}" }
                      }
                      CommandStatus { "{status}" }
                      // 错误
                      if let Some(err) = error() {
                          Alert { variant: AlertVariant::Destructive, class: "rounded-none border-x-0 px-4 py-2",
                              AlertDescription { variant: AlertVariant::Destructive, "{err}" }
                          }
                      }
                      // 结果
                      CommandList { class: "max-h-[60vh]",
                          if hits().is_empty() && !query().trim().is_empty() && !loading() {
                              CommandEmpty { "没有匹配的结果" }
                          } else if hits().is_empty() {
                              CommandEmpty { "输入关键词开始搜索 · 支持中英文 · 按 ⌘K 随时打开" }
                          } else {
                              for (i, h) in hits().iter().enumerate() {
                                  CommandItem { key: "{h.url}", id: "search-hit-{i}", value: h.url.clone(), class: "block rounded-none border-b border-border px-4 py-3 cursor-pointer",
                                      HitRow { hit: h.clone() }
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

/// 过滤栏的类型:值 `all` 表示不过滤。
const KINDS: &[(&str, &str)] =
  &[("all", "全部"), ("blog", "博客"), ("doc", "文档"), ("topic", "话题"), ("case", "案例")];

fn kind_chip_class(active: bool) -> &'static str {
  if active {
    "h-auto rounded-full bg-primary px-2.5 py-0.5 text-xs font-normal text-primary-foreground hover:bg-primary/90"
  } else {
    "h-auto rounded-full bg-secondary px-2.5 py-0.5 text-xs font-normal text-secondary-foreground hover:bg-accent"
  }
}

#[component]
fn HitRow(hit: SearchHit) -> Element {
  let badge = match hit.kind.as_str() {
    "blog" => ("BLOG", "bg-sky-100 dark:bg-sky-900/40 text-sky-700 dark:text-sky-300"),
    "doc" => {
      ("DOC", "bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300")
    }
    "topic" => {
      ("TOPIC", "bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300")
    }
    "case" => ("CASE", "bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300"),
    // Phase 6 内容板块：统一靛蓝徽章
    "embedded" => {
      ("嵌入式", "bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300")
    }
    "ai" => ("AI", "bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300"),
    "web3" => ("WEB3", "bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300"),
    "wasm" => ("WASM", "bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300"),
    "cli" => ("CLI", "bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300"),
    _ => ("?", "bg-slate-100 dark:bg-slate-800 text-slate-500"),
  };
  rsx! {
      div {
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

/// 搜索结果只跳站内路径：以 `/` 开头，且第二个字符不是 `/` 或 `\`
/// （浏览器把 `//host`、`/\host` 都当作协议相对地址，会跳出站点）。
fn is_site_path(url: &str) -> bool {
  let mut chars = url.chars();
  chars.next() == Some('/') && !matches!(chars.next(), Some('/' | '\\'))
}

#[cfg(test)]
mod tests {
  use super::is_site_path;

  #[test]
  fn accepts_site_paths() {
    assert!(is_site_path("/blog/hello"));
    assert!(is_site_path("/"));
    assert!(is_site_path("/docs/rust-basics#intro"));
  }

  #[test]
  fn rejects_paths_that_leave_the_site() {
    for url in [
      "",
      "//evil.example",
      "/\\evil.example",
      "https://evil.example",
      "javascript:alert(1)",
      "blog/x",
    ] {
      assert!(!is_site_path(url), "{url}");
    }
  }
}
