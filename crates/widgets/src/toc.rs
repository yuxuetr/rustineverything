//! 页内目录（T2）：正文 h2 / h3 的锚点列表，滚动时高亮当前标题。
//! 标题来自 [`crate::mdx::heading_outline`]，高亮由 signal 渲染，不用脚本改 Dioxus 管着的节点。

use dioxus::prelude::*;

use crate::browser::{current_heading, on_window_scroll, ScrollListener};
use crate::mdx::TocEntry;

/// 标题少于这个数时不显示目录：短文不需要。版式要为目录留栏的调用方也按它判断。
pub const MIN_ENTRIES: usize = 3;
/// 标题顶边离视口顶部不超过这么多像素就算「当前」：sticky 顶栏 3.5rem 加一点余量。
const ACTIVE_OFFSET_PX: f64 = 96.0;

/// 页内目录。`title` 由调用方按语言传入。少于 3 个标题时不渲染。
#[component]
pub fn OnThisPage(entries: Vec<TocEntry>, title: String) -> Element {
  let mut active = use_signal(String::new);
  let mut listener = use_signal(|| None::<ScrollListener>);
  let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
  // 换页（SPA）时组件不重建：随标题列表重新注册，旧监听随之 drop。
  use_effect(use_reactive!(|ids| {
    if ids.len() < MIN_ENTRIES {
      listener.set(None);
      return;
    }
    active.set(current_heading(&ids, ACTIVE_OFFSET_PX).unwrap_or_default());
    listener.set(Some(on_window_scroll(move || {
      if let Some(id) = current_heading(&ids, ACTIVE_OFFSET_PX) {
        if *active.peek() != id {
          active.set(id);
        }
      }
    })));
  }));

  if entries.len() < MIN_ENTRIES {
    return VNode::empty();
  }
  rsx! {
      nav { "aria-label": "{title}", class: "text-sm",
          p { class: "mb-3 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400",
              "{title}"
          }
          ul { class: "space-y-1 border-l border-slate-200 dark:border-slate-800",
              for entry in entries.iter() {
                  li {
                      a {
                          href: "#{entry.id}",
                          class: format_args!(
                              "-ml-px block border-l-2 py-1 leading-snug transition-colors {} {}",
                              if entry.level == 3 { "pl-6" } else { "pl-3" },
                              if active() == entry.id {
                                  "border-primary text-primary"
                              } else {
                                  "border-transparent text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white"
                              },
                          ),
                          "{entry.text}"
                      }
                  }
              }
          }
      }
  }
}
