use dioxus::prelude::*;
use dioxus_shadcn::{
  badge_class, card_class, BadgeVariant, Empty, EmptyDescription, Spinner, SpinnerSize, TreeItem,
};

use crate::server::{get_doc_content, list_doc_tree, DocTreeNode};
use widgets::Markdown;

use app_core::i18n::{t, Language};

/// 当前语言（缺省 Zh）。O11：文档页 UI 文案随语言切换。
fn current_lang() -> Language {
  try_consume_context::<Signal<Language>>().map(|s| s()).unwrap_or_default()
}

/// 给定文档路径段，构造 SPA URL（`/docs/<path>`）。
fn doc_href(path: &str) -> String {
  if path.is_empty() {
    "/docs".to_string()
  } else {
    format!("/docs/{}", path)
  }
}

/// /docs 文档首页：左侧一级分类列表 + 右侧欢迎页和分类卡片。
/// 重构 B4：树形导航改由 `DocsIndexInner` 用 use_server_future 服务端预取（随 SSR HTML 下发）。
#[component]
pub fn Docs() -> Element {
  let lang = current_lang();
  rsx! {
      widgets::PageMeta {
          title: t(lang, "docs.title"),
          description: t(lang, "docs.subtitle"),
          path: "/docs".to_string(),
      }
      section { class: "min-h-screen bg-white dark:bg-slate-950",
          SuspenseBoundary {
              fallback: |_| rsx! {
                  div { class: "flex items-center justify-center py-20",
                      Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                  }
              },
              DocsIndexInner {}
          }
      }
  }
}

