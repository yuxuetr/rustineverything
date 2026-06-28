use dioxus::prelude::*;
use dioxus::router::{Link, Routable};

use crate::components::comment::CommentBox;
use crate::components::hero::Hero;
use crate::components::home_sections::{CourseShowcase, EcosystemPillars, FeaturedCases};
use crate::components::module_gate::ModuleGate;
use crate::components::nav::Navbar;
use crate::components::view::{Container, SectionTitle};
use crate::i18n::{t, use_i18n};
use crate::server::enabled_module_ids;
use crate::server::get_seo_base_url;
use crate::server::{list_public_plugins, PublicPluginInfo};
use module_admin::admin::{
  AdminCommentsPage, AdminDashboardPage, AdminModerationPage, AdminModerationSettingsPage,
  AdminPluginsPage, AdminTopicsPage, AdminUsersPage,
};
use module_ai::ai::{AiArticlePage, AiIndexPage};
use module_blog::server::{get_blog_content, list_blog_posts};
use module_cases::cases::{CaseDetailPage, CasesIndexPage};
use module_cli::cli::{CliArticlePage, CliIndexPage};
use module_course::course::{
  AnnotationLayer, CourseDetailPage, CoursesIndexPage, LessonPage, MyAnnotationsPage,
};
use module_docs::docs::{DocPage as DocPageView, Docs as DocsView};
use module_embedded::embedded::{EmbeddedArticlePage, EmbeddedIndexPage};
use module_forum::forum::{
  DiscussionPanel, MyTopicsPage, NewTopicPage, TopicDetailPage, TopicsByTagPage, TopicsIndexPage,
};
use module_podcast::podcast::PodcastPage;
use module_wasm::wasm::{WasmArticlePage, WasmIndexPage};
use module_web3::web3::{Web3ArticlePage, Web3IndexPage};
use widgets::{inject_seo, parse_mdx, Markdown};

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

        // Phase 6 内容板块：每个一条落地页 + 一条文章详情页
        #[route("/embedded")]
        Embedded {},
        #[route("/embedded/:slug")]
        EmbeddedArticle { slug: String },

        #[route("/ai")]
        Ai {},
        #[route("/ai/:slug")]
        AiArticle { slug: String },

        #[route("/web3")]
        Web3 {},
        #[route("/web3/:slug")]
        Web3Article { slug: String },

        #[route("/wasm")]
        Wasm {},
        #[route("/wasm/:slug")]
        WasmArticle { slug: String },

        #[route("/cli")]
        Cli {},
        #[route("/cli/:slug")]
        CliArticle { slug: String },

        // Phase 5.5：公开插件浏览页
        #[route("/plugins")]
        PluginsPublic {},

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
        #[route("/admin/moderation")]
        AdminModeration {},
        #[route("/admin/moderation/settings")]
        AdminModerationSettings {},
}

