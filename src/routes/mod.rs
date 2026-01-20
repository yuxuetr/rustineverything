use dioxus::prelude::*;
use dioxus::router::{Link, Routable};

use crate::components::comment::CommentBox;
use crate::components::hero::Hero;
use crate::components::markdown::Markdown;
use crate::components::nav::Navbar;
use crate::components::podcast::PodcastPage;
use crate::components::view::{Container, SectionTitle};
use crate::server::get_blog_content;

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
        Blog { id: i32 },

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
  let domains: [(&str, &str); 8] = [
    ("AI", "/topics/ai"),
    ("后端", "/topics/backend"),
    ("前端", "/topics/frontend"),
    ("跨端", "/topics/cross-platform"),
    ("Web3", "/topics/web3"),
    ("Wasm", "/topics/wasm"),
    ("嵌入式", "/topics/embedded"),
    ("命令行", "/topics/cli"),
  ];

  rsx! {
      Hero {}

      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle {
                  title: "入口".to_string(),
                  subtitle: Some("从这里开始：系统学习 + 真实案例 + 可复用代码".to_string()),
              }

              div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4",
                  HomeCard {
                      title: "文档".to_string(),
                      desc: "架构、工具链、最佳实践、速查表".to_string(),
                      to: Route::Docs {},
                  }
                  HomeCard {
                      title: "博客".to_string(),
                      desc: "踩坑记录、深度解析、版本迁移".to_string(),
                      to: Route::BlogIndex {},
                  }
                  HomeCard {
                      title: "课程".to_string(),
                      desc: "按路径学习：从基础到工程化".to_string(),
                      to: Route::Courses {},
                  }
                  HomeCard {
                      title: "案例".to_string(),
                      desc: "可运行项目：全栈/CLI/Wasm/嵌入式".to_string(),
                      to: Route::Cases {},
                  }
              }
          }
      }

      section { class: "py-12 bg-slate-50 dark:bg-slate-900/30",
          Container {
              SectionTitle { title: "领域".to_string(), subtitle: Some("按方向聚合内容（持续补齐）".to_string()) }

              div { class: "flex flex-wrap gap-2",
                  for (label, href) in domains {
                      Link {
                          to: Route::Topic { tag: href.trim_start_matches("/topics/").to_string() },
                          class: "inline-flex items-center rounded-full border border-slate-200 bg-white px-3 py-1 text-sm text-slate-700 hover:bg-slate-100 dark:bg-slate-950 dark:border-slate-800 dark:text-slate-200 dark:hover:bg-slate-900",
                          "{label}"
                      }
                  }
              }
          }
      }

      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "最新内容".to_string(), subtitle: Some("先占位，后续接入真实数据源".to_string()) }

              div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4",
                  ContentStub { kind: "博客".to_string(), title: "Dioxus 0.7 路由与布局最佳实践".to_string(), hint: "Router / layout / Outlet".to_string(), to: Route::Blog { id: 1 } }
                  ContentStub { kind: "文档".to_string(), title: "Rust 工程化：workspace、feature、CI".to_string(), hint: "Cargo / Clippy / fmt".to_string(), to: Route::Docs {} }
                  ContentStub { kind: "案例".to_string(), title: "全栈 Rust：ServerFn + 前端状态管理".to_string(), hint: "fullstack / server functions".to_string(), to: Route::Cases {} }
              }
          }
      }
  }
}

#[derive(Clone, PartialEq, Props)]
struct HomeCardProps {
  title: String,
  desc: String,
  to: Route,
}

#[component]
fn HomeCard(props: HomeCardProps) -> Element {
  rsx! {
      Link {
          to: props.to,
          class: "group rounded-xl border border-slate-200 bg-white p-5 hover:border-slate-300 hover:shadow-sm dark:bg-slate-950 dark:border-slate-800 dark:hover:border-slate-700",
          div { class: "text-base font-semibold text-slate-900 dark:text-white", "{props.title}" }
          div { class: "mt-2 text-sm text-slate-600 dark:text-slate-300", "{props.desc}" }
          div { class: "mt-4 text-sm font-medium text-blue-600 group-hover:text-blue-700 dark:text-blue-400", "查看 →" }
      }
  }
}

