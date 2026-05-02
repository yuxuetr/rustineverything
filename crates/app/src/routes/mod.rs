use dioxus::prelude::*;
use dioxus::router::{Link, Routable};

use crate::components::comment::CommentBox;
use crate::components::hero::Hero;
use crate::components::nav::Navbar;
use crate::components::view::{Container, SectionTitle};
use crate::i18n::{t, use_i18n};
use crate::server::{list_doc_tree, get_doc_content, DocTreeNode};
use rustineverything_module_blog::markdown::Markdown;
use rustineverything_module_blog::server::{get_blog_content, list_blog_posts};
use rustineverything_module_course::course::{
    AnnotationLayer, CourseDetailPage, CoursesIndexPage, LessonPage, MyAnnotationsPage,
};
use rustineverything_module_admin::admin::{
    AdminCommentsPage, AdminDashboardPage, AdminPluginsPage, AdminTopicsPage, AdminUsersPage,
};
use rustineverything_module_forum::forum::{
    DiscussionPanel, MyTopicsPage, NewTopicPage, TopicDetailPage, TopicsByTagPage,
    TopicsIndexPage,
};
use rustineverything_module_podcast::podcast::PodcastPage;
use rustineverything_module_cases::cases::{CaseDetailPage, CasesIndexPage};

/// Application routes
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
        #[route("/")]
        Home {},

        #[route("/docs")]
        Docs {},
        #[route("/docs/:..path")]
        DocPage { path: Vec<String> },

        #[route("/blog")]
        BlogIndex {},
        #[route("/blog/:id")]
        Blog { id: String },

        #[route("/podcast")]
        Podcast {},

        // 注意：SPA 路由使用单数 /course，避免与静态 ServeDir(/courses) 冲突
        #[route("/course")]
        Courses {},
        #[route("/course/:slug")]
        CourseDetail { slug: String },
        #[route("/course/:slug/:chapter/:lesson")]
        Lesson { slug: String, chapter: String, lesson: String },

        // SPA 路由使用单数 /case，避免与静态 ServeDir(/cases) 冲突
        #[route("/case")]
        Cases {},
        #[route("/case/:slug")]
        CaseDetail { slug: String },

        #[route("/ai")]
        Ai {},

        #[route("/web3")]
        Web3 {},

        // 论坛：注意路由顺序，静态路径优先于 i32 动态参数
        #[route("/topics")]
        TopicsIndex {},
        #[route("/topics/new")]
        TopicsNew {},
        #[route("/topics/tag/:tag")]
        TopicsByTag { tag: String },
        #[route("/topics/:id")]
        TopicDetail { id: i32 },

        // 个人中心：标注管理 / 我的话题
        #[route("/me/annotations")]
        MyAnnotations {},
        #[route("/me/topics")]
        MyTopics {},

        // Admin 后台（页面内部判断 role，非 admin 渲染 403 占位）
        #[route("/admin")]
        AdminDashboard {},
        #[route("/admin/users")]
        AdminUsers {},
        #[route("/admin/comments")]
        AdminComments {},
        #[route("/admin/topics")]
        AdminTopics {},
        #[route("/admin/plugins")]
        AdminPlugins {},
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