/// Home page
#[component]
pub fn Home() -> Element {
  let lang = use_i18n();

  // 与 navbar 共用「站点模块开关」：默认全开避免首屏闪烁，server fn 返回后收敛。
  fn all_default_ids() -> Vec<String> {
    app_core::engines::module::default_module_specs().into_iter().map(|s| s.id).collect()
  }
  let enabled_res =
    use_resource(
      || async move { enabled_module_ids().await.unwrap_or_else(|_| all_default_ids()) },
    );
  let enabled: Vec<String> = enabled_res.read().as_ref().cloned().unwrap_or_else(all_default_ids);

  // 首页模块网格的展示顺序：内容主线（文档/博客/课程/案例）在前，
  // 垂直领域（AI/WASM/Web3/嵌入式/CLI）居中，社区/收听（播客/论坛）在后。
  // 每张卡片 gate 于 `enabled`，与导航栏保持一致——新增模块只动这一处 + i18n。
  let modules: Vec<(&str, Route)> = vec![
    ("docs", Route::Docs {}),
    ("blog", Route::BlogIndex {}),
    ("course", Route::Courses {}),
    ("cases", Route::Cases {}),
    ("ai", Route::Ai {}),
    ("wasm", Route::Wasm {}),
    ("web3", Route::Web3 {}),
    ("embedded", Route::Embedded {}),
    ("cli", Route::Cli {}),
    ("podcast", Route::Podcast {}),
    ("forum", Route::TopicsIndex {}),
  ];

  rsx! {
      Hero {}

      // 两大生态支柱：直接体现「Rust 生态 + AI 生态」定位
      EcosystemPillars { enabled: enabled.clone() }

      // 精选案例（旗舰）+ 课程（变现）
      FeaturedCases {}
      CourseShowcase {}

      // 按领域浏览：原模块网格下移为次级导航
      section { class: "py-20 bg-white dark:bg-slate-950",
          Container {
              SectionTitle {
                  title: t(lang(), "home.browse.title"),
                  subtitle: Some(t(lang(), "home.browse.subtitle"))
              }

              div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6 mt-12",
                  for (id , route) in modules.into_iter() {
                      if enabled.iter().any(|e| e == id) {
                          ModuleCard {
                              key: "{id}",
                              to: route,
                              icon: module_icon(id),
                              title: t(lang(), &format!("home.mod.{id}.title")),
                              desc: t(lang(), &format!("home.mod.{id}.desc")),
                          }
                      }
                  }
              }
          }
      }
  }
}

/// 首页模块卡片：整卡可点击，跳转到对应板块入口路由。
#[component]
fn ModuleCard(to: Route, icon: Element, title: String, desc: String) -> Element {
  rsx! {
      Link {
          to,
          class: "group flex flex-col p-6 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 hover:shadow-lg hover:border-[var(--color-primary)]/40 transition-all",
          div { class: "flex items-center justify-between mb-4",
              div { class: "w-11 h-11 rounded-lg bg-[var(--color-primary)]/10 flex items-center justify-center text-[var(--color-primary)]",
                  svg { class: "w-6 h-6", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "1.5",
                      {icon}
                  }
              }
              svg {
                  class: "w-5 h-5 text-slate-300 dark:text-slate-600 transition-all group-hover:translate-x-1 group-hover:text-[var(--color-primary)]",
                  fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "2",
                  path { stroke_linecap: "round", stroke_linejoin: "round", d: "M9 5l7 7-7 7" }
              }
          }
          h3 { class: "text-lg font-bold text-slate-900 dark:text-white mb-1.5", "{title}" }
          p { class: "text-sm text-slate-600 dark:text-slate-400 leading-relaxed", "{desc}" }
      }
  }
}