#[derive(Clone, PartialEq, Props)]
struct ContentStubProps {
  kind: String,
  title: String,
  hint: String,
  to: Route,
}

#[component]
fn ContentStub(props: ContentStubProps) -> Element {
  rsx! {
      Link {
          to: props.to,
          class: "rounded-xl border border-slate-200 bg-white p-5 hover:shadow-sm dark:bg-slate-950 dark:border-slate-800",
          div { class: "text-xs font-semibold text-slate-500 dark:text-slate-400", "{props.kind}" }
          div { class: "mt-2 font-semibold text-slate-900 dark:text-white", "{props.title}" }
          div { class: "mt-2 text-sm text-slate-600 dark:text-slate-300", "{props.hint}" }
      }
  }
}

#[component]
pub fn Docs() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "文档".to_string(), subtitle: Some("这里将放置 Rust 技术栈的系统文档".to_string()) }
              div { class: "rounded-xl border border-slate-200 dark:border-slate-800 p-6 text-slate-700 dark:text-slate-200",
                  "TODO: 接入文档内容（MDX/Markdown 渲染、目录、搜索）。"
              }
          }
      }
  }
}

#[component]
pub fn BlogIndex() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "博客".to_string(), subtitle: Some("文章列表（先占位）".to_string()) }
            div { class: "space-y-3",
                Link { to: Route::Blog { id: 2 }, class: "block rounded-lg border border-slate-200 dark:border-slate-800 p-4 hover:bg-slate-50 dark:hover:bg-slate-900/30",
                    div { class: "font-semibold text-slate-900 dark:text-white", "Python 的 Rust 化时刻" }
                    div { class: "text-sm text-slate-600 dark:text-slate-300", "用 Pydantic 重构你的编程思维" }
                }
                Link { to: Route::Blog { id: 1 }, class: "block rounded-lg border border-slate-200 dark:border-slate-800 p-4 hover:bg-slate-50 dark:hover:bg-slate-900/30",
                    div { class: "font-semibold text-slate-900 dark:text-white", "Dioxus 0.7：路由、布局、状态" }
                    div { class: "text-sm text-slate-600 dark:text-slate-300", "迁移与工程实践总结" }
                }
            }
          }
      }
  }
}

#[component]
pub fn Blog(id: i32) -> Element {
  let blog_content = use_resource(move || async move { get_blog_content(id.to_string()).await });

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              div { class: "text-slate-700 dark:text-slate-200",
                  match blog_content() {
                      Some(Ok(content)) => rsx! { Markdown { content: content.clone() } },
                      Some(Err(e)) => rsx! { "Error loading post: {e}" },
                      None => rsx! { "Loading..." },
                  }
              }

              div { class: "mt-6 flex gap-3",
                  if id > 1 {
                      Link { to: Route::Blog { id: id - 1 }, class: "text-blue-600 dark:text-blue-400 hover:underline", "上一篇" }
                  }
                  Link { to: Route::Blog { id: id + 1 }, class: "text-blue-600 dark:text-blue-400 hover:underline", "下一篇" }
              }

              CommentBox {}
          }
      }
  }
}

#[component]
pub fn Courses() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "课程".to_string(), subtitle: Some("学习路径（先占位）".to_string()) }
              ul { class: "list-disc pl-6 text-slate-700 dark:text-slate-200 space-y-2",
                  li { "Rust 基础与所有权" }
                  li { "异步与 Tokio 实战" }
                  li { "全栈：Dioxus + ServerFn" }
                  li { "Wasm：前端与性能" }
              }
          }
      }
  }
}

