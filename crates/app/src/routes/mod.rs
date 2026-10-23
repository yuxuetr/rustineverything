use dioxus::prelude::*;
use dioxus::router::{Link, Routable};
use dioxus_shadcn::{
  badge_class, BadgeVariant, Button, ButtonSize, ButtonVariant, Spinner, SpinnerSize,
};

#[cfg(feature = "accounts")]
use crate::components::comment::CommentBox;
use crate::components::hero::Hero;
#[cfg(feature = "course")]
use crate::components::home_sections::CourseShowcase;
use crate::components::home_sections::{
  CommunityFeed, EcosystemPage, EcosystemPillars, FeaturedCases,
};
use crate::components::module_gate::{use_enabled_modules, ModuleGate};
use crate::components::nav::Navbar;
use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use crate::server::get_about_page;
use module_ai::{AiArticlePage, AiIndexPage};
use module_blog::server::{get_blog_content, list_blog_posts};
use module_cases::cases::{CaseDetailPage, CasesIndexPage};
use module_cli::{CliArticlePage, CliIndexPage};
#[cfg(feature = "course")]
use module_course::course::AnnotationLayer;
use module_docs::docs::{DocPage as DocPageView, Docs as DocsView};
use module_embedded::{EmbeddedArticlePage, EmbeddedIndexPage};
#[cfg(feature = "accounts")]
use module_forum::forum::DiscussionPanel;
use module_podcast::podcast::PodcastPage;
use module_wasm::{WasmArticlePage, WasmIndexPage};
use module_web3::{Web3ArticlePage, Web3IndexPage};
use widgets::{get_seo_base_url, inject_seo, parse_mdx, Markdown};

#[cfg(feature = "accounts")]
mod accounts;
#[cfg(feature = "accounts")]
pub use accounts::*;
#[cfg(feature = "course")]
mod course;
#[cfg(feature = "course")]
pub use course::*;

/// 某一层没编译进来时（docs/FEATURE_TIERS.md DL1），为它的路由生成同名页面组件，
/// 渲染 404。`Route` 因此在各层都相同，导航链接照常编译，是否显示由启用列表决定。
#[allow(unused_macros)]
macro_rules! absent_pages {
  ($($name:ident { $($field:ident : $ty:ty),* }),* $(,)?) => {
    $(
      #[component]
      pub fn $name($($field: $ty),*) -> Element {
        $(let _ = $field;)*
        let lang = use_i18n();
        rsx! { widgets::NotFound { message: t(lang(), "common.not_found") } }
      }
    )*
  };
}

#[cfg(not(feature = "course"))]
absent_pages! {
  Courses {},
  CourseDetail { slug: String },
  Lesson { slug: String, chapter: String, lesson: String },
  MyAnnotations {},
  MyOrders {},
  AdminEntitlements {},
}

#[cfg(not(feature = "accounts"))]
absent_pages! {
  TopicsIndex {},
  TopicsNew {},
  TopicsByTag { tag: String },
  TopicDetail { id: i32 },
  MyTopics {},
  AdminDashboard {},
  AdminUsers {},
  AdminComments {},
  AdminTopics {},
  AdminModeration {},
  AdminModerationSettings {},
}

/// 资源讨论面板（论坛，accounts 层）；未编译时为空。
fn discussion(resource_kind: &str, resource_path: String) -> Element {
  #[cfg(feature = "accounts")]
  {
    rsx! { DiscussionPanel { resource_kind: resource_kind.to_string(), resource_path } }
  }
  #[cfg(not(feature = "accounts"))]
  {
    let _ = (resource_kind, resource_path);
    rsx! {}
  }
}

/// 博客评论区（accounts 层）；未编译时为空（连同上方分隔线）。
fn comments(blog_id: String) -> Element {
  #[cfg(feature = "accounts")]
  {
    rsx! {
        div { class: "border-t border-slate-200 dark:border-slate-800 pt-8 mt-12",
            CommentBox { blog_id }
        }
    }
  }
  #[cfg(not(feature = "accounts"))]
  {
    let _ = blog_id;
    rsx! {}
  }
}

