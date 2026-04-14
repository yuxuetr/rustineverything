use dioxus::prelude::*;
use dioxus::router::{Link, Routable};

use crate::components::comment::CommentBox;
use crate::components::hero::Hero;
use crate::components::nav::Navbar;
use crate::components::view::{Container, SectionTitle};
use rustineverything_module_blog::markdown::Markdown;
use rustineverything_module_blog::server::{get_blog_content, list_blog_posts};
use rustineverything_module_podcast::podcast::PodcastPage;

/// Application routes
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
        #[route("/")]
        Home {},

        #[route("/docs")]
        Docs {},

        #[route("/blog")]
        BlogIndex {},
        #[route("/blog/:id")]
        Blog { id: String },

        #[route("/podcast")]
        Podcast {},

        #[route("/courses")]
        Courses {},

        #[route("/cases")]
        Cases {},

        #[route("/ai")]
        Ai {},

        #[route("/web3")]
        Web3 {},

        #[route("/topics")]
        TopicsIndex {},
        #[route("/topics/:tag")]
        Topic { tag: String },
}

/// Home page
#[component]
pub fn Home() -> Element {
  rsx! {
      Hero {}
      section { class: "py-24 bg-white dark:bg-slate-950",
          Container {
              SectionTitle {
                  title: "专注 Rust 生态".to_string(),
                  subtitle: Some("从底层原理到全栈实战，构建高性能、高可靠的软件系统".to_string())
              }

              div { class: "grid grid-cols-1 md:grid-cols-3 gap-8 mt-12",
                  FeatureCard {
                      title: "Rust 基础".to_string(),
                      desc: "深入浅出所有权、生命周期、Trait 等核心概念。".to_string(),
                      icon: rsx! {
                          path { stroke_linecap: "round", stroke_linejoin: "round", d: "M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.74c0 3.821 1.77 7.239 4.537 9.477a11.981 11.981 0 0014.926 0C25.23 16.979 27 13.561 27 9.74c0-1.302-.209-2.557-.598-3.74A11.959 11.959 0 0112 2.714z" }
                      }
                  }
                  FeatureCard {
                      title: "全栈开发".to_string(),
                      desc: "使用 Dioxus, Axum, SeaORM 快速构建跨平台应用。".to_string(),
                      icon: rsx! {
                          path { stroke_linecap: "round", stroke_linejoin: "round", d: "M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" }
                      }
                  }
                  FeatureCard {
                      title: "AI 与 WASM".to_string(),
                      desc: "探索 WebAssembly 高性能计算与 Rust AI 生态。".to_string(),
                      icon: rsx! {
                          path { stroke_linecap: "round", stroke_linejoin: "round", d: "M15.362 5.214A8.252 8.252 0 0112 21 8.25 8.25 0 016.038 7.048 8.287 8.25 0 009 9.6a8.983 8.983 0 013.361-6.867 8.21 8.21 0 003 2.48z" }
                      }
                  }
              }
          }
      }
  }
}

#[component]
fn FeatureCard(title: String, desc: String, icon: Element) -> Element {
  rsx! {
      div { class: "p-8 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 hover:shadow-lg transition-all",
          div { class: "w-12 h-12 rounded-lg bg-blue-600/10 flex items-center justify-center text-blue-600 mb-6",
              svg { class: "w-6 h-6", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "1.5",
                  {icon}
              }
          }
          h3 { class: "text-xl font-bold text-slate-900 dark:text-white mb-3", "{title}" }
          p { class: "text-slate-600 dark:text-slate-400 leading-relaxed", "{desc}" }
      }
  }
}

#[component]
pub fn Docs() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "文档".to_string(), subtitle: Some("Rust 学习与实践指南".to_string()) }
              div { class: "max-w-3xl mx-auto prose prose-slate dark:prose-invert",
                  "TODO: 接入文档内容（MDX/Markdown 渲染、目录、搜索）。"
              }
          }
      }
  }
}

