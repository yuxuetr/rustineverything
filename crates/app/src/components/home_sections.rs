//! 首页分区组件（M2）。
//!
//! 围绕「Rust 生态 + AI 生态」双支柱组织首页：生态 pillars、精选案例、课程、
//! 社区动态等。数据来源沿用各模块 server fn；SEO 关键区用 `use_server_future`
//! + `SuspenseBoundary`（与内容页一致），社区/DB 区用 `use_resource`。
//! 详见 `docs/SITE_REDESIGN_SPEC.md` §4。

use dioxus::prelude::*;
use dioxus::router::Link;

use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use crate::routes::Route;
use crate::taxonomy::{ecosystem_by_id, ecosystem_of_case_category, ecosystems};
use module_blog::server::{list_blog_posts, BlogPostSummary};
use module_cases::server::{list_cases, CaseSummary};
use module_course::server::{list_courses, CourseSummary};
use module_forum::server::{list_topics, TopicSummary};

/// 两大生态 pillars：Rust 生态 | AI 生态，各列子领域 chips（链接到领域路由）。
/// `enabled` 用于隐藏被关闭模块对应的领域，与导航保持一致。
#[component]
pub fn EcosystemPillars(enabled: Vec<String>) -> Element {
  let lang = use_i18n();
  rsx! {
      section { class: "py-16 sm:py-20 bg-slate-50/60 dark:bg-slate-900/40 border-y border-slate-200/70 dark:border-slate-800",
          Container {
              div { class: "grid grid-cols-1 md:grid-cols-2 gap-6",
                  for eco in ecosystems() {
                      {
                          let domains: Vec<_> = eco.domains.iter().filter(|d| enabled.iter().any(|e| e == d.module_id)).cloned().collect();
                          rsx! {
                              div { key: "{eco.id}", class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-7 flex flex-col",
                                  h3 { class: "text-2xl font-extrabold tracking-tight text-flow", "{t(lang(), eco.label_key)}" }
                                  p { class: "mt-2 text-slate-600 dark:text-slate-400 leading-relaxed", "{t(lang(), eco.blurb_key)}" }
                                  div { class: "mt-5 flex flex-wrap gap-2",
                                      for d in domains.iter() {
                                          Link {
                                              key: "{d.id}",
                                              to: d.route.clone(),
                                              class: "inline-flex items-center rounded-full border border-slate-200 dark:border-slate-700 px-3 py-1 text-sm text-slate-700 dark:text-slate-200 hover:border-[var(--color-primary)] hover:text-[var(--color-primary)] transition-colors",
                                              "{t(lang(), d.label_key)}"
                                          }
                                      }
                                  }
                                  Link {
                                      to: Route::EcosystemPage { id: eco.id.to_string() },
                                      class: "mt-6 inline-flex items-center gap-1 text-sm font-semibold text-[var(--color-primary)] hover:underline",
                                      "{t(lang(), \"eco.enter\")}"
                                      svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "2",
                                          path { stroke_linecap: "round", stroke_linejoin: "round", d: "M9 5l7 7-7 7" }
                                      }
                                  }
                              }
                          }
                      }
                  }
              }
          }
      }
  }
}

/// 分区头部：左标题/副标题 + 右「查看全部」链接（复用于精选案例 / 课程）。
#[component]
fn SectionHeader(title: String, subtitle: String, all_label: String, to: Route) -> Element {
  rsx! {
      div { class: "flex items-end justify-between gap-4 mb-10",
          div {
              h2 { class: "text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-900 dark:text-white", "{title}" }
              p { class: "mt-2 text-slate-600 dark:text-slate-400", "{subtitle}" }
          }
          Link {
              to,
              class: "shrink-0 hidden sm:inline-flex items-center gap-1 text-sm font-medium text-[var(--color-primary)] hover:underline",
              "{all_label}"
              svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "2",
                  path { stroke_linecap: "round", stroke_linejoin: "round", d: "M9 5l7 7-7 7" }
              }
          }
      }
  }
}

/// 加载占位 spinner（SEO 区数据 SSR 预取，spinner 仅作 hydration 兜底）。
fn loading_spinner() -> Element {
  rsx! {
      div { class: "flex items-center justify-center py-16",
          div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-[var(--color-primary)]" }
      }
  }
}

// ── 精选案例 ──────────────────────────────────────────────