/// 标注层（course 层，FEATURE_TIERS DL3）；未编译课程时为空。
fn annotation_layer(resource_kind: &str, resource_path: String) -> Element {
  #[cfg(feature = "course")]
  {
    rsx! { AnnotationLayer { resource_kind: resource_kind.to_string(), resource_path } }
  }
  #[cfg(not(feature = "course"))]
  {
    let _ = (resource_kind, resource_path);
    rsx! {}
  }
}

/// 打赏卡片（payments 层，FEATURE_TIERS.md §6）；未编译支付时为空。
fn tip_jar(source: String) -> Element {
  #[cfg(feature = "payments")]
  {
    rsx! { TipCard { source } }
  }
  #[cfg(not(feature = "payments"))]
  {
    let _ = source;
    rsx! {}
  }
}

/// 给打赏卡片接上 app 层的登录状态与登录弹窗。
#[cfg(feature = "payments")]
#[component]
fn TipCard(source: String) -> Element {
  let session_user = crate::use_session_user();
  let mut show_auth_modal = crate::use_auth_modal();
  rsx! {
      module_course::pay_ui::TipJar {
          source,
          signed_in: session_user.read().is_some(),
          on_sign_in: move |_| show_auth_modal.set(true),
      }
  }
}

/// 首页课程区（course 层）；未编译课程时为空。
fn course_showcase() -> Element {
  #[cfg(feature = "course")]
  {
    rsx! { CourseShowcase {} }
  }
  #[cfg(not(feature = "course"))]
  {
    rsx! {}
  }
}

/// Application routes
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
        #[route("/")]
        Home {},

        // 生态落地页（双生态 IA 的落地）：/ecosystem/rust | /ecosystem/ai
        #[route("/ecosystem/:id")]
        EcosystemPage { id: String },

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

        #[route("/about")]
        About {},

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
        #[route("/me/orders")]
        MyOrders {},

        // Admin 后台（页面内部判断 role，非 admin 渲染 403 占位）
        #[route("/admin")]
        AdminDashboard {},
        #[route("/admin/users")]
        AdminUsers {},
        #[route("/admin/comments")]
        AdminComments {},
        #[route("/admin/topics")]
        AdminTopics {},
        #[route("/admin/entitlements")]
        AdminEntitlements {},
        #[route("/admin/moderation")]
        AdminModeration {},
        #[route("/admin/moderation/settings")]
        AdminModerationSettings {},
}

/// Home page
#[component]
pub fn Home() -> Element {
  let lang = use_i18n();
  // 与 navbar 共用「站点模块开关」。
  let enabled = use_enabled_modules();
  let on = |id: &str| enabled.iter().any(|e| e == id);
  let (on_blog, on_forum) = (on("blog"), on("forum"));

  rsx! {
      widgets::PageMeta {
          title: t(lang(), "hero.title"),
          description: t(lang(), "hero.subtitle"),
          path: "/".to_string(),
          schema: widgets::website_schema(),
      }
      Hero { enabled: enabled.clone() }

      // 两大生态支柱：直接体现「Rust 生态 + AI 生态」定位
      EcosystemPillars { enabled: enabled.clone() }

      // 精选案例（旗舰）+ 课程（变现）
      if on("cases") {
          FeaturedCases {}
      }
      if on("course") {
          {course_showcase()}
      }

      // 社区动态：最新博客 + 论坛热帖
      if on_blog || on_forum {
          CommunityFeed { blog: on_blog, forum: on_forum }
      }
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
                  {annotation_layer("doc", doc_path.clone())}
                  {discussion("doc", doc_path.clone())}
              },
          }
      }
  }
}

