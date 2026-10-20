use dioxus::prelude::*;
use dioxus::router::{Link, Routable};
use dioxus_shadcn::{
  badge_class, BadgeVariant, Button, ButtonSize, ButtonVariant, Spinner, SpinnerSize,
};

use crate::components::admin_entitlements::AdminEntitlementsPage;
use crate::components::comment::CommentBox;
use crate::components::hero::Hero;
use crate::components::home_sections::{
  CommunityFeed, CourseShowcase, EcosystemPage, EcosystemPillars, FeaturedCases,
};
use crate::components::module_gate::{use_enabled_modules, ModuleGate};
use crate::components::nav::Navbar;
use crate::components::view::Container;
use crate::i18n::{t, use_i18n};
use module_admin::admin::{
  AdminCommentsPage, AdminDashboardPage, AdminModerationPage, AdminModerationSettingsPage,
  AdminTopicsPage, AdminUsersPage,
};
use module_ai::{AiArticlePage, AiIndexPage};
use module_blog::server::{get_blog_content, list_blog_posts};
use module_cases::cases::{CaseDetailPage, CasesIndexPage};
use module_cli::{CliArticlePage, CliIndexPage};
use module_course::course::{
  AnnotationLayer, CourseDetailPage, CoursesIndexPage, LessonPage, MyAnnotationsPage,
};
#[cfg(feature = "payments")]
use module_course::pay_ui::MyOrdersPage;
use module_docs::docs::{DocPage as DocPageView, Docs as DocsView};
use module_embedded::{EmbeddedArticlePage, EmbeddedIndexPage};
use module_forum::forum::{
  DiscussionPanel, MyTopicsPage, NewTopicPage, TopicDetailPage, TopicsByTagPage, TopicsIndexPage,
};
use module_podcast::podcast::PodcastPage;
use module_wasm::{WasmArticlePage, WasmIndexPage};
use module_web3::{Web3ArticlePage, Web3IndexPage};
use widgets::{get_seo_base_url, inject_seo, parse_mdx, Markdown};

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
      }
      Hero { enabled: enabled.clone() }

      // 两大生态支柱：直接体现「Rust 生态 + AI 生态」定位
      EcosystemPillars { enabled: enabled.clone() }

      // 精选案例（旗舰）+ 课程（变现）
      if on("cases") {
          FeaturedCases {}
      }
      if on("course") {
          CourseShowcase {}
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
                  h2 { class: "text-3xl font-bold tracking-tight text-slate-900 dark:text-white sm:text-4xl", "{t(lang(), \"blog.title\")}" }
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
              div { class: "max-w-4xl mx-auto",
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
        // 评论与关联讨论只挂在存在的文章下
        div { class: "border-t border-slate-200 dark:border-slate-800 pt-8 mt-12",
            CommentBox { blog_id: id.clone() }
        }
        DiscussionPanel {
            resource_kind: "blog".to_string(),
            resource_path: id.clone(),
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
  rsx! {
      ModuleGate { id: "cases".to_string(),
          CaseDetailPage {
              slug: slug.clone(),
              footer: rsx! {
                  DiscussionPanel { resource_kind: "case".to_string(), resource_path: slug.clone() }
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

/// /me/orders 我的订单页（payments feature 门控；关闭时渲染占位提示，
/// 路由保留以免旧链接 404）。
#[component]
pub fn MyOrders() -> Element {
  #[cfg(feature = "payments")]
  {
    rsx! { ModuleGate { id: "course".to_string(), MyOrdersPage {} } }
  }
  #[cfg(not(feature = "payments"))]
  {
    rsx! {
        section { class: "py-20 bg-white dark:bg-slate-950 min-h-[60vh]",
            div { class: "mx-auto max-w-4xl px-4 text-center text-slate-500",
                "在线支付未开通，暂无订单可查。如需开通课程权益请联系管理员。"
            }
        }
    }
  }
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
pub fn AdminEntitlements() -> Element {
  rsx! { AdminEntitlementsPage {} }
}

#[component]
pub fn AdminModeration() -> Element {
  rsx! { AdminModerationPage {} }
}

#[component]
pub fn AdminModerationSettings() -> Element {
  rsx! { AdminModerationSettingsPage {} }
}