/// 精选案例分区：旗舰位，展示工业实践 Rust 项目（`cases.favorite`）。
#[component]
pub fn FeaturedCases() -> Element {
  let lang = use_i18n();
  rsx! {
      section { class: "py-20 bg-white dark:bg-slate-950",
          Container {
              SectionHeader {
                  title: t(lang(), "home.featured.title"),
                  subtitle: t(lang(), "home.featured.subtitle"),
                  all_label: t(lang(), "home.featured.all"),
                  to: Route::Cases {},
              }
              SuspenseBoundary {
                  fallback: |_| loading_spinner(),
                  FeaturedCasesInner {}
              }
          }
      }
  }
}

#[component]
fn FeaturedCasesInner() -> Element {
  let lang = use_i18n();
  let res =
    use_server_future(|| async move { list_cases(None, None, None).await.unwrap_or_default() })?;
  let all = res().unwrap_or_default();
  // 优先精选；无精选则回退到最新 6 条，保证旗舰位不空。
  let mut picks: Vec<CaseSummary> = all.iter().filter(|c| c.favorite).cloned().collect();
  if picks.is_empty() {
    picks = all.clone();
  }
  let picks: Vec<CaseSummary> = picks.into_iter().take(6).collect();

  rsx! {
      if picks.is_empty() {
          p { class: "text-center text-slate-400 py-10", "{t(lang(), \"home.featured.empty\")}" }
      } else {
          div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6",
              for c in picks.into_iter() {
                  CaseCard { key: "{c.slug}", case: c }
              }
          }
      }
  }
}

#[component]
fn CaseCard(case: CaseSummary) -> Element {
  let first = case.name.chars().next().unwrap_or('R');
  rsx! {
      Link {
          to: Route::CaseDetail { slug: case.slug.clone() },
          class: "group flex flex-col rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden hover:shadow-lg transition-all",
          if let Some(cover) = case.cover_url.clone() {
              img { src: "{cover}", class: "h-36 w-full object-cover", alt: "{case.name}", loading: "lazy" }
          } else {
              div { class: "h-36 w-full bg-linear-to-br from-slate-100 to-slate-200 dark:from-slate-800 dark:to-slate-950 flex items-center justify-center text-3xl font-extrabold text-slate-300 dark:text-slate-600",
                  "{first}"
              }
          }
          div { class: "p-5 flex flex-col flex-1",
              div { class: "flex items-center gap-2 mb-2",
                  span { class: "text-xs px-2 py-0.5 rounded-full bg-[var(--color-primary)]/10 text-[var(--color-primary)] font-medium", "{case.category}" }
                  if case.stars > 0 {
                      span { class: "inline-flex items-center gap-0.5 text-xs text-slate-400", "★ {case.stars}" }
                  }
              }
              h3 { class: "font-bold text-slate-900 dark:text-white group-hover:text-[var(--color-primary)] transition-colors", "{case.name}" }
              p { class: "mt-1 text-sm text-slate-600 dark:text-slate-400 line-clamp-2 flex-1", "{case.description}" }
          }
      }
  }
}

// ── 课程 ──────────────────────────────────────────────────

/// 课程分区：体系化 Rust / AI 实战课程（付费层级/资源徽章见 M4）。
#[component]
pub fn CourseShowcase() -> Element {
  let lang = use_i18n();
  rsx! {
      section { class: "py-20 bg-slate-50/60 dark:bg-slate-900/40 border-y border-slate-200/70 dark:border-slate-800",
          Container {
              SectionHeader {
                  title: t(lang(), "home.courses.title"),
                  subtitle: t(lang(), "home.courses.subtitle"),
                  all_label: t(lang(), "home.courses.all"),
                  to: Route::Courses {},
              }
              SuspenseBoundary {
                  fallback: |_| loading_spinner(),
                  CourseShowcaseInner {}
              }
          }
      }
  }
}

#[component]
fn CourseShowcaseInner() -> Element {
  let lang = use_i18n();
  let res = use_server_future(|| async move { list_courses().await.unwrap_or_default() })?;
  let courses = res().unwrap_or_default();
  let picks: Vec<CourseSummary> = courses.into_iter().take(3).collect();

  rsx! {
      if picks.is_empty() {
          p { class: "text-center text-slate-400 py-10", "{t(lang(), \"home.courses.empty\")}" }
      } else {
          div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6",
              for c in picks.into_iter() {
                  CourseCard { key: "{c.slug}", course: c }
              }
          }
      }
  }
}

