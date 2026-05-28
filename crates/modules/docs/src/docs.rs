use dioxus::prelude::*;

use crate::server::{get_doc_content, list_doc_tree, DocTreeNode};
use rustineverything_module_course::course::AnnotationLayer;
use rustineverything_module_forum::forum::DiscussionPanel;
use rustineverything_widgets::Markdown;

/// 给定文档路径段，构造 SPA URL（`/docs/<path>`）。
fn doc_href(path: &str) -> String {
  if path.is_empty() {
    "/docs".to_string()
  } else {
    format!("/docs/{}", path)
  }
}

/// /docs 文档首页：左侧一级分类列表 + 右侧欢迎页和分类卡片
#[component]
pub fn Docs() -> Element {
  let tree = use_resource(move || async move { list_doc_tree().await.unwrap_or_default() });

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
  let href = if node.has_content { doc_href(&node.path) } else { "/docs".to_string() };
  let child_count = count_leaves(node);

  rsx! {
      a {
          href: "{href}",
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
  let href = if node.has_content { doc_href(&node.path) } else { "/docs".to_string() };
  let child_count = count_leaves(node);

  // 子项标题列表（最多显示 5 个）
  let child_titles: Vec<String> = node.children.iter().take(5).map(|c| c.title.clone()).collect();
  let has_more = node.children.len() > 5;

  rsx! {
      a {
          href: "{href}",
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

  let tree = use_resource(move || async move { list_doc_tree().await.unwrap_or_default() });

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
pub fn TreeSection(node: DocTreeNode, active_path: String, depth: u32) -> Element {
  let has_children = !node.children.is_empty();
  let is_active = active_path == node.path;
  let is_within = is_path_within(&node, &active_path);

  // 当前页所在的分支默认展开
  let mut expanded = use_signal(move || is_within);

  let href = doc_href(&node.path);

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
                      a {
                          href: "{href}",
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
                      a {
                          href: "{href}",
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
              a {
                  href: "{href}",
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