/// 博客标签筛选 chip：选中为主色实心，否则为描边（与 podcast 一致）。
fn tag_chip_class(active: bool) -> String {
  if active {
    badge_class(BadgeVariant::Default, "gap-1 rounded-full px-3 py-1 font-medium")
  } else {
    badge_class(
      BadgeVariant::Outline,
      "gap-1 rounded-full bg-background px-3 py-1 font-medium text-muted-foreground hover:border-primary hover:text-primary",
    )
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
      widgets::PageMeta {
          title: t(lang(), "blog.title"),
          description: t(lang(), "blog.subtitle"),
          path: "/blog".to_string(),
      }
      section { class: "py-12 bg-white dark:bg-slate-950",
          div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8",
              // 页面标题（静态，始终展示）
              div { class: "text-center mb-10",
                  h1 { class: "text-3xl font-bold tracking-tight text-slate-900 dark:text-white sm:text-4xl", "{t(lang(), \"blog.title\")}" }
                  p { class: "mt-4 text-lg text-slate-500 dark:text-slate-400", "{t(lang(), \"blog.subtitle\")}" }
              }

              // 重构 B2：文章列表改由 BlogList 用 use_server_future 服务端预取（随 SSR HTML 下发）。
              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
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
                                                      r#type: "button",
                                                      "aria-pressed": is_all.to_string(),
                                                      onclick: move |_| { active_tag.set(None); current_page.set(0); },
                                                      class: tag_chip_class(is_all),
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
                                                          r#type: "button",
                                                          "aria-pressed": is_active.to_string(),
                                                          onclick: move |_| { active_tag.set(Some(t.clone())); current_page.set(0); },
                                                          class: tag_chip_class(is_active),
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
                                              h3 { class: "text-sm font-semibold text-slate-900 dark:text-white group-hover:text-primary transition-colors leading-snug flex-1",
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
                                      Button {
                                          variant: ButtonVariant::Ghost,
                                          size: ButtonSize::Sm,
                                          disabled: safe_page == 0,
                                          "aria-label": t(lang(), "blog.prev_page"),
                                          onclick: move |_| { if current_page() > 0 { current_page.set(current_page() - 1); } },
                                          "←"
                                      }
                                      span { class: "text-xs text-slate-400 tabular-nums", "{safe_page + 1} / {total_pages}" }
                                      Button {
                                          variant: ButtonVariant::Ghost,
                                          size: ButtonSize::Sm,
                                          disabled: safe_page + 1 >= total_pages,
                                          "aria-label": t(lang(), "blog.next_page"),
                                          onclick: move |_| { if current_page() + 1 < total_pages { current_page.set(current_page() + 1); } },
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
              div { class: "max-w-6xl mx-auto",
                  div { class: "text-slate-700 dark:text-slate-200 mb-12",
                      // 重构 B1：正文改由 BlogArticle 用 use_server_future 服务端预取（随 SSR HTML 下发）。
                      // SuspenseBoundary 捕获 BlogArticle 内 `?` 的挂起，未就绪时渲染 spinner。
                      SuspenseBoundary {
                          fallback: |_| rsx! {
                              div { class: "flex items-center justify-center py-20",
                                  Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                              }
                          },
                          BlogArticle { id: id.clone() }
                      }
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
  let lang = use_i18n();
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
    Some(Ok(Some(content))) => rsx! {
        div { class: "flex justify-center gap-10",
            article { class: "min-w-0 flex-1 max-w-4xl",
                // SEO 注入 + 文章头（Z2）：标题 / 日期 / 标签来自 frontmatter。正文已经以 `# 标题`
                // 开头的（如示例文章）不再重复输出标题。
                {
                    let (meta, body) = parse_mdx(&content);
                    let show_title = !meta.title.is_empty() && !starts_with_h1(&body);
                    let date = meta.date.clone().unwrap_or_default();
                    rsx! {
                        {inject_seo(&meta, &blog_path, &base_url)}
                        if show_title {
                            h1 { class: "text-3xl sm:text-4xl font-extrabold tracking-tight text-slate-900 dark:text-white mb-3", "{meta.title}" }
                        }
                        if !date.is_empty() || !meta.tags.is_empty() {
                            div { class: "flex flex-wrap items-center gap-2 mb-8 text-xs",
                                if !date.is_empty() {
                                    time { class: "text-slate-400", datetime: "{date}", "{date}" }
                                }
                                for tag in meta.tags.iter() {
                                    span { class: badge_class(BadgeVariant::Secondary, "font-normal text-muted-foreground"), "#{tag}" }
                                }
                            }
                        }
                    }
                }
                // `blog_id` 在 widgets::Markdown 内仅用于拼图片相对路径 `/posts/<id>/...`，
                // **必须传纯 slug**（如 "welcome"）。
                Markdown { content: content.clone(), blog_id: id.clone() }
                {tip_jar(blog_path.clone())}
                // 标注层（resource_kind="blog"，path = 博客 id）
                {annotation_layer("blog", anno_path.clone())}
                // 评论与关联讨论只挂在存在的文章下
                {comments(id.clone())}
                {discussion("blog", id.clone())}
            }
            aside { class: "hidden xl:block w-56 shrink-0",
                div { class: "sticky top-20 max-h-[calc(100vh-6rem)] overflow-y-auto",
                    widgets::OnThisPage { entries: widgets::heading_outline(&content), title: t(lang(), "toc.title") }
                }
            }
        }
    },
    Some(Ok(None)) => rsx! { widgets::NotFound { message: t(lang(), "blog.not_found") } },
    Some(Err(_)) => rsx! { widgets::LoadFailed { message: t(lang(), "common.load_failed") } },
    None => rsx! {
        div { class: "flex items-center justify-center py-20",
            Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
        }
    },
  }
}

/// 正文第一个非空行是否为一级标题。
fn starts_with_h1(body: &str) -> bool {
  body.lines().find(|l| !l.trim().is_empty()).is_some_and(|l| l.trim_start().starts_with("# "))
}

#[component]
pub fn Cases() -> Element {
  rsx! { ModuleGate { id: "cases".to_string(), CasesIndexPage {} } }
}

#[component]
pub fn CaseDetail(slug: String) -> Element {
  rsx! {
      ModuleGate { id: "cases".to_string(),
          CaseDetailPage {
              slug: slug.clone(),
              footer: rsx! {
                  {discussion("case", slug.clone())}
              },
          }
      }
  }
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

/// /about：渲染 `assets/pages/about.md`（frontmatter 的 title / description 进 head）。
#[component]
pub fn About() -> Element {
  rsx! {
      ModuleGate { id: "about".to_string(),
          section { class: "py-12 bg-white dark:bg-slate-950",
              Container {
                  div { class: "max-w-3xl mx-auto text-slate-700 dark:text-slate-200",
                      SuspenseBoundary {
                          fallback: |_| rsx! {
                              div { class: "flex items-center justify-center py-20",
                                  Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                              }
                          },
                          AboutBody {}
                      }
                  }
              }
          }
      }
  }
}

#[component]
fn AboutBody() -> Element {
  let lang = use_i18n();
  let page = use_server_future(|| async move { get_about_page().await })?;
  match page() {
    Some(Ok(Some(content))) => {
      let (meta, _body) = parse_mdx(&content);
      let title =
        if meta.title.is_empty() { t(lang(), "footer.about") } else { meta.title.clone() };
      rsx! {
          widgets::PageMeta {
              title,
              description: meta.description.clone().unwrap_or_default(),
              path: "/about".to_string(),
          }
          Markdown { content, blog_id: "about".to_string() }
      }
    }
    Some(Ok(None)) => rsx! { widgets::NotFound { message: t(lang(), "common.not_found") } },
    Some(Err(_)) => rsx! { widgets::LoadFailed { message: t(lang(), "common.load_failed") } },
    None => rsx! {},
  }
}

#[component]
pub fn Podcast() -> Element {
  rsx! {
      ModuleGate { id: "podcast".to_string(),
          PodcastPage { footer: tip_jar("/podcast".to_string()) }
      }
  }
}

#[cfg(test)]
mod tests {
  use super::starts_with_h1;

  #[test]
  fn detects_leading_h1() {
    assert!(starts_with_h1("\n# 标题\n正文"));
    assert!(!starts_with_h1("正文\n# 标题"));
    assert!(!starts_with_h1("## 小节"));
    assert!(!starts_with_h1(""));
  }
}