/// /docs 文档首页：左侧一级分类列表 + 右侧欢迎页和分类卡片
#[component]
pub fn Docs() -> Element {
  let tree = use_resource(move || async move {
      list_doc_tree().await.unwrap_or_default()
  });

  rsx! {
      section { class: "min-h-screen bg-white dark:bg-slate-950",
          div { class: "max-w-7xl mx-auto flex",
              // 左侧导航：一级分类列表（仅桌面端显示）
              aside { class: "hidden lg:block shrink-0 w-64 sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 pt-8 pb-12 px-4",
                  nav {
                      h3 { class: "text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 px-3 mb-4", "文档导航" }
                      if let Some(ref nodes) = tree() {
                          for node in nodes.iter() {
                              {render_sidebar_link(node, "")}
                          }
                      }
                  }
              }

              // 右侧：欢迎页 + 分类卡片
              div { class: "flex-1 min-w-0 px-6 lg:px-12 py-10",
                  div { class: "max-w-4xl mx-auto",
                      // 标题
                      div { class: "mb-12",
                          h1 { class: "text-3xl font-extrabold text-slate-900 dark:text-white mb-3", "文档" }
                          p { class: "text-lg text-slate-600 dark:text-slate-400 leading-relaxed",
                              "从基础语法到全栈实战，系统化学习 Rust 生态。选择一个主题开始探索。"
                          }
                      }

                      // 分类卡片
                      match tree() {
                          Some(nodes) if !nodes.is_empty() => rsx! {
                              div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6",
                                  for node in nodes.iter() {
                                      {render_doc_card(node)}
                                  }
                              }
                          },
                          Some(_) => rsx! {
                              div { class: "text-center text-slate-500 py-20", "暂无文档内容" }
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
}

/// 侧栏链接：一级分类项
fn render_sidebar_link(node: &DocTreeNode, _active: &str) -> Element {
    let path_segments: Vec<String> = node.path.split('/').map(|s| s.to_string()).collect();
    let child_count = count_leaves(node);

    rsx! {
        Link {
            to: if node.has_content { Route::DocPage { path: path_segments } } else { Route::Docs {} },
            class: "flex items-center justify-between px-3 py-2.5 rounded-lg text-sm font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors mb-1",
            span { "{node.title}" }
            span { class: "text-xs text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded-full",
                "{child_count}"
            }
        }
    }
}

/// 统计节点下的叶子文档数
fn count_leaves(node: &DocTreeNode) -> usize {
    if node.children.is_empty() {
        return if node.has_content { 1 } else { 0 };
    }
    let mut count = if node.has_content { 1 } else { 0 };
    for child in &node.children {
        count += count_leaves(child);
    }
    count
}

/// 文档分类卡片
fn render_doc_card(node: &DocTreeNode) -> Element {
    let path_segments: Vec<String> = node.path.split('/').map(|s| s.to_string()).collect();
    let child_count = count_leaves(node);

    // 子项标题列表（最多显示 5 个）
    let child_titles: Vec<String> = node.children.iter().take(5).map(|c| c.title.clone()).collect();
    let has_more = node.children.len() > 5;

    rsx! {
        Link {
            to: if node.has_content { Route::DocPage { path: path_segments } } else { Route::Docs {} },
            class: "group block p-6 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 hover:border-blue-300 dark:hover:border-blue-700 hover:shadow-lg transition-all",
            // 标题行
            div { class: "flex items-center justify-between mb-3",
                h3 { class: "text-lg font-bold text-slate-900 dark:text-white group-hover:text-blue-600 transition-colors",
                    "{node.title}"
                }
                span { class: "text-xs text-slate-400 bg-slate-200 dark:bg-slate-800 px-2 py-0.5 rounded-full",
                    "{child_count} 篇"
                }
            }
            // 子项列表
            if !child_titles.is_empty() {
                ul { class: "space-y-1",
                    for title in child_titles.iter() {
                        li { class: "text-sm text-slate-600 dark:text-slate-400 flex items-center gap-2",
                            span { class: "w-1 h-1 rounded-full bg-slate-400 dark:bg-slate-600 shrink-0" }
                            "{title}"
                        }
                    }
                    if has_more {
                        li { class: "text-xs text-slate-400 italic", "…更多" }
                    }
                }
            }
            // 底部箭头
            div { class: "mt-4 flex items-center text-sm font-medium text-blue-600 dark:text-blue-400 opacity-0 group-hover:opacity-100 transition-opacity",
                "开始阅读"
                svg { class: "w-4 h-4 ml-1", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                    path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M9 5l7 7-7 7" }
                }
            }
        }
    }
}

/// 文档页面：左侧树形导航 + 右侧 Markdown 内容
#[component]
pub fn DocPage(path: Vec<String>) -> Element {
  let doc_path = path.join("/");
  let doc_path_for_tree = doc_path.clone();
  let doc_path_for_content = doc_path.clone();
  // 标注资源路径：resource_kind="doc"，resource_path = 叶子路径
  let anno_path = doc_path.clone();
  // Markdown blog_id 携带 "doc:<path>" 前缀，JS 运行时以此识别资源归属
  let anno_blog_id = format!("doc:{}", doc_path);

  let tree = use_resource(move || async move {
      list_doc_tree().await.unwrap_or_default()
  });

  let content = use_resource(move || {
      let p = doc_path_for_content.clone();
      async move { get_doc_content(p).await }
  });

  rsx! {
      section { class: "min-h-screen bg-white dark:bg-slate-950",
          div { class: "max-w-7xl mx-auto flex",
              // 左侧导航（仅桌面端显示）
              aside { class: "hidden lg:block shrink-0 w-64 sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 pt-8 pb-12 px-4",
                  nav {
                      if let Some(ref nodes) = tree() {
                          for node in nodes.iter() {
                              TreeSection { node: node.clone(), active_path: doc_path_for_tree.clone(), depth: 0 }
                          }
                      }
                  }
              }

              // 右侧内容
              div { class: "flex-1 min-w-0 px-6 lg:px-12 py-8",
                  div { class: "max-w-3xl mx-auto",
                      match content() {
                          Some(Ok(resp)) => rsx! {
                              // SEO: 注入 title / description / keywords / og:image
                              if !resp.meta.title.is_empty() {
                                  document::Title { "{resp.meta.title} - Rust in Everything" }
                              }
                              if !resp.meta.description.is_empty() {
                                  document::Meta { name: "description", content: "{resp.meta.description}" }
                              }
                              if !resp.meta.keywords.is_empty() {
                                  document::Meta { name: "keywords", content: "{resp.meta.keywords.join(\", \")}" }
                              }
                              if let Some(ref img) = resp.meta.image {
                                  document::Meta { property: "og:image", content: "{img}" }
                              }
                              div { class: "text-slate-700 dark:text-slate-200",
                                  Markdown { content: resp.content.clone(), blog_id: anno_blog_id.clone() }
                              }
                              // 标注层（resource_kind="doc"，path 为叶子路径）
                              AnnotationLayer {
                                  resource_kind: "doc".to_string(),
                                  resource_path: anno_path.clone(),
                              }
                              // 资源讨论面板：关联论坛话题
                              DiscussionPanel {
                                  resource_kind: "doc".to_string(),
                                  resource_path: anno_path.clone(),
                              }
                          },
                          Some(Err(e)) => rsx! {
                              div { class: "p-4 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 rounded-lg",
                                  "加载失败: {e}"
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
}

/// 判断 active_path 是否属于某节点及其子树
fn is_path_within(node: &DocTreeNode, active_path: &str) -> bool {
    if active_path == node.path || active_path.starts_with(&format!("{}/", node.path)) {
        return true;
    }
    node.children.iter().any(|c| is_path_within(c, active_path))
}

/// 可折叠的树节点组件
#[component]
fn TreeSection(node: DocTreeNode, active_path: String, depth: u32) -> Element {
    let has_children = !node.children.is_empty();
    let is_active = active_path == node.path;
    let is_within = is_path_within(&node, &active_path);

    // 当前页所在的分支默认展开
    let mut expanded = use_signal(move || is_within);

    let path_segments: Vec<String> = node.path.split('/').map(|s| s.to_string()).collect();

    match depth {
        // === 一级：分组标题，带展开/折叠箭头 ===
        0 => {
            rsx! {
                div { class: "mb-3",
                    // 标题行：可点击展开/折叠，也可导航
                    div { class: "flex items-center gap-1",
                        if has_children {
                            button {
                                class: "p-0.5 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-400 transition-colors",
                                onclick: move |_| expanded.set(!expanded()),
                                svg {
                                    class: format_args!("w-3.5 h-3.5 transition-transform {}", if expanded() { "rotate-90" } else { "" }),
                                    fill: "currentColor", view_box: "0 0 20 20",
                                    path { d: "M6.293 7.293a1 1 0 011.414 0L10 9.586l2.293-2.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" }
                                }
                            }
                        }
                        if node.has_content {
                            Link {
                                to: Route::DocPage { path: path_segments },
                                class: format_args!("text-xs font-semibold uppercase tracking-wider py-1 {}",
                                    if is_active { "text-blue-600 dark:text-blue-400" } else { "text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white" }
                                ),
                                "{node.title}"
                            }
                        } else {
                            button {
                                class: "text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 py-1",
                                onclick: move |_| expanded.set(!expanded()),
                                "{node.title}"
                            }
                        }
                    }
                    // 子节点
                    if has_children && expanded() {
                        div { class: "mt-1 ml-1 border-l-2 border-slate-200 dark:border-slate-800 pl-3",
                            for child in node.children.iter() {
                                TreeSection { node: child.clone(), active_path: active_path.clone(), depth: 1 }
                            }
                        }
                    }
                }
            }
        }
        // === 二级：章节，带展开/折叠 ===
        1 => {
            let item_class = if is_active {
                "text-sm font-medium text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20"
            } else {
                "text-sm text-slate-700 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white hover:bg-slate-50 dark:hover:bg-slate-800/50"
            };

            rsx! {
                div { class: "mb-0.5",
                    div { class: "flex items-center",
                        if has_children {
                            button {
                                class: "p-0.5 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-400 shrink-0",
                                onclick: move |_| expanded.set(!expanded()),
                                svg {
                                    class: format_args!("w-3 h-3 transition-transform {}", if expanded() { "rotate-90" } else { "" }),
                                    fill: "currentColor", view_box: "0 0 20 20",
                                    path { d: "M6.293 7.293a1 1 0 011.414 0L10 9.586l2.293-2.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" }
                                }
                            }
                        }
                        if node.has_content {
                            Link {
                                to: Route::DocPage { path: path_segments },
                                class: "flex-1 block px-2 py-1.5 rounded-md transition-colors {item_class}",
                                "{node.title}"
                            }
                        } else {
                            button {
                                class: "flex-1 text-left px-2 py-1.5 rounded-md transition-colors {item_class}",
                                onclick: move |_| expanded.set(!expanded()),
                                "{node.title}"
                            }
                        }
                    }
                    if has_children && expanded() {
                        div { class: "ml-4 mt-0.5 border-l border-slate-200 dark:border-slate-800 pl-3",
                            for child in node.children.iter() {
                                TreeSection { node: child.clone(), active_path: active_path.clone(), depth: 2 }
                            }
                        }
                    }
                }
            }
        }
        // === 三级：小节，叶子节点 ===
        _ => {
            let item_class = if is_active {
                "text-xs font-medium text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20"
            } else {
                "text-xs text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white hover:bg-slate-50 dark:hover:bg-slate-800/50"
            };

            rsx! {
                if node.has_content {
                    Link {
                        to: Route::DocPage { path: path_segments },
                        class: "block px-2 py-1 rounded-md transition-colors mb-0.5 {item_class}",
                        "{node.title}"
                    }
                } else {
                    div { class: "px-2 py-1 {item_class}", "{node.title}" }
                }
            }
        }
    }
}

#[component]
pub fn BlogIndex() -> Element {
  let lang = use_i18n();

  let posts_res = use_resource(move || async move {
      list_blog_posts().await.unwrap_or_default()
  });
  let posts = posts_res.read().as_ref().cloned().unwrap_or_default();

  let mut active_tag = use_signal::<Option<String>>(|| None);
  let mut current_page = use_signal(|| 0usize);
  const PAGE_SIZE: usize = 10;

  // 汇总全部标签
  let all_tags: Vec<String> = {
      let mut seen = std::collections::BTreeSet::new();
      for post in posts.iter() {
          for t in post.tags.iter() { seen.insert(t.clone()); }
      }
      seen.into_iter().collect()
  };

  // 按标签过滤
  let filtered: Vec<_> = match active_tag() {
      Some(ref tag) => posts.iter().filter(|p| p.tags.contains(tag)).cloned().collect(),
      None => posts.clone(),
  };

  // 分页
  let total_pages = ((filtered.len() + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
  let safe_page = current_page().min(total_pages - 1);
  let paged: Vec<_> = filtered.iter().skip(safe_page * PAGE_SIZE).take(PAGE_SIZE).cloned().collect();

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8",
              // 页面标题
              div { class: "text-center mb-10",
                  h2 { class: "text-3xl font-bold tracking-tight text-slate-900 dark:text-white sm:text-4xl", "{t(lang(), \"blog.title\")}" }
                  p { class: "mt-4 text-lg text-slate-500 dark:text-slate-400", "{t(lang(), \"blog.subtitle\")}" }
              }

              match posts_res.read().as_ref() {
                  None => rsx! {
                      div { class: "flex items-center justify-center py-20",
                          div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                      }
                  },
                  Some(_) => rsx! {
                      // 4列网格：标签(1列=25%) | 文章列表(3列=75%)
                      div { class: "grid grid-cols-1 lg:grid-cols-4 gap-6 lg:items-start",

                          // ── 左列：标签筛选 (sticky, 辅助内容) ──
                          div { class: "lg:sticky lg:top-20",
                              if all_tags.is_empty() {
                                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 p-8 text-center",
                                      p { class: "text-slate-400 text-sm", "{t(lang(), \"blog.empty\")}" }
                                  }
                              } else {
                                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 p-5",
                                      p { class: "text-xs font-semibold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-4", "{t(lang(), \"blog.filter\")}" }
                                      div { class: "flex flex-wrap gap-2",
                                          // 全部 chip
                                          {
                                              let is_all = active_tag().is_none();
                                              let label_all = t(lang(), "blog.all");
                                              rsx! {
                                                  button {
                                                      onclick: move |_| { active_tag.set(None); current_page.set(0); },
                                                      class: format_args!(
                                                          "inline-flex items-center gap-1 text-xs px-3 py-1 rounded-full font-medium transition-colors {}",
                                                          if is_all { "bg-blue-600 text-white" }
                                                          else { "bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-300 border border-slate-200 dark:border-slate-700 hover:border-blue-400 hover:text-blue-600" }
                                                      ),
                                                      "{label_all}"
                                                      span { class: "opacity-60", "{posts.len()}" }
                                                  }
                                              }
                                          }
                                          // 各 tag chip
                                          for tag in all_tags.iter() {
                                              {
                                                  let t = tag.clone();
                                                  let t2 = tag.clone();
                                                  let count = posts.iter().filter(|p| p.tags.contains(&t)).count();
                                                  let is_active = active_tag().as_deref() == Some(tag.as_str());
                                                  rsx! {
                                                      button {
                                                          key: "{t}",
                                                          onclick: move |_| { active_tag.set(Some(t.clone())); current_page.set(0); },
                                                          class: format_args!(
                                                              "inline-flex items-center gap-1 text-xs px-3 py-1 rounded-full font-medium transition-colors {}",
                                                              if is_active { "bg-blue-600 text-white" }
                                                              else { "bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-300 border border-slate-200 dark:border-slate-700 hover:border-blue-400 hover:text-blue-600" }
                                                          ),
                                                          "{t2}"
                                                          span { class: "opacity-60", "{count}" }
                                                      }
                                                  }
                                              }
                                          }
                                      }
                                  }
                              }
                          }

                          // ── 右列：文章列表 + 分页 (3/4 宽度, 主内容) ──
                          div { class: "lg:col-span-3 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 flex flex-col",
                              // 头部：标题 + 计数
                              div { class: "flex items-center justify-between px-5 pt-5 pb-3",
                                  h3 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200", "{t(lang(), \"blog.articles\")}" }
                                  span { class: "text-xs text-slate-400 tabular-nums", "{filtered.len()} / {posts.len()}" }
                              }
                              div { class: "border-t border-slate-200 dark:border-slate-800" }

                              // 文章列表
                              div { class: "divide-y divide-slate-200 dark:divide-slate-800",
                                  if paged.is_empty() {
                                      div { class: "text-center text-slate-400 text-sm py-10", "{t(lang(), \"blog.no_results\")}" }
                                  }
                                  for post in paged.iter() {
                                      Link {
                                          key: "{post.slug}",
                                          to: Route::Blog { id: post.slug.clone() },
                                          class: "group block px-5 py-4 hover:bg-white dark:hover:bg-slate-800/60 transition-all",
                                          div { class: "flex justify-between items-start gap-3",
                                              h3 { class: "text-sm font-semibold text-slate-900 dark:text-white group-hover:text-blue-600 transition-colors leading-snug flex-1",
                                                  "{post.title}"
                                              }
                                              if !post.date.is_empty() {
                                                  span { class: "text-xs text-slate-400 whitespace-nowrap shrink-0 mt-0.5", "{post.date}" }
                                              }
                                          }
                                          if !post.description.is_empty() {
                                              p { class: "text-xs text-slate-500 dark:text-slate-400 line-clamp-2 mt-1", "{post.description}" }
                                          }
                                          if !post.tags.is_empty() {
                                              div { class: "flex flex-wrap gap-1.5 mt-2",
                                                  for tag in post.tags.iter() {
                                                      span { class: "text-xs px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400",
                                                          "{tag}"
                                                      }
                                                  }
                                              }
                                          }
                                      }
                                  }
                              }

                              // 分页栏
                              if total_pages > 1 {
                                  div { class: "flex items-center justify-between px-5 py-3 border-t border-slate-200 dark:border-slate-800",
                                      button {
                                          disabled: safe_page == 0,
                                          onclick: move |_| { if current_page() > 0 { current_page.set(current_page() - 1); } },
                                          class: format_args!("px-3 py-1 rounded-lg text-sm transition-colors {}",
                                              if safe_page == 0 { "text-slate-300 dark:text-slate-600 cursor-not-allowed" }
                                              else { "text-slate-600 dark:text-slate-300 hover:bg-white dark:hover:bg-slate-800" }
                                          ),
                                          "←"
                                      }
                                      span { class: "text-xs text-slate-400 tabular-nums", "{safe_page + 1} / {total_pages}" }
                                      button {
                                          disabled: safe_page + 1 >= total_pages,
                                          onclick: move |_| { if current_page() + 1 < total_pages { current_page.set(current_page() + 1); } },
                                          class: format_args!("px-3 py-1 rounded-lg text-sm transition-colors {}",
                                              if safe_page + 1 >= total_pages { "text-slate-300 dark:text-slate-600 cursor-not-allowed" }
                                              else { "text-slate-600 dark:text-slate-300 hover:bg-white dark:hover:bg-slate-800" }
                                          ),
                                          "→"
                                      }
                                  }
                              }
                          }
                      }
                  },
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
  // 标注资源路径：resource_kind="blog"，resource_path = 博客 id
  let anno_path = id.clone();
  let anno_blog_id = format!("blog:{}", id);

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              div { class: "max-w-4xl mx-auto",
                  div { class: "text-slate-700 dark:text-slate-200 mb-12",
                      match blog_content() {
                          Some(Ok(content)) => rsx! {
                              Markdown { content: content.clone(), blog_id: anno_blog_id.clone() }
                              // 标注层（resource_kind="blog"，path = 博客 id）
                              AnnotationLayer {
                                  resource_kind: "blog".to_string(),
                                  resource_path: anno_path.clone(),
                              }
                          },
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
                      CommentBox { blog_id: id.clone() }
                  }

                  // 资源讨论面板：关联论坛话题
                  DiscussionPanel {
                      resource_kind: "blog".to_string(),
                      resource_path: id,
                  }
              }
          }
      }
  }
}

#[component]
pub fn Courses() -> Element {
  rsx! { CoursesIndexPage {} }
}

#[component]
pub fn CourseDetail(slug: String) -> Element {
  rsx! { CourseDetailPage { slug: slug } }
}

#[component]
pub fn Lesson(slug: String, chapter: String, lesson: String) -> Element {
  let lesson_path = format!("{}/{}/{}", slug, chapter, lesson);
  rsx! {
      LessonPage { slug: slug.clone(), chapter: chapter.clone(), lesson: lesson.clone() }
      // 资源讨论面板：关联论坛话题（以 lesson kind 记录）
      Container {
          DiscussionPanel {
              resource_kind: "lesson".to_string(),
              resource_path: lesson_path,
          }
      }
  }
}

#[component]
pub fn Cases() -> Element {
  rsx! { CasesIndexPage {} }
}

#[component]
pub fn CaseDetail(slug: String) -> Element {
  rsx! { CaseDetailPage { slug } }
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
  rsx! { TopicsIndexPage {} }
}

#[component]
pub fn TopicsNew() -> Element {
  rsx! { NewTopicPage {} }
}

#[component]
pub fn TopicsByTag(tag: String) -> Element {
  rsx! { TopicsByTagPage { tag } }
}

#[component]
pub fn TopicDetail(id: i32) -> Element {
  rsx! { TopicDetailPage { id } }
}

#[component]
pub fn MyTopics() -> Element {
  rsx! { MyTopicsPage {} }
}

#[component]
pub fn Podcast() -> Element {
  rsx! {
      PodcastPage {}
  }
}

/// /me/annotations 个人标注列表页
#[component]
pub fn MyAnnotations() -> Element {
  rsx! { MyAnnotationsPage {} }
}

#[component]
pub fn AdminDashboard() -> Element {
  rsx! { AdminDashboardPage {} }
}

#[component]
pub fn AdminUsers() -> Element {
  rsx! { AdminUsersPage {} }
}

#[component]
pub fn AdminComments() -> Element {
  rsx! { AdminCommentsPage {} }
}

#[component]
pub fn AdminTopics() -> Element {
  rsx! { AdminTopicsPage {} }
}

#[component]
pub fn AdminPlugins() -> Element {
  rsx! { AdminPluginsPage {} }
}
