//! Cli 板块的落地页与文章详情页。导航用 `<a href>`，避免对 app `Route` 的循环依赖。
//!
//! 重构 B3：数据获取从 `use_resource` 迁移到 `use_server_future`，由服务端预取并随
//! SSR HTML 下发，客户端 hydration 直接拿到内容（消除首屏 spinner + 二次抓取）。
//! 列表的子主题筛选 / 搜索仍是客户端 signal 交互，置于 `SuspenseBoundary` 内的子组件。

use dioxus::prelude::*;
use widgets::{parse_mdx, Markdown};

use app_core::i18n::{t, Language};

use crate::server::{get_cli_article, list_cli_articles, ArticleSummary};
use crate::text::{matches_query, normalize_tag, BOARD_ID, BOARD_ROUTE, FEATURED_CRATES, SUBTOPICS};

/// 读取全局语言信号（缺省回退 Zh）。方案 A：板块文案随该信号切换。
fn current_lang() -> Language {
  try_consume_context::<Signal<Language>>().map(|s| s()).unwrap_or_default()
}

#[component]
pub fn CliIndexPage() -> Element {
  let lang = current_lang();
  let label = t(lang, &format!("{BOARD_ID}.label"));
  let tagline = t(lang, &format!("{BOARD_ID}.tagline"));
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "max-w-6xl mx-auto px-4 sm:px-6",
              div { class: "mb-10",
                  h1 { class: "text-3xl sm:text-4xl font-extrabold tracking-tight text-slate-900 dark:text-white", "{label}" }
                  p { class: "mt-3 text-lg text-slate-500 dark:text-slate-400 max-w-2xl", "{tagline}" }
              }
              // 文章列表经 use_server_future 服务端预取；SuspenseBoundary 在未就绪时渲染 spinner。
              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                      }
                  },
                  CliIndexList {}
              }
          }
      }
  }
}

/// 文章列表（重构 B3）：`list_cli_articles` 经 `use_server_future` 服务端预取，
/// 子主题筛选 / 搜索仍是客户端 signal 交互。置于 SuspenseBoundary 内。
#[component]
fn CliIndexList() -> Element {
  let lang = current_lang();
  let articles_res =
    use_server_future(|| async move { list_cli_articles().await.unwrap_or_default() })?;
  let articles: Vec<ArticleSummary> = articles_res().unwrap_or_default();

  let mut active_subtopic = use_signal(String::new);
  let mut query = use_signal(String::new);

  let q = query();
  let sub = active_subtopic();
  let filtered: Vec<ArticleSummary> = articles
    .iter()
    .filter(|a| sub.is_empty() || a.subtopic == sub)
    .filter(|a| matches_query(&a.title, &a.description, &a.tags, &q))
    .cloned()
    .collect();

  rsx! {
      div { class: "mb-6",
          input {
              r#type: "search",
              class: "w-full max-w-md px-4 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white",
              placeholder: "{t(lang, \"board.search\")}",
              value: "{query}",
              oninput: move |e| query.set(e.value()),
          }
      }

      div { class: "flex flex-wrap gap-2 mb-8",
          button {
              class: if sub.is_empty() {
                  "px-3 py-1.5 rounded-full text-sm font-semibold bg-blue-600 text-white"
              } else {
                  "px-3 py-1.5 rounded-full text-sm font-medium bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700"
              },
              onclick: move |_| active_subtopic.set(String::new()),
              "{t(lang, \"board.all\")}"
          }
          for s in SUBTOPICS.iter() {
              {
                  let slug = s.slug.to_string();
                  let is_active = sub == s.slug;
                  let chip = t(lang, &format!("{}.sub.{}.label", BOARD_ID, s.slug));
                  let blurb = t(lang, &format!("{}.sub.{}.blurb", BOARD_ID, s.slug));
                  rsx! {
                      button {
                          class: if is_active {
                              "px-3 py-1.5 rounded-full text-sm font-semibold bg-blue-600 text-white"
                          } else {
                              "px-3 py-1.5 rounded-full text-sm font-medium bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700"
                          },
                          title: "{blurb}",
                          onclick: move |_| active_subtopic.set(slug.clone()),
                          "{chip}"
                      }
                  }
              }
          }
      }

      div { class: "grid grid-cols-1 lg:grid-cols-3 gap-8",
          div { class: "lg:col-span-2",
              if filtered.is_empty() {
                  div { class: "py-16 text-center text-slate-400",
                      "{t(lang, \"board.empty\")}"
                  }
              } else {
                  div { class: "space-y-4",
                      for a in filtered.iter() {
                          ArticleCard { key: "{a.slug}", article: a.clone() }
                      }
                  }
              }
          }

          aside {
              h2 { class: "text-sm font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 mb-4", "{t(lang, \"board.featured\")}" }
              div { class: "space-y-3",
                  for c in FEATURED_CRATES.iter() {
                      {
                          let blurb = t(lang, &format!("{}.crate.{}.blurb", BOARD_ID, normalize_tag(c.name)));
                          rsx! {
                              a {
                                  href: "{c.url}",
                                  target: "_blank",
                                  rel: "noopener noreferrer",
                                  class: "block p-3 rounded-lg border border-slate-200 dark:border-slate-800 hover:border-blue-400 dark:hover:border-blue-600 transition-colors",
                                  div { class: "font-mono text-sm font-bold text-slate-900 dark:text-white", "{c.name}" }
                                  div { class: "text-xs text-slate-500 dark:text-slate-400 mt-0.5", "{blurb}" }
                              }
                          }
                      }
                  }
              }
          }
      }
  }
}

