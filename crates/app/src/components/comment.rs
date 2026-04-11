use super::markdown::Markdown;
use crate::server::{get_comments, post_comment, upload_image};
use dioxus::prelude::*;

#[derive(PartialEq, Props, Clone)]
pub struct CommentBoxProps {
  pub blog_id: String,
}

#[component]
pub fn CommentBox(props: CommentBoxProps) -> Element {
  let mut content = use_signal(|| String::new());
  let mut preview_mode = use_signal(|| false);
  let mut is_submitting = use_signal(|| false);

  // Make props reactive for the resource
  let mut blog_id_signal = use_signal(|| props.blog_id.clone());
  if blog_id_signal() != props.blog_id {
    blog_id_signal.set(props.blog_id.clone());
  }

  let mut comments_resource =
    use_resource(move || async move { get_comments(blog_id_signal()).await });

  rsx! {
      div { class: "mt-12 rounded-xl border border-slate-200 bg-slate-50 p-6 dark:bg-slate-900/50 dark:border-slate-800",
          div { class: "mb-4 flex items-center justify-between",
              h3 { class: "text-lg font-semibold text-slate-900 dark:text-white", "评论区" }
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

          // Hidden file input for image upload
          input {
              type: "file",
              id: "image-upload",
              hidden: true,
              accept: "image/*",
              onchange: move |_| async move {
                  let mut eval = document::eval(r#"
                        const fileInput = document.getElementById('image-upload');
                        const file = fileInput.files[0];
                        if (!file) return;

                        console.log("JS: File selected", file.name);
                        // dioxus.send(JSON.stringify({ type: 'log', msg: 'File selected: ' + file.name }));

                        const reader = new FileReader();
                        reader.onload = async (e) => {
                            const raw = e.target.result;
                            // console.log("JS: Read complete", raw.length);
                            // dioxus.send(JSON.stringify({ type: 'log', msg: 'Read complete, length: ' + raw.length }));
                            
                            // Send metadata
                            await dioxus.send(JSON.stringify({ type: 'meta', name: file.name }));
                            
                            // Send chunks
                            const chunkSize = 32 * 1024; // 32KB chunks
                            let sent = 0;
                            while (sent < raw.length) {
                                const end = Math.min(sent + chunkSize, raw.length);
                                const chunk = raw.substring(sent, end);
                                await dioxus.send(JSON.stringify({ type: 'chunk', data: chunk }));
                                sent = end;
                            }
                            
                            await dioxus.send(JSON.stringify({ type: 'done' }));
                        };
                        reader.onerror = (e) => {
                             dioxus.send(JSON.stringify({ type: 'error', msg: 'Read error' }));
                        };
                        reader.readAsDataURL(file);
                    "#);

                  let mut filename = String::new();
                  let mut full_data = String::new();

                  loop {
                      match eval.recv::<String>().await {
                          Ok(msg_str) => {
                              if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg_str) {
                                  match val["type"].as_str() {
                                      Some("log") => println!("JS Log: {}", val["msg"].as_str().unwrap_or("")),
                                      Some("meta") => {
                                          filename = val["name"].as_str().unwrap_or("image.png").to_string();
                                      }
                                      Some("chunk") => {
                                          if let Some(chunk) = val["data"].as_str() {
                                              full_data.push_str(chunk);
                                          }
                                      }
                                      Some("done") => {
                                          println!("Rust: Reconstruction complete. Uploading {} ({} bytes)...", filename, full_data.len());
                                          match upload_image(filename.clone(), full_data.clone()).await {
                                               Ok(url) => {
                                                   println!("Upload success: {}", url);
                                                   let new_text = format!("\n![Image]({})", url);
                                                   let current_val = content.peek().clone();
                                                   content.set(format!("{}{}", current_val, new_text));
                                               }
                                               Err(e) => {
                                                   println!("Upload failed: {}", e);
                                                   let _ = document::eval(&format!("alert('上传失败: {}')", e));
                                               },
                                          }
                                          break;
                                      }
                                      Some("error") => {
                                          println!("JS Error: {}", val["msg"].as_str().unwrap_or("Unknown"));
                                          break;
                                      }
                                      _ => {}
                                  }
                              }
                          }
                          Err(e) => {
                              println!("Eval channel closed or error: {}", e);
                              break;
                          }
                      }
                  }

                  // Reset input
                  let _ = document::eval("document.getElementById('image-upload').value = '';");
              }
          }

          if preview_mode() {
              div { class: "min-h-[150px] p-4 bg-white dark:bg-slate-950 rounded-lg border border-slate-300 dark:border-slate-700 prose dark:prose-invert max-w-none",
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
                              let _ = document::eval("document.getElementById('image-upload').click()");
                          },
                          "Image"
                      }
                  }
                  textarea {
                      class: "w-full min-h-[150px] p-4 rounded-lg border border-slate-300 bg-white text-slate-900 focus:border-blue-500 focus:ring-blue-500 dark:bg-slate-950 dark:border-slate-700 dark:text-white",
                      placeholder: "写下你的评论 (支持 Markdown, 图片)...",
                      value: "{content}",
                      oninput: move |e| content.set(e.value())
                  }
              }
          }

          div { class: "mt-4 flex justify-end",
              button {
                  class: "rounded-lg bg-blue-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600 disabled:opacity-50 disabled:cursor-not-allowed",
                  disabled: content().trim().is_empty() || is_submitting(),
                  onclick: move |_| {
                      let blog_id = props.blog_id.clone();
                      async move {
                          is_submitting.set(true);
                          match post_comment(blog_id, content()).await {
                              Ok(_) => {
                                  content.set(String::new());
                                  preview_mode.set(false);
                                  comments_resource.restart();
                              }
                              Err(e) => println!("Error posting comment: {}", e),
                          }
                          is_submitting.set(false);
                      }
                  },
                  if is_submitting() { "提交中..." } else { "发布评论" }
              }
          }
      }

      // Comments List
      div { class: "mt-8 space-y-6",
          match &*comments_resource.read_unchecked() {
              Some(Ok(comments)) => {
                  if comments.is_empty() {
                       rsx! {
                           div { class: "text-center text-slate-500 py-8", "暂无评论，快来抢沙发吧！" }
                       }
                  } else {
                      rsx! {
                          for comment in comments {
                              div { class: "flex gap-4", key: "{comment.id}",
                                  div { class: "flex-none w-10 h-10 rounded-full bg-slate-200 dark:bg-slate-800 flex items-center justify-center text-slate-500 font-bold",
                                      "{comment.author.chars().next().unwrap_or('?')}"
                                  }
                                  div { class: "flex-1 space-y-1",
                                      div { class: "flex items-center justify-between",
                                          h3 { class: "text-sm font-semibold text-slate-900 dark:text-white", "{comment.author}" }
                                          span { class: "text-xs text-slate-500", "{comment.date}" }
                                      }
                                      div { class: "prose prose-sm dark:prose-invert max-w-none text-slate-700 dark:text-slate-300",
                                          Markdown { content: comment.content.clone() }
                                      }
                                  }
                              }
                          }
                      }
                  }
              },
              Some(Err(e)) => rsx! { div { class: "text-red-500", "加载评论失败: {e}" } },
              None => rsx! { div { class: "text-slate-500", "加载中..." } }
          }
      }
  }
}
