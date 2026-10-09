//! Wasm 板块的落地页与文章详情页。导航用 `<a href>`，避免对 app `Route` 的循环依赖。
//!
//! 重构 B3：数据获取从 `use_resource` 迁移到 `use_server_future`，由服务端预取并随
//! SSR HTML 下发，客户端 hydration 直接拿到内容（消除首屏 spinner + 二次抓取）。
//! 列表的子主题筛选 / 搜索仍是客户端 signal 交互，置于 `SuspenseBoundary` 内的子组件。

use dioxus::prelude::*;
use dioxus_shadcn::{
  badge_class, card_class, Alert, AlertDescription, AlertVariant, BadgeVariant, Empty,
  EmptyDescription, Input, Spinner, SpinnerSize,
};
use widgets::{parse_mdx, Markdown};

use app_core::i18n::{t, Language};

use crate::server::{get_wasm_article, list_wasm_articles, ArticleSummary};
use crate::text::{
  matches_query, normalize_tag, BOARD_ID, BOARD_ROUTE, FEATURED_CRATES, SUBTOPICS,
};

/// 读取全局语言信号（缺省回退 Zh）。方案 A：板块文案随该信号切换。
fn current_lang() -> Language {
  try_consume_context::<Signal<Language>>().map(|s| s()).unwrap_or_default()
}

/// 子主题筛选 chip：选中为主色实心，否则为次要底色。
fn chip_class(active: bool) -> String {
  if active {
    badge_class(BadgeVariant::Default, "rounded-full px-3 py-1.5 text-sm")
  } else {
    badge_class(
      BadgeVariant::Secondary,
      "rounded-full px-3 py-1.5 text-sm font-medium hover:bg-accent",
    )
  }
}

#[component]
pub fn WasmIndexPage() -> Element {
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
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                      }
                  },
                  WasmIndexList {}
              }
          }
      }
  }
}

/// 文章列表（重构 B3）：`list_wasm_articles` 经 `use_server_future` 服务端预取，
/// 子主题筛选 / 搜索仍是客户端 signal 交互。置于 SuspenseBoundary 内。
#[component]
fn WasmIndexList() -> Element {
  let lang = current_lang();
  let articles_res =
    use_server_future(|| async move { list_wasm_articles().await.unwrap_or_default() })?;
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
          Input {
              r#type: "search",
              class: "max-w-md",
              placeholder: t(lang, "board.search"),
              "aria-label": t(lang, "board.search"),
              value: query(),
              on_value_change: move |v: String| query.set(v),
          }
      }

      div { class: "flex flex-wrap gap-2 mb-8",
          button {
              r#type: "button",
              "aria-pressed": sub.is_empty().to_string(),
              class: chip_class(sub.is_empty()),
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
                          r#type: "button",
                          "aria-pressed": is_active.to_string(),
                          class: chip_class(is_active),
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
                  Empty { class: "py-16",
                      EmptyDescription { "{t(lang, \"board.empty\")}" }
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
                                  class: "block p-3 rounded-lg border border-border hover:border-primary/50 transition-colors",
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
          class: card_class("block p-5 rounded-xl shadow-none hover:shadow-md hover:border-primary/50 transition-all"),
          div { class: "flex items-center gap-2 mb-2 text-xs",
              if !sub.is_empty() {
                  span { class: badge_class(BadgeVariant::Secondary, "font-medium text-primary"), "{sub}" }
              }
              span { class: "text-slate-400", "{article.date}" }
          }
          h3 { class: "text-lg font-bold text-slate-900 dark:text-white", "{article.title}" }
          p { class: "mt-1 text-sm text-slate-600 dark:text-slate-400", "{article.description}" }
          if !article.tags.is_empty() {
              div { class: "mt-3 flex flex-wrap gap-1.5",
                  for tag in article.tags.iter() {
                      span { class: badge_class(BadgeVariant::Secondary, "font-normal text-muted-foreground"), "#{tag}" }
                  }
              }
          }
      }
  }
}

#[component]
pub fn WasmArticlePage(slug: String) -> Element {
  let lang = current_lang();
  let back = format!("{} {}", t(lang, "board.back_prefix"), t(lang, &format!("{BOARD_ID}.label")));
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "max-w-4xl mx-auto px-4 sm:px-6",
              a {
                  href: "{BOARD_ROUTE}",
                  class: "inline-flex items-center gap-1 text-sm text-primary hover:underline mb-8",
                  "{back}"
              }
              div { class: "text-slate-700 dark:text-slate-200",
                  // 正文经 use_server_future 服务端预取（随 SSR HTML 下发）。
                  SuspenseBoundary {
                      fallback: |_| rsx! {
                          div { class: "flex items-center justify-center py-20",
                              Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                          }
                      },
                      WasmArticleContent { slug: slug.clone() }
                  }
              }
          }
      }
  }
}

/// 文章正文（重构 B3）：`get_wasm_article` 经 `use_server_future` + `use_reactive!` 服务端预取，
/// 随路由参数 slug 变化重取。置于 SuspenseBoundary 内。
#[component]
fn WasmArticleContent(slug: String) -> Element {
  let lang = current_lang();
  let content_res =
    use_server_future(use_reactive!(|slug| async move { get_wasm_article(slug).await }))?;
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
          Alert { variant: AlertVariant::Destructive,
              AlertDescription { variant: AlertVariant::Destructive, "{msg}" }
          }
      }
    }
    None => rsx! {
        div { class: "flex items-center justify-center py-20",
            Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
        }
    },
  }
}