#[component]
pub fn Cases() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "案例".to_string(), subtitle: Some("可运行项目合集（先占位）".to_string()) }
              div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                  CaseCard { title: "全栈 Dashboard".to_string(), desc: "鉴权、数据可视化、ServerFn".to_string() }
                  CaseCard { title: "CLI 工具箱".to_string(), desc: "配置、日志、插件化".to_string() }
                  CaseCard { title: "Wasm 组件".to_string(), desc: "前端交互与性能优化".to_string() }
                  CaseCard { title: "嵌入式 Demo".to_string(), desc: "HAL、RTOS、调试".to_string() }
              }
          }
      }
  }
}

#[derive(Clone, PartialEq, Props)]
struct CaseCardProps {
  title: String,
  desc: String,
}

#[component]
fn CaseCard(props: CaseCardProps) -> Element {
  rsx! {
      div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 p-5",
          div { class: "font-semibold text-slate-900 dark:text-white", "{props.title}" }
          div { class: "mt-2 text-sm text-slate-600 dark:text-slate-300", "{props.desc}" }
      }
  }
}

#[component]
pub fn TopicsIndex() -> Element {
  let badge = "inline-flex items-center rounded-full border border-slate-200 bg-white px-3 py-1 text-sm text-slate-700 hover:bg-slate-100 dark:bg-slate-950 dark:border-slate-800 dark:text-slate-200 dark:hover:bg-slate-900";

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "领域".to_string(), subtitle: Some("选择一个方向查看聚合内容".to_string()) }
              div { class: "flex flex-wrap gap-2",
                  Link { to: Route::Topic { tag: "ai".to_string() }, class: badge, "AI" }
                  Link { to: Route::Topic { tag: "backend".to_string() }, class: badge, "后端" }
                  Link { to: Route::Topic { tag: "frontend".to_string() }, class: badge, "前端" }
                  Link { to: Route::Topic { tag: "cross-platform".to_string() }, class: badge, "跨端" }
                  Link { to: Route::Topic { tag: "web3".to_string() }, class: badge, "Web3" }
                  Link { to: Route::Topic { tag: "wasm".to_string() }, class: badge, "Wasm" }
                  Link { to: Route::Topic { tag: "embedded".to_string() }, class: badge, "嵌入式" }
                  Link { to: Route::Topic { tag: "cli".to_string() }, class: badge, "命令行" }
              }
          }
      }
  }
}

#[component]
pub fn Podcast() -> Element {
  rsx! { PodcastPage {} }
}

#[component]
pub fn Ai() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "AI + Rust".to_string(), subtitle: Some("大模型、推理、Agent 开发".to_string()) }
              div { class: "rounded-xl border border-slate-200 dark:border-slate-800 p-6 text-slate-700 dark:text-slate-200",
                  "TODO: AI 相关内容聚合。"
              }
          }
      }
  }
}

#[component]
pub fn Web3() -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: "Web3 & Blockchain".to_string(), subtitle: Some("智能合约、链开发、加密学".to_string()) }
              div { class: "rounded-xl border border-slate-200 dark:border-slate-800 p-6 text-slate-700 dark:text-slate-200",
                  "TODO: Web3 相关内容聚合。"
              }
          }
      }
  }
}
#[component]
pub fn Topic(tag: String) -> Element {
  let title = match tag.as_str() {
    "ai" => "AI",
    "backend" => "后端",
    "frontend" => "前端",
    "cross-platform" => "跨端",
    "web3" => "Web3",
    "wasm" => "Wasm",
    "embedded" => "嵌入式",
    "cli" => "命令行",
    _ => "其它",
  };

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              SectionTitle { title: format!("领域：{}", title), subtitle: Some("聚合文档/博客/课程/案例（先占位）".to_string()) }
              div { class: "rounded-xl border border-slate-200 dark:border-slate-800 p-6 text-slate-700 dark:text-slate-200",
                  "TODO: 为 "
                  strong { "{title}" }
                  " 构建内容聚合页（标签、筛选、排序）。"
              }
          }
      }
  }
}