#[component]
fn ArticleCard(article: ArticleSummary) -> Element {
  let lang = current_lang();
  let href = format!("{}/{}", BOARD_ROUTE, article.slug);
  let known = SUBTOPICS.iter().any(|s| s.slug == article.subtopic);
  let sub = if known {
    t(lang, &format!("{}.sub.{}.label", BOARD_ID, article.subtopic))
  } else {
    String::new()
  };
  rsx! {
      a {
          href: "{href}",
          class: "block p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/40 hover:shadow-md hover:border-blue-300 dark:hover:border-blue-700 transition-all",
          div { class: "flex items-center gap-2 mb-2 text-xs",
              if !sub.is_empty() {
                  span { class: "px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 font-medium", "{sub}" }
              }
              span { class: "text-slate-400", "{article.date}" }
          }
          h3 { class: "text-lg font-bold text-slate-900 dark:text-white", "{article.title}" }
          p { class: "mt-1 text-sm text-slate-600 dark:text-slate-400", "{article.description}" }
          if !article.tags.is_empty() {
              div { class: "mt-3 flex flex-wrap gap-1.5",
                  for tag in article.tags.iter() {
                      span { class: "text-xs px-2 py-0.5 rounded bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400", "#{tag}" }
                  }
              }
          }
      }
  }
}

#[component]
pub fn CliArticlePage(slug: String) -> Element {
  let lang = current_lang();
  let back = format!("{} {}", t(lang, "board.back_prefix"), t(lang, &format!("{BOARD_ID}.label")));
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "max-w-4xl mx-auto px-4 sm:px-6",
              a {
                  href: "{BOARD_ROUTE}",
                  class: "inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-700 mb-8",
                  "{back}"
              }
              div { class: "text-slate-700 dark:text-slate-200",
                  // 正文经 use_server_future 服务端预取（随 SSR HTML 下发）。
                  SuspenseBoundary {
                      fallback: |_| rsx! {
                          div { class: "flex items-center justify-center py-20",
                              div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                          }
                      },
                      CliArticleContent { slug: slug.clone() }
                  }
              }
          }
      }
  }
}

/// 文章正文（重构 B3）：`get_cli_article` 经 `use_server_future` + `use_reactive!` 服务端预取，
/// 随路由参数 slug 变化重取。置于 SuspenseBoundary 内。
#[component]
fn CliArticleContent(slug: String) -> Element {
  let lang = current_lang();
  let content_res =
    use_server_future(use_reactive!(|slug| async move { get_cli_article(slug).await }))?;
  match content_res() {
    Some(Ok(content)) => {
      let (_meta, _body) = parse_mdx(&content);
      rsx! {
          Markdown { content: content.clone(), blog_id: slug.clone() }
      }
    }
    Some(Err(e)) => {
      let msg = format!("{}{}", t(lang, "board.load_error_prefix"), e);
      rsx! {
          div { class: "p-4 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 rounded-lg", "{msg}" }
      }
    }
    None => rsx! {
        div { class: "flex items-center justify-center py-20",
            div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
        }
    },
  }
}