/// 按模块 id 返回卡片图标的 `<path>`（Heroicons outline，统一 24×24 viewBox）。
fn module_icon(id: &str) -> Element {
  let d = match id {
    "docs" => "M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z",
    "blog" => "M16.862 4.487l1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10",
    "course" => "M4.26 10.147a60.438 60.438 0 0 0-.491 6.347A48.62 48.62 0 0 1 12 20.904a48.62 48.62 0 0 1 8.232-4.41 60.46 60.46 0 0 0-.491-6.347m-15.482 0a50.636 50.636 0 0 0-2.658-.813A59.906 59.906 0 0 1 12 3.493a59.903 59.903 0 0 1 10.399 5.84c-.896.248-1.783.52-2.658.814m-15.482 0A50.717 50.717 0 0 1 12 13.489a50.702 50.702 0 0 1 7.74-3.342M6.75 15a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm0 0v-3.675A55.378 55.378 0 0 1 12 8.443m-7.007 11.55A5.981 5.981 0 0 0 6.75 15.75v-1.5",
    "cases" => "M3.75 6A2.25 2.25 0 0 1 6 3.75h2.25A2.25 2.25 0 0 1 10.5 6v2.25a2.25 2.25 0 0 1-2.25 2.25H6a2.25 2.25 0 0 1-2.25-2.25V6ZM3.75 15.75A2.25 2.25 0 0 1 6 13.5h2.25a2.25 2.25 0 0 1 2.25 2.25V18a2.25 2.25 0 0 1-2.25 2.25H6A2.25 2.25 0 0 1 3.75 18v-2.25ZM13.5 6a2.25 2.25 0 0 1 2.25-2.25H18A2.25 2.25 0 0 1 20.25 6v2.25A2.25 2.25 0 0 1 18 10.5h-2.25a2.25 2.25 0 0 1-2.25-2.25V6ZM13.5 15.75a2.25 2.25 0 0 1 2.25-2.25H18a2.25 2.25 0 0 1 2.25 2.25V18A2.25 2.25 0 0 1 18 20.25h-2.25A2.25 2.25 0 0 1 13.5 18v-2.25Z",
    "ai" => "M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 0 0-2.456 2.456Z",
    "wasm" => "M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75Z",
    "web3" => "M21 7.5l-2.25-1.313M21 7.5v2.25m0-2.25l-2.25 1.313M3 7.5l2.25-1.313M3 7.5l2.25 1.313M3 7.5v2.25m9 3l2.25-1.313M12 12.75l-2.25-1.313M12 12.75V15m0 6.75l2.25-1.313M12 21.75V19.5m0 2.25l-2.25-1.313m0-16.875L12 2.25l2.25 1.313M21 14.25v2.25l-2.25 1.313m-13.5 0L3 16.5v-2.25",
    "embedded" => "M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 0 0 2.25-2.25V6.75a2.25 2.25 0 0 0-2.25-2.25H6.75A2.25 2.25 0 0 0 4.5 6.75v10.5a2.25 2.25 0 0 0 2.25 2.25Zm.75-12h9v9h-9v-9Z",
    "cli" => "M6.75 7.5l3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z",
    "podcast" => "M12 18.75a6 6 0 0 0 6-6v-1.5m-6 7.5a6 6 0 0 1-6-6v-1.5m6 7.5v3.75m-3.75 0h7.5M12 15.75a3 3 0 0 1-3-3V4.5a3 3 0 1 1 6 0v8.25a3 3 0 0 1-3 3Z",
    "forum" => "M20.25 8.511c.884.284 1.5 1.128 1.5 2.097v4.286c0 1.136-.847 2.1-1.98 2.193-.34.027-.68.052-1.02.072v3.091l-3-3c-1.354 0-2.694-.055-4.02-.163a2.115 2.115 0 0 1-.825-.242m9.345-8.334a2.126 2.126 0 0 0-.476-.095 48.64 48.64 0 0 0-8.048 0c-1.131.094-1.976 1.057-1.976 2.192v4.286c0 .837.46 1.58 1.155 1.951m9.345-8.334V6.637c0-1.621-1.152-3.026-2.76-3.235A48.455 48.455 0 0 0 11.25 3c-2.115 0-4.198.137-6.24.402-1.608.209-2.76 1.614-2.76 3.235v6.226c0 1.621 1.152 3.026 2.76 3.235.577.075 1.157.14 1.74.194V21l4.155-4.155",
    _ => "M9 5l7 7-7 7",
  };
  rsx! {
      path { stroke_linecap: "round", stroke_linejoin: "round", d }
  }
}

/// /docs 文档首页：转交给 docs 模块的 Docs 组件渲染
#[component]
pub fn Docs() -> Element {
  rsx! { ModuleGate { id: "docs".to_string(), DocsView {} } }
}