#[component]
fn DocsIndexInner() -> Element {
  let lang = current_lang();
  let tree = use_server_future(|| async move { list_doc_tree().await.unwrap_or_default() })?;

  rsx! {
      div { class: "max-w-7xl mx-auto flex",
              // 左侧导航：一级分类列表（仅桌面端显示）
              aside { class: "hidden lg:block shrink-0 w-64 sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 pt-8 pb-12 px-4",
                  nav {
                      h3 { class: "text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 px-3 mb-4", "{t(lang, \"docs.nav\")}" }
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
                          h1 { class: "text-3xl font-extrabold text-slate-900 dark:text-white mb-3", "{t(lang, \"docs.title\")}" }
                          p { class: "text-lg text-slate-600 dark:text-slate-400 leading-relaxed",
                              "{t(lang, \"docs.subtitle\")}"
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
                              Empty { class: "py-20", EmptyDescription { "{t(lang, \"docs.empty\")}" } }
                          },
                          None => rsx! {
                              div { class: "flex items-center justify-center py-20",
                                  Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                              }
                          },
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
      Link {
          to: href.clone(),
          class: "flex items-center justify-between px-3 py-2.5 rounded-lg text-sm font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors mb-1",
          span { "{node.title}" }
          span { class: badge_class(BadgeVariant::Secondary, "rounded-full px-1.5 font-normal text-muted-foreground"),
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
  let lang = current_lang();
  let href = if node.has_content { doc_href(&node.path) } else { "/docs".to_string() };
  let child_count = count_leaves(node);

  // 子项标题列表（最多显示 5 个）
  let child_titles: Vec<String> = node.children.iter().take(5).map(|c| c.title.clone()).collect();
  let has_more = node.children.len() > 5;

  rsx! {
      Link {
          to: href.clone(),
          class: card_class("group block p-6 rounded-2xl hover:border-primary/50 hover:shadow-lg transition-all"),
          // 标题行
          div { class: "flex items-center justify-between mb-3",
              h3 { class: "text-lg font-bold text-slate-900 dark:text-white group-hover:text-primary transition-colors",
                  "{node.title}"
              }
              span { class: badge_class(BadgeVariant::Secondary, "rounded-full font-normal text-muted-foreground"),
                  "{child_count} {t(lang, \"docs.count_unit\")}"
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
                      li { class: "text-xs text-slate-400 italic", "{t(lang, \"docs.more\")}" }
                  }
              }
          }
          // 底部箭头
          div { class: "mt-4 flex items-center text-sm font-medium text-primary opacity-0 group-hover:opacity-100 transition-opacity",
              "{t(lang, \"docs.start_reading\")}"
              svg { class: "w-4 h-4 ml-1", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                  path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M9 5l7 7-7 7" }
              }
          }
      }
  }
}

/// 文档页面：左侧树形导航 + 右侧 Markdown 内容（手机上导航收进抽屉，`ReadingLayout`，T3）。
///
/// `footer` 插槽：标注层（AnnotationLayer）与讨论面板（DiscussionPanel）是跨模块
/// 组合（分属 course / forum），为避免 docs 模块编译期依赖兄弟模块，这两个组件
/// 由组合根 `app` 装配后通过该插槽注入；docs 自身只渲染内容主体。
/// 侧栏（`DocTree`）与正文（`DocPageInner`）都由 use_server_future 服务端预取
/// （随 SSR HTML 下发），各在一个 SuspenseBoundary 里：换页只有正文重取，侧栏留在原处不闪。
#[component]
pub fn DocPage(path: Vec<String>, footer: Element) -> Element {
  let doc_path = path.join("/");
  rsx! {
      section { class: "min-h-screen bg-white dark:bg-slate-950",
          widgets::ReadingLayout {
              path: doc_path.clone(),
              nav_label: t(current_lang(), "docs.nav"),
              nav: rsx! {
                  SuspenseBoundary { fallback: |_| VNode::empty(),
                      DocTree { active_path: doc_path.clone() }
                  }
              },
              // 正文 + 页内目录（DocPageInner 里，目录来自正文）
              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                      }
                  },
                  DocPageInner { path: path.clone(), footer: footer.clone() }
              }
          }
      }
  }
}

/// 侧栏树：服务端预取。用 use_resource 时 hydration 那一刻它还是 None，
/// SSR 出来的侧栏不会被接管，里面的 Link 点下去是整页刷新。
/// 有正文的节点值为站内路径（可导航），没有正文的值为 `group:<path>`（只展开）。
#[component]
fn DocTree(active_path: String) -> Element {
  let tree = use_server_future(|| async move { list_doc_tree().await.unwrap_or_default() })?;
  let Some(nodes) = tree() else {
    return VNode::empty();
  };
  // 当前页所在的分支（含当前页自己）都展开
  let mut expand = Vec::new();
  collect_within(&nodes, &active_path, &mut expand);
  rsx! {
      nav { "aria-label": "{t(current_lang(), \"docs.nav\")}",
          widgets::NavTree {
              label: t(current_lang(), "docs.nav"),
              active: doc_href(&active_path),
              expand,
              for node in nodes.iter() {
                  {doc_tree_item(node, 1)}
              }
          }
      }
  }
}

fn tree_value(node: &DocTreeNode) -> String {
  if node.has_content {
    doc_href(&node.path)
  } else {
    format!("group:{}", node.path)
  }
}

/// `active_path` 所在的节点及其所有祖先的树值。
fn collect_within(nodes: &[DocTreeNode], active_path: &str, out: &mut Vec<String>) {
  for node in nodes {
    if is_path_within(node, active_path) {
      out.push(tree_value(node));
      collect_within(&node.children, active_path, out);
    }
  }
}

fn doc_tree_item(node: &DocTreeNode, level: u32) -> Element {
  let row_class = if level == 1 {
    "text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 [[aria-selected=true]>&]:text-primary"
  } else {
    "text-sm text-slate-700 dark:text-slate-300 [[aria-selected=true]>&]:text-primary"
  };
  let group = (!node.children.is_empty()).then(|| {
    rsx! {
        for child in node.children.iter() {
            {doc_tree_item(child, level + 1)}
        }
    }
  });
  rsx! {
      TreeItem { value: tree_value(node), class: row_class, group,
          if node.has_content {
              Link { to: doc_href(&node.path), class: "flex-1 min-w-0 py-0.5", "{node.title}" }
          } else {
              span { class: "flex-1 min-w-0", "{node.title}" }
          }
      }
  }
}

#[component]
fn DocPageInner(path: Vec<String>, footer: Element) -> Element {
  let lang = current_lang();
  let doc_path = path.join("/");
  let doc_path_for_meta = doc_path.clone();
  // Markdown blog_id 携带 "doc:<path>" 前缀，JS 运行时以此识别资源归属
  let anno_blog_id = format!("doc:{}", doc_path);

  // 正文：use_server_future 服务端预取，随路由参数 doc_path 变化重取。
  let content =
    use_server_future(use_reactive!(|doc_path| async move { get_doc_content(doc_path).await }))?;

  rsx! {
      match content() {
          Some(Ok(Some(resp))) => rsx! {
              div { class: "flex justify-center gap-10",
                  article { class: "min-w-0 flex-1 max-w-3xl",
                      // SEO：title / description / canonical / og 由 PageMeta 输出（服务端已从正文补齐），
                      // 这里只补 keywords 与 og:image。
                      widgets::PageMetaTags {
                          title: resp.meta.title.clone(),
                          description: resp.meta.description.clone(),
                          path: format!("/docs/{}", doc_path_for_meta),
                      }
                      if !resp.meta.keywords.is_empty() {
                          document::Meta { name: "keywords", content: "{resp.meta.keywords.join(\", \")}" }
                      }
                      if let Some(ref img) = resp.meta.image {
                          document::Meta { property: "og:image", content: "{img}" }
                      }
                      if let Some(ref src) = resp.meta.source {
                          a {
                              href: "https://docs.rs/{src.krate}/{src.version}",
                              target: "_blank",
                              rel: "noopener",
                              class: badge_class(BadgeVariant::Secondary, "mb-4 rounded-full font-normal text-muted-foreground"),
                              "{t(lang, \"docs.based_on\")} {src.krate} {src.version}"
                          }
                      }
                      div { class: "text-slate-700 dark:text-slate-200",
                          Markdown { content: resp.content.clone(), blog_id: anno_blog_id.clone() }
                      }
                      if resp.prev.is_some() || resp.next.is_some() {
                          nav { class: "mt-12 pt-6 border-t border-slate-200 dark:border-slate-800 grid grid-cols-1 sm:grid-cols-2 gap-4",
                              if let Some(ref link) = resp.prev {
                                  Link {
                                      to: doc_href(&link.path),
                                      rel: "prev",
                                      class: "block rounded-lg border border-slate-200 dark:border-slate-800 p-4 hover:border-primary transition-colors",
                                      span { class: "block text-xs text-slate-500 dark:text-slate-400", "← {t(lang, \"docs.prev\")}" }
                                      span { class: "block mt-1 font-medium text-slate-900 dark:text-white", "{link.title}" }
                                  }
                              }
                              if let Some(ref link) = resp.next {
                                  Link {
                                      to: doc_href(&link.path),
                                      rel: "next",
                                      class: "block rounded-lg border border-slate-200 dark:border-slate-800 p-4 hover:border-primary transition-colors text-right sm:col-start-2",
                                      span { class: "block text-xs text-slate-500 dark:text-slate-400", "{t(lang, \"docs.next\")} →" }
                                      span { class: "block mt-1 font-medium text-slate-900 dark:text-white", "{link.title}" }
                                  }
                              }
                          }
                      }
                      // 跨模块组合（标注层 + 讨论面板）由 app 层注入。
                      {footer.clone()}
                  }
                  aside { class: "hidden xl:block w-56 shrink-0",
                      div { class: "sticky top-20 max-h-[calc(100vh-6rem)] overflow-y-auto",
                          widgets::OnThisPage {
                              entries: widgets::heading_outline(&resp.content),
                              title: t(lang, "toc.title"),
                          }
                      }
                  }
              }
          },
          Some(Ok(None)) => rsx! { widgets::NotFound { message: t(lang, "docs.not_found") } },
          Some(Err(_)) => rsx! { widgets::LoadFailed { message: t(lang, "common.load_failed") } },
          None => rsx! {
              div { class: "flex items-center justify-center py-20",
                  Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
              }
          },
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