#[component]
pub fn BlogIndex() -> Element {
  let posts = use_resource(move || async move {
      list_blog_posts().await.unwrap_or_default()
  });

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "博客".to_string(), subtitle: Some("探索 Rust 的无限可能".to_string()) }
              div { class: "space-y-4 max-w-3xl mx-auto",
                  match posts() {
                      Some(list) => rsx! {
                          for post in list.iter() {
                              Link {
                                  key: "{post.slug}",
                                  to: Route::Blog { id: post.slug.clone() },
                                  class: "group block rounded-xl border border-slate-200 dark:border-slate-800 p-6 hover:bg-slate-50 dark:hover:bg-slate-900/30 transition-all",
                                  div { class: "flex justify-between items-start mb-2",
                                      h3 { class: "text-lg font-bold text-slate-900 dark:text-white group-hover:text-blue-600 transition-colors",
                                          "{post.title}"
                                      }
                                      if !post.date.is_empty() {
                                          span { class: "text-xs text-slate-500 whitespace-nowrap ml-4", "{post.date}" }
                                      }
                                  }
                                  if !post.description.is_empty() {
                                      p { class: "text-sm text-slate-600 dark:text-slate-400 line-clamp-2", "{post.description}" }
                                  }
                                  if !post.tags.is_empty() {
                                      div { class: "flex flex-wrap gap-2 mt-3",
                                          for tag in post.tags.iter() {
                                              span { class: "text-xs px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400",
                                                  "{tag}"
                                              }
                                          }
                                      }
                                  }
                              }
                          }
                      },
                      None => rsx! {
                          div { class: "flex items-center justify-center py-20",
                              div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                          }
                      },
                  }
              }
          }
      }
  }
}

#[component]
pub fn Blog(id: String) -> Element {
  // 修复：在闭包外先克隆一次 id 用于 resource，保留原 id 用于后续组件
  let id_for_res = id.clone();
  let blog_content = use_resource(move || {
      let inner_id = id_for_res.clone();
      async move { get_blog_content(inner_id).await }
  });

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              div { class: "max-w-4xl mx-auto",
                  div { class: "text-slate-700 dark:text-slate-200 mb-12",
                      match blog_content() {
                          Some(Ok(content)) => rsx! { Markdown { content: content.clone(), blog_id: id.clone() } },
                          Some(Err(e)) => rsx! { 
                              div { class: "p-4 bg-red-50 text-red-700 rounded-lg", "Error loading post: {e}" }
                          },
                          None => rsx! { 
                              div { class: "flex items-center justify-center py-20",
                                  div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                              }
                          },
                      }
                  }

                  div { class: "border-t border-slate-200 dark:border-slate-800 pt-8 mt-12",
                      CommentBox { blog_id: id }
                  }
              }
          }
      }
  }
}

#[component]
pub fn Courses() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "课程".to_string(), subtitle: Some("系统化学习路径".to_string()) }
              div { class: "max-w-3xl mx-auto grid grid-cols-1 md:grid-cols-2 gap-6",
                  CourseItem { name: "Rust 基础与所有权", progress: "100%" }
                  CourseItem { name: "异步与 Tokio 实战", progress: "0%" }
                  CourseItem { name: "全栈：Dioxus + ServerFn", progress: "0%" }
                  CourseItem { name: "Wasm：前端与性能", progress: "0%" }
              }
          }
      }
  }
}

#[component]
fn CourseItem(name: &'static str, progress: &'static str) -> Element {
    rsx! {
        div { class: "p-6 rounded-xl border border-slate-200 dark:border-slate-800",
            h4 { class: "font-bold text-slate-900 dark:text-white mb-2", "{name}" }
            div { class: "w-full bg-slate-200 dark:bg-slate-800 h-2 rounded-full overflow-hidden",
                div { class: "bg-blue-600 h-full", style: "width: {progress}" }
            }
            div { class: "mt-2 text-right text-xs text-slate-500", "{progress}" }
        }
    }
}

#[component]
pub fn Cases() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "案例".to_string(), subtitle: Some("真实世界中的 Rust 应用".to_string()) }
              div { class: "text-center text-slate-500 py-20", "Case studies are coming soon..." }
          }
      }
  }
}

#[component]
pub fn Ai() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "AI".to_string(), subtitle: Some("Rust 驱动的 AI 开发".to_string()) }
              div { class: "text-center text-slate-500 py-20", "AI modules are coming soon..." }
          }
      }
  }
}

#[component]
pub fn Web3() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "Web3".to_string(), subtitle: Some("区块链与去中心化应用".to_string()) }
              div { class: "text-center text-slate-500 py-20", "Web3 modules are coming soon..." }
          }
      }
  }
}

#[component]
pub fn TopicsIndex() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "论坛".to_string(), subtitle: Some("交流、分享、共同进步".to_string()) }
              div { class: "text-center text-slate-500 py-20", "Forum is under development..." }
          }
      }
  }
}

#[component]
pub fn Topic(tag: String) -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              div { "Topic: {tag}" }
          }
      }
  }
}

#[component]
pub fn Podcast() -> Element {
  rsx! {
      PodcastPage {}
  }
}