/// 文档详情页：转交给 docs 模块的 DocPage 组件渲染。
/// 标注层（course）+ 讨论面板（forum）是跨模块组合，在组合根 app 这里装配后
/// 通过 DocPage 的 `footer` 插槽注入，docs 模块本身不依赖 course / forum。
#[component]
pub fn DocPage(path: Vec<String>) -> Element {
  let doc_path = path.join("/");
  rsx! {
      ModuleGate { id: "docs".to_string(),
          DocPageView {
              path: path.clone(),
              footer: rsx! {
                  AnnotationLayer {
                      resource_kind: "doc".to_string(),
                      resource_path: doc_path.clone(),
                  }
                  DiscussionPanel {
                      resource_kind: "doc".to_string(),
                      resource_path: doc_path.clone(),
                  }
              },
          }
      }
  }
}

#[component]
pub fn BlogIndex() -> Element {
  rsx! { ModuleGate { id: "blog".to_string(), BlogIndexInner {} } }
}

#[component]
fn BlogIndexInner() -> Element {
  let lang = use_i18n();

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8",
              // 页面标题（静态，始终展示）
              div { class: "text-center mb-10",
                  h2 { class: "text-3xl font-bold tracking-tight text-slate-900 dark:text-white sm:text-4xl", "{t(lang(), \"blog.title\")}" }
                  p { class: "mt-4 text-lg text-slate-500 dark:text-slate-400", "{t(lang(), \"blog.subtitle\")}" }
              }

              // 重构 B2：文章列表改由 BlogList 用 use_server_future 服务端预取（随 SSR HTML 下发）。
              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                      }
                  },
                  BlogList {}
              }
          }
      }
  }
}

/// 博客文章列表（重构 B2）：`list_blog_posts` 经 `use_server_future` 服务端预取，
/// 标签筛选 / 分页仍是客户端 signal 交互。置于 SuspenseBoundary 内。
#[component]
fn BlogList() -> Element {
  let lang = use_i18n();

  let posts_res = use_server_future(|| async move { list_blog_posts().await.unwrap_or_default() })?;
  let posts = posts_res().unwrap_or_default();

  let mut active_tag = use_signal::<Option<String>>(|| None);
  let mut current_page = use_signal(|| 0usize);
  const PAGE_SIZE: usize = 10;

  // 汇总全部标签
  let all_tags: Vec<String> = {
    let mut seen = std::collections::BTreeSet::new();
    for post in posts.iter() {
      for t in post.tags.iter() {
        seen.insert(t.clone());
      }
    }
    seen.into_iter().collect()
  };

  // 按标签过滤
  let filtered: Vec<_> = match active_tag() {
    Some(ref tag) => posts.iter().filter(|p| p.tags.contains(tag)).cloned().collect(),
    None => posts.clone(),
  };

  // 分页
  let total_pages = filtered.len().div_ceil(PAGE_SIZE).max(1);
  let safe_page = current_page().min(total_pages - 1);
  let paged: Vec<_> =
    filtered.iter().skip(safe_page * PAGE_SIZE).take(PAGE_SIZE).cloned().collect();

  rsx! {
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
  }
}

#[component]
pub fn Blog(id: String) -> Element {
  rsx! { ModuleGate { id: "blog".to_string(), BlogInner { id: id.clone() } } }
}

