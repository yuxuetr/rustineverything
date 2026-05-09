use rustineverything_module_blog::markdown::Markdown;
use rustineverything_module_comments::server::{get_comments, post_comment};
use crate::server::upload_image;
use dioxus::prelude::*;

#[derive(PartialEq, Props, Clone)]
pub struct CommentBoxProps {
  pub blog_id: String,
}

#[component]
pub fn CommentBox(props: CommentBoxProps) -> Element {
  let mut content = use_signal(|| String::new());
  let mut is_preview = use_signal(|| false);
  let mut is_submitting = use_signal(|| false);
  let session_user = crate::use_session_user();
  let mut show_auth_modal = crate::use_auth_modal();

  // Fetch comments
  let blog_id_for_res = props.blog_id.clone();
  let mut comments = use_resource(move || {
      let id = blog_id_for_res.clone();
      async move {
          get_comments(id).await.unwrap_or_default()
      }
  });

  let blog_id_for_submit = props.blog_id.clone();
  let handle_submit = move |_| {
    if content().trim().is_empty() || is_submitting() {
      return;
    }

    let id = blog_id_for_submit.clone();
    let current_content = content();

    spawn(async move {
      is_submitting.set(true);
      match post_comment(id, current_content).await {
        Ok(new_comments) => {
          comments.set(Some(new_comments));
          content.set(String::new());
          is_preview.set(false);
        }
        Err(e) => {
          println!("[Comment] post_comment error: {:?}", e);
        }
      }
      is_submitting.set(false);
    });
  };

  let handle_upload = move |evt: Event<FormData>| {
      spawn(async move {
          let files = evt.data().files();
          for file in files {
              if let Ok(data) = file.read_bytes().await {
                  use base64::Engine as _;
                  let base64_data = base64::engine::general_purpose::STANDARD.encode(data);
                  let data_url = format!("data:image/png;base64,{}", base64_data);

                  if let Ok(url) = upload_image(file.name(), data_url).await {
                      let markdown_image = format!("\n![Image]({})", url);
                      content.with_mut(|c| c.push_str(&markdown_image));
                      content.set(content());
                  }
              }
          }
      });
  };

  let is_logged_in = session_user().is_some();

  rsx! {
      div { class: "mt-16 border-t border-slate-200 dark:border-slate-800 pt-10",
          h3 { class: "text-2xl font-bold text-slate-900 dark:text-white mb-8", "评论区" }

          // Input Area — 已登录才展示编辑器，未登录提示登录
          if is_logged_in {
              div { class: "bg-white dark:bg-slate-900 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm overflow-hidden",
                  // Toolbar
                  div { class: "flex items-center justify-between px-5 py-3 border-b border-slate-100 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-800/50",
                      div { class: "flex gap-4 text-xs font-medium text-slate-500",
                          button {
                              class: format_args!("pb-2 border-b-2 transition-all {}", if !is_preview() { "text-blue-600 border-blue-600" } else { "border-transparent hover:text-slate-700" }),
                              onclick: move |_| is_preview.set(false),
                              "编辑"
                          }
                          button {
                              class: format_args!("pb-2 border-b-2 transition-all {}", if is_preview() { "text-blue-600 border-blue-600" } else { "border-transparent hover:text-slate-700" }),
                              onclick: move |_| is_preview.set(true),
                              "预览"
                          }
                      }

                      label { class: "cursor-pointer p-1.5 rounded-md hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-600 transition-colors",
                          input {
                              r#type: "file",
                              class: "hidden",
                              accept: "image/*",
                              onchange: handle_upload
                          }
                          svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                              path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" }
                          }
                      }
                  }

                  // Body
                  div { class: "px-5 py-4",
                      if !is_preview() {
                          textarea {
                              class: "w-full h-36 bg-transparent border-0 focus:ring-0 text-sm text-slate-700 dark:text-slate-300 placeholder-slate-400 resize-vertical",
                              value: "{content}",
                              placeholder: "写下你的评论 (支持 Markdown, 图片)...",
                              oninput: move |evt| content.set(evt.value())
                          }
                      } else {
                          div { class: "min-h-[8rem] py-2",
                              Markdown { content: content(), blog_id: props.blog_id.clone() }
                          }
                      }
                  }

                  // Footer
                  div { class: "px-5 py-3 bg-slate-50/50 dark:bg-slate-800/50 flex justify-end border-t border-slate-100 dark:border-slate-800",
                      button {
                          class: format_args!("px-5 py-2 rounded-lg font-semibold text-sm transition-all {}",
                              if is_submitting() { "bg-slate-200 text-slate-400 cursor-not-allowed" }
                              else { "bg-blue-600 text-white hover:bg-blue-700 shadow-sm" }
                          ),
                          onclick: handle_submit,
                          if is_submitting() { "提交中..." } else { "发布评论" }
                      }
                  }
              }
          } else {
              // 未登录提示
              div { class: "bg-slate-50 dark:bg-slate-900/50 rounded-2xl border border-slate-200 dark:border-slate-800 p-8 text-center",
                  p { class: "text-slate-500 dark:text-slate-400 mb-4", "登录后即可发表评论" }
                  button {
                      onclick: move |_| show_auth_modal.set(true),
                      class: "inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 text-white text-sm font-semibold hover:bg-blue-700 transition-colors",
                      "登录"
                  }
              }
          }

          // List of comments
          div { class: "mt-12 space-y-8",
              if let Some(comment_list) = comments.read().as_ref() {
                  for comment in comment_list.iter() {
                      div { key: "{comment.id}", class: "flex gap-4",
                          // Avatar
                          div { class: "flex-none",
                              if let Some(ref avatar) = comment.author_avatar {
                                  img {
                                      src: "{avatar}",
                                      class: "w-10 h-10 rounded-full object-cover",
                                      alt: "{comment.author}"
                                  }
                              } else {
                                  div { class: "w-10 h-10 rounded-full bg-slate-200 dark:bg-slate-800 flex items-center justify-center text-slate-500",
                                      svg { class: "w-6 h-6", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" } }
                                  }
                              }
                          }
                          // Content
                          div { class: "flex-1 space-y-1",
                              div { class: "flex items-center justify-between",
                                  h4 { class: "text-sm font-bold text-slate-900 dark:text-white", "{comment.author}" }
                                  span { class: "text-xs text-slate-500", "{comment.date}" }
                              }
                              div { class: "text-sm text-slate-700 dark:text-slate-300 prose-comment",
                                  Markdown { content: comment.content.clone(), blog_id: props.blog_id.clone() }
                              }
                          }
                      }
                  }
              } else {
                  div { class: "text-center text-slate-500 py-10", "加载评论中..." }
              }
          }
      }
  }
}