#[component]
fn CourseCard(course: CourseSummary) -> Element {
  let lang = use_i18n();
  let first = course.title.chars().next().unwrap_or('R');
  let lessons = format!("{} {}", course.lesson_count, lang().pick("节课", "lessons"));
  rsx! {
      Link {
          to: Route::CourseDetail { slug: course.slug.clone() },
          class: "group flex flex-col rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden hover:shadow-lg transition-all",
          if let Some(cover) = course.cover.clone() {
              img { src: "{cover}", class: "h-36 w-full object-cover", alt: "{course.title}", loading: "lazy" }
          } else {
              div { class: "h-36 w-full bg-linear-to-br from-[var(--color-primary)]/15 to-[var(--color-primary)]/5 flex items-center justify-center text-3xl font-extrabold text-[var(--color-primary)]/60",
                  "{first}"
              }
          }
          div { class: "p-5 flex flex-col flex-1",
              div { class: "flex items-center gap-2 mb-2 text-xs text-slate-400",
                  if let Some(level) = course.level.clone() {
                      span { class: "px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 font-medium", "{level}" }
                  }
                  span { "{lessons}" }
              }
              h3 { class: "font-bold text-slate-900 dark:text-white group-hover:text-[var(--color-primary)] transition-colors", "{course.title}" }
              p { class: "mt-1 text-sm text-slate-600 dark:text-slate-400 line-clamp-2 flex-1", "{course.description}" }
          }
      }
  }
}

// ── 社区动态 ──────────────────────────────────────────────

/// 社区动态：左「最新博客」（SSR 预取）+ 右「论坛热帖」（DB，use_resource）。
#[component]
pub fn CommunityFeed() -> Element {
  let lang = use_i18n();
  rsx! {
      section { class: "py-20 bg-white dark:bg-slate-950",
          Container {
              div { class: "mb-10",
                  h2 { class: "text-2xl sm:text-3xl font-extrabold tracking-tight text-slate-900 dark:text-white", "{t(lang(), \"home.community.title\")}" }
                  p { class: "mt-2 text-slate-600 dark:text-slate-400", "{t(lang(), \"home.community.subtitle\")}" }
              }
              div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 p-5",
                      div { class: "flex items-center justify-between mb-3",
                          h3 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200", "{t(lang(), \"home.community.blog\")}" }
                          Link { to: Route::BlogIndex {}, class: "text-xs text-[var(--color-primary)] hover:underline", "{t(lang(), \"home.community.blog_all\")}" }
                      }
                      SuspenseBoundary { fallback: |_| loading_spinner(), BlogColumn {} }
                  }
                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 p-5",
                      div { class: "flex items-center justify-between mb-3",
                          h3 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200", "{t(lang(), \"home.community.forum\")}" }
                          Link { to: Route::TopicsIndex {}, class: "text-xs text-[var(--color-primary)] hover:underline", "{t(lang(), \"home.community.forum_all\")}" }
                      }
                      ForumColumn {}
                  }
              }
          }
      }
  }
}

#[component]
fn BlogColumn() -> Element {
  let lang = use_i18n();
  let res = use_server_future(|| async move { list_blog_posts().await.unwrap_or_default() })?;
  let posts: Vec<BlogPostSummary> = res().unwrap_or_default().into_iter().take(4).collect();
  rsx! {
      if posts.is_empty() {
          p { class: "text-sm text-slate-400 py-6 text-center", "{t(lang(), \"home.community.blog_empty\")}" }
      } else {
          div { class: "divide-y divide-slate-200 dark:divide-slate-800",
              for p in posts.into_iter() {
                  Link {
                      key: "{p.slug}",
                      to: Route::Blog { id: p.slug.clone() },
                      class: "group block py-3",
                      div { class: "flex items-baseline justify-between gap-3",
                          span { class: "font-medium text-slate-800 dark:text-slate-100 group-hover:text-[var(--color-primary)] transition-colors line-clamp-1", "{p.title}" }
                          span { class: "shrink-0 text-xs text-slate-400 tabular-nums", "{p.date}" }
                      }
                      p { class: "mt-0.5 text-sm text-slate-500 dark:text-slate-400 line-clamp-1", "{p.description}" }
                  }
              }
          }
      }
  }
}