#[component]
fn BlogInner(id: String) -> Element {
  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          Container {
              div { class: "max-w-4xl mx-auto",
                  div { class: "text-slate-700 dark:text-slate-200 mb-12",
                      // 重构 B1：正文改由 BlogArticle 用 use_server_future 服务端预取（随 SSR HTML 下发）。
                      // SuspenseBoundary 捕获 BlogArticle 内 `?` 的挂起，未就绪时渲染 spinner。
                      SuspenseBoundary {
                          fallback: |_| rsx! {
                              div { class: "flex items-center justify-center py-20",
                                  div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                              }
                          },
                          BlogArticle { id: id.clone() }
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

/// 博客正文（重构 B1 参照实现）：用 `use_server_future` 在服务端预取并随 SSR HTML
/// 下发，客户端 hydration 直接拿到内容，避免首屏 spinner + 二次抓取。
///
/// 必须置于 SuspenseBoundary 内：`?` 会在数据未就绪时挂起，由边界渲染 fallback。
#[component]
fn BlogArticle(id: String) -> Element {
  let anno_path = id.clone();
  let blog_path = format!("/blog/{}", id);
  // 正文随路由参数 id 变化重取：use_reactive 让闭包订阅 id（避免 SPA 导航后内容 stale）。
  let blog_content =
    use_server_future(use_reactive!(|id| async move { get_blog_content(id).await }))?;
  // BASE_URL 用于 canonical URL，常量级，无需响应式依赖。
  let base_url_res =
    use_server_future(|| async move { get_seo_base_url().await.unwrap_or_default() })?;
  let base_url: String = base_url_res().unwrap_or_default();

  match blog_content() {
    Some(Ok(content)) => rsx! {
        // SEO 注入：inject_seo 从 frontmatter 取 metadata。
        {
            let (meta, _body) = parse_mdx(&content);
            rsx! { {inject_seo(&meta, &blog_path, &base_url)} }
        }
        // `blog_id` 在 widgets::Markdown 内仅用于拼图片相对路径 `/posts/<id>/...`，
        // **必须传纯 slug**（如 "welcome"）。
        Markdown { content: content.clone(), blog_id: id.clone() }
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

#[component]
pub fn Courses() -> Element {
  rsx! { ModuleGate { id: "course".to_string(), CoursesIndexPage {} } }
}

#[component]
pub fn CourseDetail(slug: String) -> Element {
  rsx! { ModuleGate { id: "course".to_string(), CourseDetailPage { slug: slug } } }
}

#[component]
pub fn Lesson(slug: String, chapter: String, lesson: String) -> Element {
  let lesson_path = format!("{}/{}/{}", slug, chapter, lesson);
  rsx! {
      ModuleGate { id: "course".to_string(),
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
}

#[component]
pub fn Cases() -> Element {
  rsx! { ModuleGate { id: "cases".to_string(), CasesIndexPage {} } }
}

#[component]
pub fn CaseDetail(slug: String) -> Element {
  rsx! { ModuleGate { id: "cases".to_string(), CaseDetailPage { slug } } }
}

// ── Phase 6 内容板块路由（委派给各模块的页面组件，ModuleGate 控制开关）──

#[component]
pub fn Embedded() -> Element {
  rsx! { ModuleGate { id: "embedded".to_string(), EmbeddedIndexPage {} } }
}

#[component]
pub fn EmbeddedArticle(slug: String) -> Element {
  rsx! { ModuleGate { id: "embedded".to_string(), EmbeddedArticlePage { slug } } }
}

#[component]
pub fn Ai() -> Element {
  rsx! { ModuleGate { id: "ai".to_string(), AiIndexPage {} } }
}

#[component]
pub fn AiArticle(slug: String) -> Element {
  rsx! { ModuleGate { id: "ai".to_string(), AiArticlePage { slug } } }
}

#[component]
pub fn Web3() -> Element {
  rsx! { ModuleGate { id: "web3".to_string(), Web3IndexPage {} } }
}

#[component]
pub fn Web3Article(slug: String) -> Element {
  rsx! { ModuleGate { id: "web3".to_string(), Web3ArticlePage { slug } } }
}

#[component]
pub fn Wasm() -> Element {
  rsx! { ModuleGate { id: "wasm".to_string(), WasmIndexPage {} } }
}

#[component]
pub fn WasmArticle(slug: String) -> Element {
  rsx! { ModuleGate { id: "wasm".to_string(), WasmArticlePage { slug } } }
}

#[component]
pub fn Cli() -> Element {
  rsx! { ModuleGate { id: "cli".to_string(), CliIndexPage {} } }
}

#[component]
pub fn CliArticle(slug: String) -> Element {
  rsx! { ModuleGate { id: "cli".to_string(), CliArticlePage { slug } } }
}

/// Phase 5.5：公开插件浏览页 `/plugins`。列出已安装且声明了 manifest 的 WASM 插件。
#[component]
pub fn PluginsPublic() -> Element {
  let res = use_resource(|| async move { list_public_plugins().await.unwrap_or_default() });
  let plugins: Vec<PublicPluginInfo> = res.read().as_ref().cloned().unwrap_or_default();

  rsx! {
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "max-w-4xl mx-auto px-4 sm:px-6",
              div { class: "mb-10",
                  h1 { class: "text-3xl sm:text-4xl font-extrabold tracking-tight text-slate-900 dark:text-white", "插件" }
                  p { class: "mt-3 text-lg text-slate-500 dark:text-slate-400",
                      "本站基于 WASM 插件运行时构建：主题 / 多语言 / 登录 / 审核 均由沙箱化插件提供。以下是当前已安装的插件。"
                  }
              }
              match res.read().as_ref() {
                  None => rsx! {
                      div { class: "flex items-center justify-center py-20",
                          div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                      }
                  },
                  Some(_) if plugins.is_empty() => rsx! {
                      div { class: "py-16 text-center text-slate-400", "没有发现已声明 manifest 的插件" }
                  },
                  Some(_) => rsx! {
                      div { class: "grid grid-cols-1 sm:grid-cols-2 gap-4",
                          for p in plugins.iter() {
                              PublicPluginCard { key: "{p.filename}", plugin: p.clone() }
                          }
                      }
                  },
              }
          }
      }
  }
}

fn capability_label(cap: &str) -> &'static str {
  match cap {
    "auth-provider" => "登录",
    "theme" => "主题",
    "i18n" => "多语言",
    "moderation-provider" => "审核",
    "notification" => "通知",
    "layout" => "布局",
    "mdx-component" => "MDX 组件",
    _ => "其它",
  }
}

#[component]
fn PublicPluginCard(plugin: PublicPluginInfo) -> Element {
  rsx! {
      div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/40 p-5",
          div { class: "flex items-center justify-between gap-2 mb-1",
              h3 { class: "text-base font-bold text-slate-900 dark:text-white", "{plugin.name}" }
              span { class: "text-xs font-mono text-slate-400", "v{plugin.version}" }
          }
          div { class: "text-xs font-mono text-slate-500 dark:text-slate-400 mb-2", "{plugin.id}" }
          if !plugin.description.is_empty() {
              p { class: "text-sm text-slate-600 dark:text-slate-400 mb-3", "{plugin.description}" }
          }
          div { class: "flex flex-wrap items-center gap-1.5",
              for cap in plugin.capabilities.iter() {
                  span { class: "text-xs px-2 py-0.5 rounded bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300",
                      "{capability_label(cap)}"
                  }
              }
              if !plugin.abi_compatible {
                  span { class: "text-xs px-2 py-0.5 rounded bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300",
                      "ABI 不兼容"
                  }
              }
          }
      }
  }
}

#[component]
pub fn TopicsIndex() -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), TopicsIndexPage {} } }
}

#[component]
pub fn TopicsNew() -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), NewTopicPage {} } }
}

#[component]
pub fn TopicsByTag(tag: String) -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), TopicsByTagPage { tag } } }
}

#[component]
pub fn TopicDetail(id: i32) -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), TopicDetailPage { id } } }
}

#[component]
pub fn MyTopics() -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), MyTopicsPage {} } }
}

#[component]
pub fn Podcast() -> Element {
  rsx! {
      ModuleGate { id: "podcast".to_string(), PodcastPage {} }
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

#[component]
pub fn AdminModeration() -> Element {
  rsx! { AdminModerationPage {} }
}

#[component]
pub fn AdminModerationSettings() -> Element {
  rsx! { AdminModerationSettingsPage {} }
}
