use super::markdown::Markdown;
use dioxus::prelude::*;

#[component]
pub fn CommentBox() -> Element {
  let mut content = use_signal(|| String::new());
  let mut preview_mode = use_signal(|| false);

  rsx! {
      div { class: "mt-12 rounded-xl border border-slate-200 bg-slate-50 p-6 dark:bg-slate-900/50 dark:border-slate-800",
          div { class: "mb-4 flex items-center justify-between",
              h3 { class: "text-lg font-semibold text-slate-900 dark:text-white", "发表评论" }
              div { class: "flex space-x-2 bg-slate-200 dark:bg-slate-800 rounded-lg p-1",
                  button {
                      class: if !preview_mode() {
                          "px-3 py-1 text-sm font-medium rounded-md bg-white text-slate-900 shadow dark:bg-slate-700 dark:text-white transition-all"
                      } else {
                          "px-3 py-1 text-sm font-medium rounded-md text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-white transition-all"
                      },
                      onclick: move |_| preview_mode.set(false),
                      "编辑"
                  }
                  button {
                      class: if preview_mode() {
                          "px-3 py-1 text-sm font-medium rounded-md bg-white text-slate-900 shadow dark:bg-slate-700 dark:text-white transition-all"
                      } else {
                          "px-3 py-1 text-sm font-medium rounded-md text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-white transition-all"
                      },
                      onclick: move |_| preview_mode.set(true),
                      "预览"
                  }
              }
          }

          if preview_mode() {
              div { class: "min-h-[150px] p-4 bg-white dark:bg-slate-950 rounded-lg border border-slate-300 dark:border-slate-700",
                  if content().trim().is_empty() {
                      span { class: "text-slate-400 italic", "Nothing to preview" }
                  } else {
                      Markdown { content: content() }
                  }
              }
          } else {
              div { class: "space-y-2",
                  // Toolbar
                  div { class: "flex gap-2 text-sm text-slate-600 dark:text-slate-400",
                      button {
                          class: "hover:text-blue-600 dark:hover:text-blue-400",
                          onclick: move |_| {
                              let current = content();
                              content.set(format!("{}**bold**", current));
                          },
                          "Bold"
                      }
                      button {
                          class: "hover:text-blue-600 dark:hover:text-blue-400",
                          onclick: move |_| {
                              let current = content();
                              content.set(format!("{}*italic*", current));
                          },
                          "Italic"
                      }
                      button {
                          class: "hover:text-blue-600 dark:hover:text-blue-400",
                          onclick: move |_| {
                              let current = content();
                              content.set(format!("{} [Link text](url)", current));
                          },
                          "Link"
                      }
                      button {
                          class: "hover:text-blue-600 dark:hover:text-blue-400",
                          onclick: move |_| {
                              let current = content();
                              content.set(format!("{} ![Image alt](url)", current));
                          },
                          "Image"
                      }
                  }
                  textarea {
                      class: "w-full min-h-[150px] p-4 rounded-lg border border-slate-300 bg-white text-slate-900 focus:border-blue-500 focus:ring-blue-500 dark:bg-slate-950 dark:border-slate-700 dark:text-white",
                      placeholder: "支持 Markdown 语法...",
                      value: "{content}",
                      oninput: move |e| content.set(e.value())
                  }
              }
          }

          div { class: "mt-4 flex justify-end",
              button {
                  class: "rounded-lg bg-blue-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600 disabled:opacity-50 disabled:cursor-not-allowed",
                  disabled: content().trim().is_empty(),
                  onclick: move |_| {
                      // TODO: Implement comment submission logic
                      println!("Submitting comment: {}", content());
                      content.set(String::new());
                      preview_mode.set(false);
                  },
                  "发布评论"
              }
          }
      }
  }
}