#[component]
fn ForumColumn() -> Element {
  let lang = use_i18n();
  // 论坛走 DB，用 use_resource（非 SEO、可能未连库）；失败/空时优雅降级。
  let res = use_resource(|| async move { list_topics(None, Some(0)).await.unwrap_or_default() });
  let topics: Vec<TopicSummary> =
    res.read().clone().unwrap_or_default().into_iter().take(5).collect();
  let loaded = res.read().is_some();
  let reply_unit = lang().pick("回复", "replies");
  rsx! {
      if !loaded {
          {loading_spinner()}
      } else if topics.is_empty() {
          p { class: "text-sm text-slate-400 py-6 text-center", "{t(lang(), \"home.community.forum_empty\")}" }
      } else {
          div { class: "divide-y divide-slate-200 dark:divide-slate-800",
              for tpc in topics.into_iter() {
                  Link {
                      key: "{tpc.id}",
                      to: Route::TopicDetail { id: tpc.id },
                      class: "group block py-3",
                      div { class: "flex items-baseline justify-between gap-3",
                          span { class: "font-medium text-slate-800 dark:text-slate-100 group-hover:text-[var(--color-primary)] transition-colors line-clamp-1", "{tpc.title}" }
                          span { class: "shrink-0 text-xs text-slate-400 tabular-nums", "{tpc.reply_count} {reply_unit}" }
                      }
                      div { class: "mt-0.5 flex items-center gap-2 text-xs text-slate-400",
                          span { class: "px-1.5 py-0.5 rounded bg-slate-100 dark:bg-slate-800", "{tpc.tag}" }
                          span { "{tpc.author}" }
                      }
                  }
              }
          }
      }
  }
}

// ── 生态落地页 ────────────────────────────────────────────

/// 生态落地页 `/ecosystem/:id`：生态简介 + 领域入口 + 该生态精选案例。
/// 完成「生态为主」IA 的闭环——导航/pillars 的生态标题有真实落地页。
#[component]
pub fn EcosystemPage(id: String) -> Element {
  let lang = use_i18n();
  let Some(eco) = ecosystem_by_id(&id) else {
    return rsx! {
        section { class: "py-24",
            Container {
                p { class: "text-center text-slate-400", "{t(lang(), \"eco.not_found\")}" }
            }
        }
    };
  };

  rsx! {
      // 头部
      section { class: "py-16 sm:py-20 bg-gradient-to-b from-slate-950 to-slate-900",
          div { class: "max-w-6xl mx-auto px-4 sm:px-6 lg:px-8",
              h1 { class: "text-3xl md:text-4xl font-extrabold tracking-tight text-flow-light", "{t(lang(), eco.label_key)}" }
              p { class: "mt-4 text-lg text-slate-300 max-w-2xl", "{t(lang(), eco.blurb_key)}" }
          }
      }

      // 应用领域
      section { class: "py-16 bg-white dark:bg-slate-950",
          Container {
              h2 { class: "text-xl font-bold text-slate-900 dark:text-white mb-6", "{t(lang(), \"mega.col.domains\")}" }
              div { class: "grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-4",
                  for d in eco.domains.iter() {
                      Link {
                          key: "{d.id}",
                          to: d.route.clone(),
                          class: "group flex items-center justify-between rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 px-4 py-3 hover:border-[var(--color-primary)] transition-colors",
                          span { class: "font-medium text-slate-800 dark:text-slate-100 group-hover:text-[var(--color-primary)] transition-colors", "{t(lang(), d.label_key)}" }
                          svg { class: "w-4 h-4 text-slate-300 dark:text-slate-600 group-hover:text-[var(--color-primary)] group-hover:translate-x-0.5 transition-all", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "2",
                              path { stroke_linecap: "round", stroke_linejoin: "round", d: "M9 5l7 7-7 7" }
                          }
                      }
                  }
              }
          }
      }

      // 该生态精选案例
      section { class: "py-16 bg-slate-50/60 dark:bg-slate-900/40 border-t border-slate-200/70 dark:border-slate-800",
          Container {
              SectionHeader {
                  title: t(lang(), "home.featured.title"),
                  subtitle: t(lang(), "home.featured.subtitle"),
                  all_label: t(lang(), "home.featured.all"),
                  to: Route::Cases {},
              }
              SuspenseBoundary {
                  fallback: |_| loading_spinner(),
                  EcosystemCasesInner { eco_id: id.clone() }
              }
          }
      }
  }
}

#[component]
fn EcosystemCasesInner(eco_id: String) -> Element {
  let lang = use_i18n();
  let res =
    use_server_future(|| async move { list_cases(None, None, None).await.unwrap_or_default() })?;
  let picks: Vec<CaseSummary> = res()
    .unwrap_or_default()
    .into_iter()
    .filter(|c| ecosystem_of_case_category(&c.category) == Some(eco_id.as_str()))
    .take(9)
    .collect();

  rsx! {
      if picks.is_empty() {
          p { class: "text-center text-slate-400 py-10", "{t(lang(), \"home.featured.empty\")}" }
      } else {
          div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6",
              for c in picks.into_iter() {
                  CaseCard { key: "{c.slug}", case: c }
              }
          }
      }
  }
}
