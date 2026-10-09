use crate::server::{
  get_annotations_config, get_course, get_last_lesson, get_lesson, get_progress, list_annotations,
  list_courses, list_my_annotations, mark_lesson_complete, Annotation, AnnotationsConfig, Chapter,
  CodeFile, Course, CourseSummary, DownloadFile, Lesson, LessonKind, LessonProgress, LessonSummary,
  MediaRef,
};
use dioxus::prelude::*;
use dioxus_shadcn::{
  button_class, card_class, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Collapsible,
  CollapsibleContent, CollapsibleTrigger, Progress, Spinner, SpinnerSize, Tabs, TabsContent,
  TabsList, TabsTrigger, UiDensity,
};
use widgets::Markdown;

// ============================================================
// Local layout helpers (本模块自用，不污染上层组件树)
// ============================================================

#[component]
fn LocalContainer(children: Element) -> Element {
  rsx! { div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8", {children} } }
}

#[component]
fn LocalSectionTitle(title: String, subtitle: Option<String>) -> Element {
  rsx! {
      div { class: "text-center mb-10",
          h2 { class: "text-3xl font-bold tracking-tight text-[var(--color-text)] sm:text-4xl",
              "{title}" }
          if let Some(s) = subtitle {
              p { class: "mt-4 text-lg leading-8 text-[var(--color-text-muted)]", "{s}" }
          }
      }
  }
}

// ============================================================
// /courses  Index Page
// ============================================================

/// 课程列表页：卡片网格。
/// 重构 B4：课程列表改由 `CoursesList` 用 use_server_future 服务端预取（随 SSR HTML 下发）。
#[component]
pub fn CoursesIndexPage() -> Element {
  rsx! {
      section { class: "py-12 min-h-screen bg-[var(--color-bg)] transition-colors duration-300",
          LocalContainer {
              LocalSectionTitle {
                  title: "Rust 课程".to_string(),
                  subtitle: Some("系统化学习路径，从基础到全栈实战".to_string())
              }

              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                      }
                  },
                  CoursesList {}
              }
          }
      }
  }
}

#[component]
fn CoursesList() -> Element {
  let courses_res = use_server_future(|| async move { list_courses().await.unwrap_or_default() })?;
  let list = courses_res().unwrap_or_default();

  if list.is_empty() {
    rsx! {
        div { class: "text-center text-slate-500 py-20",
            "暂无课程内容。把课程目录放到 ", code { "assets/courses/" }, " 下即可。"
        }
    }
  } else {
    rsx! {
        div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mt-8",
            for course in list.iter() {
                CourseCard { course: course.clone() }
            }
        }
    }
  }
}

#[component]
fn CourseCard(course: CourseSummary) -> Element {
  let CourseSummary {
    slug,
    title,
    description,
    cover,
    tags,
    level,
    chapter_count,
    lesson_count,
    ..
  } = course;
  let href = format!("/course/{slug}");
  rsx! {
      a {
          href: "{href}",
          class: card_class("group block rounded-2xl hover:border-primary/50 hover:shadow-lg transition-all overflow-hidden"),
          // 封面
          div { class: "aspect-video w-full bg-slate-200 dark:bg-slate-800 overflow-hidden",
              if let Some(ref c) = cover {
                  img {
                      src: "{c}",
                      class: "w-full h-full object-cover group-hover:scale-105 transition-transform duration-500",
                      alt: "{title}",
                  }
              } else {
                  div { class: "w-full h-full flex items-center justify-center text-slate-400",
                      span { class: "text-5xl", "📚" }
                  }
              }
          }
          // 内容
          div { class: "p-6",
              if let Some(ref lv) = level {
                  span { class: "inline-block text-[10px] font-bold uppercase tracking-widest text-primary mb-2",
                      "{lv}"
                  }
              }
              h3 { class: "text-lg font-bold text-slate-900 dark:text-white group-hover:text-primary transition-colors mb-2 line-clamp-2",
                  "{title}"
              }
              if !description.is_empty() {
                  p { class: "text-sm text-slate-600 dark:text-slate-400 mb-4 line-clamp-2",
                      "{description}"
                  }
              }
              div { class: "flex items-center gap-3 text-xs text-slate-500 dark:text-slate-400",
                  span { "{chapter_count} 章" }
                  span { "·" }
                  span { "{lesson_count} 节" }
              }
              if !tags.is_empty() {
                  div { class: "mt-3 flex flex-wrap gap-1.5",
                      for tag in tags.iter() {
                          Badge { variant: BadgeVariant::Secondary, class: "rounded-full font-normal", "#{tag}" }
                      }
                  }
              }
          }
      }
  }
}

// ============================================================
// /courses/:slug  Detail Page
// ============================================================

/// 重构 B4：课程详情页拆为 SuspenseBoundary 外壳 + `CourseDetailLoaded`；课程元数据
/// （get_course）由 use_server_future 服务端预取（SEO），进度 / 上次学习位置是每用户
/// DB 数据，保留客户端 use_resource。
#[component]
pub fn CourseDetailPage(slug: String) -> Element {
  rsx! {
      section { class: "py-12 min-h-screen bg-[var(--color-bg)] transition-colors duration-300",
          LocalContainer {
              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                      }
                  },
                  CourseDetailLoaded { slug: slug.clone() }
              }
          }
      }
  }
}

#[component]
fn CourseDetailLoaded(slug: String) -> Element {
  // 课程元数据：use_server_future 服务端预取，随 slug 变化重取。
  let course_res =
    use_server_future(use_reactive!(|slug| async move { get_course(slug).await.ok().flatten() }))?;

  // 进度 / 上次学习位置：每用户 DB 数据（非 SEO），保留客户端 use_resource。
  let slug_for_progress = slug.clone();
  let progress_res = use_resource(move || {
    let s = slug_for_progress.clone();
    async move { get_progress(s).await.unwrap_or_default() }
  });
  let progress = progress_res.read().as_ref().cloned().unwrap_or_default();

  let slug_for_last = slug.clone();
  let last_res = use_resource(move || {
    let s = slug_for_last.clone();
    async move { get_last_lesson(s).await.ok().flatten() }
  });
  let last = last_res.read().as_ref().cloned().flatten();

  match course_res() {
    Some(Some(c)) => {
      rsx! { CourseDetailBody { course: c, progress: progress.clone(), last: last.clone() } }
    }
    _ => rsx! {
        div { class: "text-center py-20",
            h2 { class: "text-2xl font-bold text-slate-900 dark:text-white mb-4",
                "课程未找到"
            }
            p { class: "text-slate-500", "课程 \"{slug}\" 不存在或尚未发布。" }
            a { href: "/course",
                class: button_class(ButtonVariant::Primary, ButtonSize::Md, UiDensity::Comfortable, "mt-6"),
                "返回课程列表"
            }
        }
    },
  }
}

/// 进度查找表：`<chapter>/<lesson>` 是否已完成
fn lesson_completed(progress: &[LessonProgress], chapter: &str, lesson: &str) -> bool {
  let path = format!("{}/{}", chapter, lesson);
  progress.iter().any(|p| p.completed && p.lesson_path == path)
}

#[component]
fn CourseDetailBody(
  course: Course,
  progress: Vec<LessonProgress>,
  last: Option<String>,
) -> Element {
  let total_lessons: usize = course.chapters.iter().map(|ch| ch.lessons.len()).sum();
  let completed_lessons: usize = course
    .chapters
    .iter()
    .map(|ch| ch.lessons.iter().filter(|l| lesson_completed(&progress, &ch.slug, &l.slug)).count())
    .sum();
  let percent = (completed_lessons * 100).checked_div(total_lessons).unwrap_or(0);
  let has_last = last.is_some();
  let continue_link = match &last {
    Some(p) => Some(format!("/course/{}/{}", course.slug, p)),
    None => first_lesson_link(&course),
  };
  let continue_label = if has_last { "继续学习" } else { "开始学习" };
  // 付费课程：查当前用户是否已拥有（决定显示「购买」还是「已拥有」）。
  let owned_res =
    use_resource(|| async move { crate::server::list_my_entitlements().await.unwrap_or_default() });
  let owned = owned_res.read().clone().unwrap_or_default().iter().any(|s| s == &course.slug);
  let yuan = course.price / 100;
  rsx! {
      // Hero
      div { class: "grid grid-cols-1 lg:grid-cols-3 gap-8 mb-12",
          // 封面
          div { class: "lg:col-span-1",
              div { class: "aspect-video w-full rounded-2xl overflow-hidden bg-slate-200 dark:bg-slate-800 shadow-xl",
                  if let Some(ref c) = course.cover {
                      img { src: "{c}", class: "w-full h-full object-cover", alt: "{course.title}" }
                  } else {
                      div { class: "w-full h-full flex items-center justify-center text-slate-400",
                          span { class: "text-7xl", "📚" }
                      }
                  }
              }
          }
          // 信息
          div { class: "lg:col-span-2 flex flex-col justify-center",
              if let Some(ref lv) = course.level {
                  span { class: "inline-block text-[10px] font-bold uppercase tracking-widest text-primary mb-3",
                      "{lv}"
                  }
              }
              h1 { class: "text-3xl md:text-4xl font-extrabold text-slate-900 dark:text-white mb-4",
                  "{course.title}"
              }
              if !course.description.is_empty() {
                  p { class: "text-base md:text-lg text-slate-600 dark:text-slate-400 mb-6 leading-relaxed",
                      "{course.description}"
                  }
              }
              div { class: "flex items-center gap-3 text-sm text-slate-500 dark:text-slate-400",
                  span { class: "font-medium text-slate-700 dark:text-slate-300",
                      "{course.chapters.len()} 章"
                  }
                  span { "·" }
                  span { class: "font-medium text-slate-700 dark:text-slate-300",
                      "{total_lessons} 节"
                  }
              }
              if !course.tags.is_empty() {
                  div { class: "mt-4 flex flex-wrap gap-2",
                      for tag in course.tags.iter() {
                          Badge { variant: BadgeVariant::Secondary, class: "rounded-full px-2.5 py-1 font-normal", "#{tag}" }
                      }
                  }
              }
              // 付费课程购买入口
              if course.is_paid() {
                  if owned {
                      Badge { variant: BadgeVariant::Success, class: "mt-6 w-fit gap-2 rounded-lg px-4 py-2 text-sm font-medium",
                          "✓ 已拥有本课程"
                      }
                  } else {
                      div { class: "mt-6 flex items-center gap-4",
                          span { class: "text-2xl font-extrabold text-primary", "¥{yuan}" }
                          PurchaseEntry { course_slug: course.slug.clone(), price: course.price }
                      }
                  }
              }
              // 进度条
              if total_lessons > 0 {
                  div { class: "mt-6 max-w-md",
                      div { class: "flex items-center justify-between text-xs text-slate-500 dark:text-slate-400 mb-1",
                          span { "学习进度" }
                          span { "{completed_lessons}/{total_lessons} · {percent}%" }
                      }
                      Progress { value: percent as f32, class: "h-2 bg-border", "aria-label": "学习进度" }
                  }
              }
              // 继续学习按钮
              if let Some(href) = continue_link {
                  a { href: "{href}",
                      class: button_class(ButtonVariant::Primary, ButtonSize::Md, UiDensity::Comfortable, "mt-8 w-fit gap-2 px-5"),
                      "{continue_label}"
                      span { "→" }
                  }
              }
          }
      }

      // 章节手风琴
      h2 { class: "text-2xl font-bold text-slate-900 dark:text-white mb-6", "课程目录" }
      div { class: "space-y-4",
          for chapter in course.chapters.iter() {
              ChapterAccordion {
                  course_slug: course.slug.clone(),
                  chapter: chapter.clone(),
                  progress: progress.clone(),
                  course_paid: course.is_paid(),
              }
          }
      }
  }
}

fn first_lesson_link(c: &Course) -> Option<String> {
  let ch = c.chapters.first()?;
  let l = ch.lessons.first()?;
  Some(format!("/course/{}/{}/{}", c.slug, ch.slug, l.slug))
}

#[component]
fn ChapterAccordion(
  course_slug: String,
  chapter: Chapter,
  progress: Vec<LessonProgress>,
  course_paid: bool,
) -> Element {
  let lesson_count = chapter.lessons.len();

  rsx! {
      Collapsible {
          default_open: true,
          class: "gap-0 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 overflow-hidden",
          CollapsibleTrigger {
              class: "group w-full rounded-none px-5 py-4 text-left hover:bg-slate-50 hover:text-foreground dark:hover:bg-slate-900/60",
              div { class: "flex items-center gap-3",
                  span { class: "text-xs font-bold uppercase tracking-wider text-slate-400 dark:text-slate-500",
                      "Ch.{chapter.order}"
                  }
                  h3 { class: "text-base font-semibold text-slate-900 dark:text-white",
                      "{chapter.title}"
                  }
                  span { class: "text-xs font-normal text-slate-400 dark:text-slate-500",
                      "{lesson_count} 节"
                  }
              }
              svg {
                  class: "w-4 h-4 shrink-0 text-slate-400 transition-transform group-data-[state=open]:rotate-180",
                  fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                  path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2",
                      d: "M19 9l-7 7-7-7" }
              }
          }
          CollapsibleContent { class: "text-base text-foreground",
              if !chapter.description.is_empty() {
                  p { class: "px-5 pb-2 text-sm text-slate-500 dark:text-slate-400",
                      "{chapter.description}"
                  }
              }
              ul { class: "border-t border-slate-100 dark:border-slate-800/60",
                  for lesson in chapter.lessons.iter() {
                      LessonRow {
                          course_slug: course_slug.clone(),
                          chapter_slug: chapter.slug.clone(),
                          lesson: lesson.clone(),
                          completed: lesson_completed(&progress, &chapter.slug, &lesson.slug),
                          course_paid,
                      }
                  }
              }
          }
      }
  }
}

#[component]
fn LessonRow(
  course_slug: String,
  chapter_slug: String,
  lesson: LessonSummary,
  completed: bool,
  course_paid: bool,
) -> Element {
  let href = format!("/course/{}/{}/{}", course_slug, chapter_slug, lesson.slug);
  let icon = lesson.kind.icon();
  let kind_label = lesson.kind.as_str();
  // 付费课程：非试看课节加锁标记；试看课节标「试看」。
  let locked = course_paid && !lesson.preview;
  rsx! {
      li {
          a { href: "{href}",
              class: "flex items-center gap-3 px-5 py-3 hover:bg-slate-50 dark:hover:bg-slate-900/60 transition-colors group",
              if completed {
                  span { class: "text-base flex-shrink-0 text-green-500", "✅" }
              } else {
                  span { class: "text-base flex-shrink-0", "{icon}" }
              }
              span { class: "flex-1 text-sm text-slate-700 dark:text-slate-300 group-hover:text-primary",
                  "{lesson.title}"
              }
              if lesson.preview && course_paid {
                  Badge { variant: BadgeVariant::Success, class: "px-1.5 text-[10px]", "试看" }
              }
              if locked {
                  span { class: "text-xs flex-shrink-0 text-slate-400", "🔒" }
              }
              span { class: "text-[10px] uppercase tracking-wider text-slate-400 dark:text-slate-500",
                  "{kind_label}"
              }
              if let Some(ref d) = lesson.duration {
                  span { class: "text-xs text-slate-400 dark:text-slate-500", "{d}" }
              }
          }
      }
  }
}

// ============================================================
// /courses/:slug/:chapter/:lesson  Lesson Page
// 按 LessonKind 自适应布局：Doc / Video / Audio / Code
// ============================================================

/// 重构 B4：课节页拆为 SuspenseBoundary 外壳 + `LessonLoaded`；课节内容（get_lesson）
/// 由 use_server_future 服务端预取（SEO）。返回链接在外壳始终展示。
#[component]
pub fn LessonPage(slug: String, chapter: String, lesson: String) -> Element {
  rsx! {
      section { class: "py-8 min-h-screen bg-[var(--color-bg)]",
          LocalContainer {
              a { href: "/course/{slug}",
                  class: "inline-flex items-center gap-1 text-sm text-primary hover:underline mb-6",
                  "← 返回课程目录"
              }
              SuspenseBoundary {
                  fallback: |_| rsx! {
                      div { class: "flex items-center justify-center py-20",
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                      }
                  },
                  LessonLoaded {
                      slug: slug.clone(),
                      chapter: chapter.clone(),
                      lesson: lesson.clone(),
                  }
              }
          }
      }
  }
}

#[component]
fn LessonLoaded(slug: String, chapter: String, lesson: String) -> Element {
  let blog_id = format!("course:{}/{}/{}", slug, chapter, lesson);
  // 课节内容：use_server_future 服务端预取，随（slug, chapter, lesson）变化重取。
  let lesson_res = use_server_future(use_reactive!(|slug, chapter, lesson| async move {
    get_lesson(slug, chapter, lesson).await.ok().flatten()
  }))?;

  match lesson_res() {
    Some(Some(l)) => rsx! {
        LessonContent {
            lesson: l,
            blog_id: blog_id.clone(),
            course_slug: slug.clone(),
            chapter_slug: chapter.clone(),
            lesson_slug: lesson.clone(),
        }
    },
    _ => rsx! {
        div { class: "text-center py-20 text-slate-500", "课节未找到。" }
    },
  }
}

/// Lesson 主体：根据 kind 选择主区布局，右侧栏统一放代码 / 下载。
#[component]
fn LessonContent(
  lesson: Lesson,
  blog_id: String,
  course_slug: String,
  chapter_slug: String,
  lesson_slug: String,
) -> Element {
  let has_sidebar = !lesson.code.is_empty() || !lesson.downloads.is_empty();
  let resource_path = format!("{}/{}/{}", course_slug, chapter_slug, lesson_slug);
  // M4c：锁定课节只渲染头部 + Paywall，不渲染正文/资源/标注。
  if lesson.locked {
    return rsx! {
        div { class: "flex items-center gap-3 mb-3",
            span { class: "text-xl", "🔒" }
            span { class: "text-[11px] font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-500",
                "{lesson.kind.as_str()}"
            }
        }
        h1 { class: "text-3xl md:text-4xl font-extrabold text-slate-900 dark:text-white mb-6",
            "{lesson.title}"
        }
        LessonPaywall { course_slug: course_slug.clone(), price: lesson.price }
    };
  }
  rsx! {
      // 顶部头
      div { class: "flex items-center gap-3 mb-3",
          span { class: "text-xl", "{lesson.kind.icon()}" }
          span { class: "text-[11px] font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-500",
              "{lesson.kind.as_str()}"
          }
          if let Some(audio) = lesson.audio.as_ref() {
              if let Some(d) = audio.duration.as_ref() {
                  span { class: "text-xs text-slate-400", "· {d}" }
              }
          }
      }
      h1 { class: "text-3xl md:text-4xl font-extrabold text-slate-900 dark:text-white mb-6",
          "{lesson.title}"
      }

      // 布局容器（主区 + 右侧栏）
      div { class: format_args!(
          "grid gap-8 {}",
          if has_sidebar { "grid-cols-1 lg:grid-cols-3" } else { "grid-cols-1" }
      ),
          div { class: format_args!("min-w-0 {}", if has_sidebar { "lg:col-span-2" } else { "" }),
              {render_main_area(&lesson, &blog_id)}
              CompleteLessonButton {
                  course_slug: course_slug.clone(),
                  chapter_slug: chapter_slug.clone(),
                  lesson_slug: lesson_slug.clone(),
              }
          }
          if has_sidebar {
              aside { class: "space-y-6 lg:sticky lg:top-20 self-start",
                  if !lesson.code.is_empty() {
                      CodeTabs { files: lesson.code.clone() }
                  }
                  if !lesson.downloads.is_empty() {
                      DownloadList { items: lesson.downloads.clone() }
                  }
              }
          }
      }
      // 标注层（资源范围 = 当前 lesson 叶子页）
      AnnotationLayer { resource_kind: "course".to_string(), resource_path: resource_path }
  }
}

/// 付费墙：锁定课节的占位卡片 + 购买入口（M5d）。
#[component]
fn LessonPaywall(course_slug: String, price: i64) -> Element {
  let yuan = price / 100;
  rsx! {
      div { class: card_class("rounded-2xl p-10 text-center max-w-xl mx-auto"),
          div { class: "w-14 h-14 mx-auto rounded-full bg-primary/10 flex items-center justify-center text-3xl mb-5",
              "🔒"
          }
          h2 { class: "text-xl font-bold text-slate-900 dark:text-white", "本课节为付费内容" }
          p { class: "mt-3 text-slate-600 dark:text-slate-400 leading-relaxed",
              "开通本课程后即可解锁全部课节，含文档、视频、音频与代码资源。"
          }
          if price > 0 {
              div { class: "mt-5 text-3xl font-extrabold text-primary", "¥{yuan}" }
          }
          div { class: "mt-6",
              PurchaseEntry { course_slug, price }
          }
          p { class: "mt-4 text-sm text-slate-400",
              "需登录后购买；试看课节免费开放。"
          }
      }
  }
}

/// 购买入口（PM4 feature 门控）：`payments` 开启时渲染在线购买按钮；
/// 关闭时退化为「联系管理员开通」提示（权益仍可由 /admin/entitlements
/// 手动授予，Paywall 鉴权不变）。
#[component]
fn PurchaseEntry(course_slug: String, price: i64) -> Element {
  #[cfg(feature = "payments")]
  {
    rsx! {
        crate::pay_ui::PurchaseButton { course_slug, price }
    }
  }
  #[cfg(not(feature = "payments"))]
  {
    let _ = (course_slug, price);
    rsx! {
        span { class: "inline-flex items-center rounded-md bg-slate-100 dark:bg-slate-800 px-4 py-2.5 text-sm font-medium text-slate-500 dark:text-slate-400",
            "在线购买未开通，请联系管理员开通课程权益"
        }
    }
  }
}

// ============================================================
// Complete-lesson button (PR-C)
// ============================================================

#[component]
fn CompleteLessonButton(course_slug: String, chapter_slug: String, lesson_slug: String) -> Element {
  let mut completed = use_signal(|| false);
  let mut pending = use_signal(|| false);
  let cs = course_slug.clone();
  let lp = format!("{}/{}", chapter_slug, lesson_slug);

  // 进入页面时获取当前 lesson 状态
  {
    let cs2 = cs.clone();
    let lp2 = lp.clone();
    use_effect(move || {
      let cs2 = cs2.clone();
      let lp2 = lp2.clone();
      spawn(async move {
        let list = get_progress(cs2).await.unwrap_or_default();
        if list.iter().any(|p| p.completed && p.lesson_path == lp2) {
          completed.set(true);
        }
      });
    });
  }

  rsx! {
      div { class: "mt-12 pt-8 border-t border-slate-200 dark:border-slate-800 flex items-center gap-4",
          Button {
              variant: if completed() { ButtonVariant::Secondary } else { ButtonVariant::Primary },
              disabled: pending(),
              class: if completed() { "px-5 cursor-default bg-success/15 text-success hover:bg-success/15" } else { "px-5" },
              onclick: move |_| {
                  if completed() || pending() { return; }
                  let cs = cs.clone();
                  let lp = lp.clone();
                  pending.set(true);
                  spawn(async move {
                      let res = mark_lesson_complete(cs, lp, true).await;
                      pending.set(false);
                      if res.is_ok() {
                          completed.set(true);
                      }
                  });
              },
              if completed() {
                  "✅ 已完成本节"
              } else if pending() {
                  "提交中…"
              } else {
                  "标记完成本节"
              }
          }
          span { class: "text-xs text-slate-400 dark:text-slate-500",
              "需登录才能记录进度"
          }
      }
  }
}

// ============================================================
// Annotation layer (PR-D)
// 导出供其它资源页（doc / blog）复用
// ============================================================

#[component]
pub fn AnnotationLayer(resource_kind: String, resource_path: String) -> Element {
  let cfg_res = use_resource(|| async move { get_annotations_config().await.unwrap_or_default() });
  let cfg = cfg_res.read().as_ref().cloned().unwrap_or(AnnotationsConfig {
    course: false,
    doc: false,
    blog: false,
  });
  let enabled = match resource_kind.as_str() {
    "course" => cfg.course,
    "doc" => cfg.doc,
    "blog" => cfg.blog,
    _ => false,
  };
  if !enabled {
    return rsx! { div { class: "hidden" } };
  }

  // 加载现有标注并交给 JS 渲染
  let kind_for_eff = resource_kind.clone();
  let path_for_eff = resource_path.clone();
  use_effect(move || {
    let kind = kind_for_eff.clone();
    let path = path_for_eff.clone();
    spawn(async move {
      let list = list_annotations(kind.clone(), path.clone()).await.unwrap_or_default();
      inject_annotations(&kind, &path, &list);
    });
  });

  rsx! { AnnotationToggle {} }
}

/// 浮动眼睛按钮：切换 body.no-anno 类以隐藏/显示标注样式（仅视图层，数据不动）。
/// 状态本地管理，选择记在 `localStorage['rie-anno-visible']`。
#[component]
fn AnnotationToggle() -> Element {
  // 初始从 localStorage 同步一下状态，后续完全本地主导
  let mut visible = use_signal(|| true);
  use_effect(move || {
    let v = widgets::browser::storage_get("rie-anno-visible").as_deref() != Some("0");
    widgets::browser::set_class(false, "no-anno", !v);
    visible.set(v);
  });

  // 固定在 navbar 正下方右侧，与页面标题几乎同水平，更易被发现。
  // 颜色走主题变量（bg-background / text-foreground），暗色下不再是白底。
  let label = if visible() { "隐藏标注" } else { "显示标注" };
  let icon_tone = if visible() { "text-foreground" } else { "text-muted-foreground" };
  rsx! {
      Button {
          r#type: "button",
          variant: ButtonVariant::Outline,
          size: ButtonSize::Icon,
          class: "fixed top-20 right-6 z-[9999] rounded-full shadow-lg {icon_tone}",
          title: label,
          "aria-label": label,
          onclick: move |_| {
              let next = !visible();
              visible.set(next);
              // 防御性：同时走 CSS 类（未来新创建的 span）+ 逐个 inline style。
              // 原因：!important 的 CSS 规则在某些热重载 / 幂等拦截场景下可能
              // 未被重新注入的样式表覆盖，直接写 inline style 最保险。
              widgets::browser::storage_set("rie-anno-visible", if next { "1" } else { "0" });
              widgets::browser::set_class(false, "no-anno", !next);
              widgets::browser::set_inline_styles(
                  "span.rie-anno",
                  &[("background", "transparent"), ("text-decoration", "none"), ("outline", "none")],
                  !next,
              );
          },
          // 单一 SVG 眼睛图标，隐藏状态多一道斜线
          svg {
              width: "20",
              height: "20",
              view_box: "0 0 24 24",
              fill: "none",
              stroke: "currentColor",
              stroke_width: "2",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              path { d: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" }
              circle { cx: "12", cy: "12", r: "3" }
              if !visible() {
                  path { d: "M3 3l18 18" }
              }
          }
      }
  }
}

/// 把标注数据交给 `assets/js/annotations.js` 来在 DOM 上包裹 span
fn inject_annotations(kind: &str, path: &str, list: &[Annotation]) {
  let payload = serde_json::json!({
      "kind": kind,
      "path": path,
      "items": list,
  });
  let data = serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string());
  // annotations.js 未加载时先挂到全局，它加载后自取
  if !widgets::browser::call_global("RIE_ANNO", "apply", Some(&data)) {
    widgets::browser::set_global_json("__rieAnnoPending", &data);
  }
}

/// 主区布局分派（按 kind）
fn render_main_area(lesson: &Lesson, blog_id: &str) -> Element {
  match lesson.kind {
    LessonKind::Doc => render_doc_main(lesson, blog_id),
    LessonKind::Video => render_video_main(lesson, blog_id),
    LessonKind::Audio => render_audio_main(lesson, blog_id),
    LessonKind::Code => render_code_main(lesson, blog_id),
  }
}

/// Doc Lesson：顶部紧凑音频条 + 可折叠视频块 + Markdown 正文
fn render_doc_main(lesson: &Lesson, blog_id: &str) -> Element {
  let markdown = lesson.doc.as_ref().map(|d| d.markdown.clone()).unwrap_or_default();
  rsx! {
      if let Some(audio) = lesson.audio.as_ref() {
          CompactAudioBar { audio: audio.clone() }
      }
      if let Some(video) = lesson.video.as_ref() {
          CollapsibleVideo { video: video.clone() }
      }
      div { class: "text-slate-700 dark:text-slate-200",
          Markdown { content: markdown, blog_id: blog_id.to_string() }
      }
  }
}

/// Video Lesson：顶部 16:9 视频 + 下方如有 index.md 则作为笔记/字幕
fn render_video_main(lesson: &Lesson, blog_id: &str) -> Element {
  rsx! {
      if let Some(video) = lesson.video.as_ref() {
          VideoPlayer { video: video.clone() }
      }
      if let Some(audio) = lesson.audio.as_ref() {
          div { class: "mt-4",
              CompactAudioBar { audio: audio.clone() }
          }
      }
      if let Some(doc) = lesson.doc.as_ref() {
          div { class: "mt-8 text-slate-700 dark:text-slate-200",
              Markdown { content: doc.markdown.clone(), blog_id: blog_id.to_string() }
          }
      }
  }
}

/// Audio Lesson：顶部音频卡片 + 下方 index.md 转写/笔记
fn render_audio_main(lesson: &Lesson, blog_id: &str) -> Element {
  rsx! {
      if let Some(audio) = lesson.audio.as_ref() {
          AudioCard { audio: audio.clone(), title: lesson.title.clone() }
      }
      if let Some(doc) = lesson.doc.as_ref() {
          div { class: "mt-8 text-slate-700 dark:text-slate-200",
              Markdown { content: doc.markdown.clone(), blog_id: blog_id.to_string() }
          }
      }
  }
}

/// Code Lesson：主区即代码 Tab（右侧栏不再重复） + 下方 index.md 题解
fn render_code_main(lesson: &Lesson, blog_id: &str) -> Element {
  rsx! {
      if !lesson.code.is_empty() {
          CodeTabs { files: lesson.code.clone(), large: true }
      } else {
          div { class: "text-sm text-slate-500", "本课节暂无代码文件。" }
      }
      if let Some(doc) = lesson.doc.as_ref() {
          div { class: "mt-8 text-slate-700 dark:text-slate-200",
              Markdown { content: doc.markdown.clone(), blog_id: blog_id.to_string() }
          }
      }
  }
}

// ============================================================
// Reusable presentational pieces
// ============================================================

/// 紧凑音频条（Doc Lesson 顶部 / Video Lesson 辅助位）
#[component]
fn CompactAudioBar(audio: MediaRef) -> Element {
  rsx! {
      div { class: "sticky top-14 z-10 my-4 px-4 py-3 rounded-xl border border-slate-200 dark:border-slate-800 bg-white/80 dark:bg-slate-900/80 backdrop-blur flex items-center gap-3 shadow-sm",
          span { class: "text-base", "🎧" }
          audio { class: "flex-1 h-8", controls: true, src: "{audio.url}" }
          if let Some(d) = audio.duration.as_ref() {
              span { class: "text-xs text-slate-400", "{d}" }
          }
      }
  }
}

/// 可折叠视频块（Doc Lesson 辅助位）
#[component]
fn CollapsibleVideo(video: MediaRef) -> Element {
  let poster = video.poster.clone().unwrap_or_default();
  let url = video.url.clone();
  rsx! {
      Collapsible {
          default_open: true,
          class: "gap-0 my-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden",
          CollapsibleTrigger {
              class: "group w-full rounded-none px-4 py-2.5 text-left hover:bg-slate-50 hover:text-foreground dark:hover:bg-slate-900/60",
              span { class: "text-sm font-medium text-slate-700 dark:text-slate-200 flex items-center gap-2",
                  "🎬 课节视频"
                  if let Some(d) = video.duration.as_ref() {
                      span { class: "text-xs text-slate-400 font-normal", "{d}" }
                  }
              }
              svg {
                  class: "w-4 h-4 shrink-0 text-slate-400 transition-transform group-data-[state=open]:rotate-180",
                  fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                  path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M19 9l-7 7-7-7" }
              }
          }
          CollapsibleContent { class: "aspect-video w-full bg-black",
              video {
                  class: "w-full h-full",
                  controls: true,
                  src: "{url}",
                  poster: if !poster.is_empty() { "{poster}" },
              }
          }
      }
  }
}

/// 主视频播放器（Video Lesson）
#[component]
fn VideoPlayer(video: MediaRef) -> Element {
  let poster = video.poster.clone().unwrap_or_default();
  let url = video.url.clone();
  rsx! {
      div { class: "aspect-video w-full rounded-2xl overflow-hidden bg-black shadow-xl",
          video {
              class: "w-full h-full",
              controls: true,
              src: "{url}",
              poster: if !poster.is_empty() { "{poster}" },
          }
      }
  }
}

/// 主音频卡片（Audio Lesson）
#[component]
fn AudioCard(audio: MediaRef, title: String) -> Element {
  rsx! {
      div { class: "relative overflow-hidden rounded-2xl bg-slate-900 shadow-xl border border-slate-200 dark:border-slate-800",
          div { class: "p-8 md:p-10 flex flex-col items-center text-center",
              div { class: "w-24 h-24 rounded-full bg-primary/20 flex items-center justify-center mb-6",
                  span { class: "text-5xl", "🎧" }
              }
              h3 { class: "text-2xl font-bold text-white mb-2", "{title}" }
              if let Some(d) = audio.duration.as_ref() {
                  div { class: "text-sm text-slate-400 mb-6", "时长 {d}" }
              }
              audio {
                  class: "w-full max-w-2xl focus:outline-none",
                  controls: true,
                  src: "{audio.url}",
              }
          }
      }
  }
}

// ============================================================
// Code Tabs (sidebar / large variants)
// ============================================================

#[component]
fn CodeTabs(files: Vec<CodeFile>, #[props(default = false)] large: bool) -> Element {
  let mut active = use_signal(|| 0usize);
  let active_idx = active().min(files.len().saturating_sub(1));
  let active_file = files.get(active_idx).cloned();
  let panel_class = if large {
    "rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden"
  } else {
    "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden"
  };

  // 加载后调用 PrismJS
  use_effect(move || {
    let _ = active(); // 让下面的 effect 随 tab 切换也重跑
    widgets::browser::call_global("Prism", "highlightAll", None);
  });

  // 页签值用下标：同一课节里文件名可能重复。
  rsx! {
      Tabs {
          class: "{panel_class}",
          value: active_idx.to_string(),
          on_value_change: move |v: String| active.set(v.parse().unwrap_or(0)),
          TabsList { class: "h-auto w-full justify-start gap-1 overflow-x-auto rounded-none border-b border-border bg-transparent px-2 pt-2 pb-0",
              for (i, f) in files.iter().enumerate() {
                  TabsTrigger { key: "{i}", value: i.to_string(), class: "rounded-b-none px-3 py-2 text-xs", "{f.name}" }
              }
          }
          if let Some(file) = active_file {
              TabsContent { value: active_idx.to_string(), class: "mt-0",
                  // 按文件重挂：Prism 已把 <code> 的文本节点换成高亮 span，
                  // 原地更新会写到已不存在的节点上。
                  CodePanel { key: "{active_idx}", file: file, large: large }
              }
          }
      }
  }
}

#[component]
fn CodePanel(file: CodeFile, large: bool) -> Element {
  let CodeFile { name, lang, content, raw_url } = file;
  let max_h_class = if large { "max-h-[70vh]" } else { "max-h-[60vh]" };
  let copy_payload = content.clone();
  rsx! {
      div { class: "relative",
          // 顶部工具条
          div { class: "flex items-center justify-end gap-2 px-3 py-1.5 text-[11px] text-slate-400 bg-slate-900",
              button {
                  class: "px-2 py-0.5 rounded hover:bg-slate-700 transition-colors",
                  onclick: move |_| {
                      let text = copy_payload.clone();
                      spawn(async move {
                          widgets::browser::copy_text(&text).await;
                      });
                  },
                  "复制"
              }
              a {
                  href: "{raw_url}",
                  download: "{name}",
                  class: "px-2 py-0.5 rounded hover:bg-slate-700 transition-colors",
                  "下载"
              }
          }
          pre { class: "overflow-auto bg-slate-900 p-4 {max_h_class}",
              code { class: "language-{lang} text-sm text-slate-200", "{content}" }
          }
      }
  }
}

// ============================================================
// Download list
// ============================================================

#[component]
fn DownloadList(items: Vec<DownloadFile>) -> Element {
  rsx! {
      div { class: card_class("rounded-xl overflow-hidden"),
          div { class: "px-4 py-3 border-b border-border",
              h3 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200", "下载附件" }
          }
          ul {
              for item in items.iter() {
                  li { class: "border-b last:border-b-0 border-slate-100 dark:border-slate-800/60",
                      a {
                          href: "{item.url}",
                          download: "{item.name}",
                          class: "flex items-center justify-between px-4 py-2.5 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-900/60 transition-colors",
                          span { class: "truncate", "📎 {item.name}" }
                          span { class: "text-xs text-slate-400 ml-3 flex-shrink-0",
                              "{format_size(item.size_bytes)}"
                          }
                      }
                  }
              }
          }
      }
  }
}

fn format_size(bytes: u64) -> String {
  const KB: u64 = 1024;
  const MB: u64 = KB * 1024;
  const GB: u64 = MB * 1024;
  if bytes >= GB {
    format!("{:.1} GB", bytes as f64 / GB as f64)
  } else if bytes >= MB {
    format!("{:.1} MB", bytes as f64 / MB as f64)
  } else if bytes >= KB {
    format!("{:.1} KB", bytes as f64 / KB as f64)
  } else {
    format!("{} B", bytes)
  }
}

// ============================================================
// /me/annotations　个人标注列表页（Feature 3）
// ============================================================

/// 个人标注列表页：拉取当前用户全部标注，按 (resource_kind, resource_path) 分组。
/// 点击可跳回原文位置（附带 #anno-{id}，由 annotations.js 负责闪烁该标注）。
#[component]
pub fn MyAnnotationsPage() -> Element {
  let res = use_resource(|| async move { list_my_annotations().await.unwrap_or_default() });
  let state = res.read().as_ref().cloned();
  rsx! {
      section { class: "py-12 min-h-screen bg-[var(--color-bg)] transition-colors duration-300",
          LocalContainer {
              LocalSectionTitle {
                  title: "我的标注".to_string(),
                  subtitle: Some("按资源分组、按创建时间倒序".to_string())
              }
              match state {
                  None => rsx! {
                      div { class: "flex items-center justify-center py-20",
                          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
                      }
                  },
                  Some(list) if list.is_empty() => rsx! {
                      div { class: "text-center text-slate-500 py-20",
                          p { class: "mb-2", "还没有任何标注。" }
                          p { class: "text-sm", "在文档 / 课程 / 博客页选中文本即可创建标注；如未登录请先登录。" }
                      }
                  },
                  Some(list) => rsx! {
                      div { class: "space-y-8 max-w-4xl mx-auto",
                          for group in group_annotations(list).into_iter() {
                              AnnotationGroupCard { group: group }
                          }
                      }
                  },
              }
          }
      }
  }
}

/// 一组 (kind, path) 下的标注
#[derive(Clone, PartialEq)]
struct AnnoGroup {
  kind: String,
  path: String,
  items: Vec<Annotation>,
}

/// 按 (kind, path) 分组，组内保持输入序。输入已按 created_at desc，因此输出也是。
fn group_annotations(list: Vec<Annotation>) -> Vec<AnnoGroup> {
  let mut groups: Vec<AnnoGroup> = Vec::new();
  for a in list {
    let key = (a.resource_kind.clone(), a.resource_path.clone());
    if let Some(g) = groups.iter_mut().find(|g| g.kind == key.0 && g.path == key.1) {
      g.items.push(a);
    } else {
      groups.push(AnnoGroup { kind: key.0, path: key.1, items: vec![a] });
    }
  }
  groups
}

/// 根据 (kind, path) 拼接原文跳转链接；带标注 id 时附 `#anno-{id}`。
/// 不用 block_id：它是顶层块序号，正文一改就指向别的块，而 annotations.js
/// 会按原文重新定位标注，闪烁该标注本身才准。
fn build_jump_url(kind: &str, path: &str, annotation_id: Option<i64>) -> String {
  let hash = annotation_id.map(|id| format!("#anno-{id}")).unwrap_or_default();
  match kind {
    "course" => format!("/course/{}{}", path, hash),
    "doc" => format!("/docs/{}{}", path, hash),
    "blog" => format!("/blog/{}{}", path, hash),
    _ => format!("/{}{}", path, hash),
  }
}

fn kind_badge(kind: &str) -> (&'static str, &'static str) {
  match kind {
    "course" => ("📚", "课程"),
    "doc" => ("📄", "文档"),
    "blog" => ("✍️", "博客"),
    _ => ("🔖", kind_label_fallback(kind)),
  }
}

fn kind_label_fallback(kind: &str) -> &'static str {
  match kind {
    "course" => "课程",
    "doc" => "文档",
    "blog" => "博客",
    _ => "资源",
  }
}

fn style_swatch_class(style: &str) -> &'static str {
  match style {
    "yellow" => "bg-yellow-300/60",
    "green" => "bg-green-400/60",
    "blue" => "bg-blue-400/60",
    "pink" => "bg-pink-400/60",
    "purple" => "bg-purple-400/60",
    "underline" => "underline decoration-2 underline-offset-2",
    "wavy" => "underline decoration-wavy decoration-2 underline-offset-2",
    "strikethrough" => "line-through decoration-2",
    _ => "bg-yellow-300/60",
  }
}

fn visibility_label(v: &str) -> (&'static str, &'static str) {
  match v {
    "public" => ("🌐", "公开"),
    "course-public" => ("📚", "课程内公开"),
    "doc-public" => ("📄", "文档内公开"),
    _ => ("🔒", "私密"),
  }
}

// ============================================================
// Tests — 纯函数单元测试（不需要 Dioxus 运行时 / DB）
// ============================================================

#[cfg(test)]
mod course_helpers_tests {
  use super::*;

  fn mk_anno(
    id: i64,
    kind: &str,
    path: &str,
    block_id: &str,
    style: &str,
    visibility: &str,
    author: Option<&str>,
  ) -> Annotation {
    Annotation {
      id,
      user_id: 1,
      resource_kind: kind.to_string(),
      resource_path: path.to_string(),
      block_id: block_id.to_string(),
      start_offset: 0,
      end_offset: 5,
      exact_text: "abcde".to_string(),
      prefix_text: None,
      suffix_text: None,
      style: style.to_string(),
      note: None,
      visibility: visibility.to_string(),
      created_at: "2026-01-01 00:00".to_string(),
      author_nickname: author.map(|s| s.to_string()),
    }
  }

  #[test]
  fn test_build_jump_url_per_kind() {
    assert_eq!(
      build_jump_url("course", "rust-basics/01-foo/02-bar", Some(3)),
      "/course/rust-basics/01-foo/02-bar#anno-3"
    );
    assert_eq!(
      build_jump_url("doc", "axum/basic/router", Some(1)),
      "/docs/axum/basic/router#anno-1"
    );
    assert_eq!(build_jump_url("blog", "welcome", Some(2)), "/blog/welcome#anno-2");
  }

  #[test]
  fn test_build_jump_url_without_annotation_no_hash() {
    assert_eq!(build_jump_url("course", "a/b/c", None), "/course/a/b/c");
    assert_eq!(build_jump_url("doc", "foo", None), "/docs/foo");
  }

  #[test]
  fn test_build_jump_url_unknown_kind_falls_back() {
    assert_eq!(build_jump_url("weird", "x/y", Some(9)), "/x/y#anno-9");
  }

  #[test]
  fn test_kind_badge_known_kinds() {
    let (icon, label) = kind_badge("course");
    assert_eq!(label, "课程");
    assert!(!icon.is_empty());
    assert_eq!(kind_badge("doc").1, "文档");
    assert_eq!(kind_badge("blog").1, "博客");
  }

  #[test]
  fn test_kind_badge_fallback() {
    let (_, label) = kind_badge("unknown");
    assert_eq!(label, "资源");
  }

  #[test]
  fn test_visibility_label_all_variants() {
    assert_eq!(visibility_label("public").1, "公开");
    assert_eq!(visibility_label("course-public").1, "课程内公开");
    assert_eq!(visibility_label("doc-public").1, "文档内公开");
    assert_eq!(visibility_label("private").1, "私密");
    // 未知取值归为私密
    assert_eq!(visibility_label("hacker-attempt").1, "私密");
  }

  #[test]
  fn test_style_swatch_class_known_styles() {
    assert_eq!(style_swatch_class("yellow"), "bg-yellow-300/60");
    assert_eq!(style_swatch_class("green"), "bg-green-400/60");
    assert_eq!(style_swatch_class("blue"), "bg-blue-400/60");
    assert_eq!(style_swatch_class("pink"), "bg-pink-400/60");
    assert_eq!(style_swatch_class("purple"), "bg-purple-400/60");
    assert!(style_swatch_class("underline").contains("underline"));
    assert!(style_swatch_class("wavy").contains("decoration-wavy"));
    assert!(style_swatch_class("strikethrough").contains("line-through"));
  }

  #[test]
  fn test_style_swatch_class_unknown_falls_back_to_yellow() {
    assert_eq!(style_swatch_class(""), "bg-yellow-300/60");
    assert_eq!(style_swatch_class("rainbow"), "bg-yellow-300/60");
  }

  #[test]
  fn test_group_annotations_preserves_input_order_within_group() {
    // 输入中同一资源的多条按 created_at desc 出现（这里用 id 递增模拟）
    let list = vec![
      mk_anno(3, "course", "a/01/01", "b3", "yellow", "private", None),
      mk_anno(2, "course", "a/01/01", "b1", "underline", "public", None),
      mk_anno(1, "doc", "axum/basic", "b1", "blue", "private", None),
    ];
    let groups = group_annotations(list);
    assert_eq!(groups.len(), 2);
    // 分组顺序：首次出现顺序 — course 在前
    assert_eq!(groups[0].kind, "course");
    assert_eq!(groups[0].path, "a/01/01");
    assert_eq!(groups[0].items.len(), 2);
    // 组内保持输入顺序
    assert_eq!(groups[0].items[0].id, 3);
    assert_eq!(groups[0].items[1].id, 2);
    assert_eq!(groups[1].kind, "doc");
    assert_eq!(groups[1].items[0].id, 1);
  }

  #[test]
  fn test_group_annotations_empty() {
    let groups = group_annotations(vec![]);
    assert!(groups.is_empty());
  }

  #[test]
  fn test_group_annotations_separates_different_paths_same_kind() {
    let list = vec![
      mk_anno(1, "doc", "a/x", "b1", "yellow", "private", None),
      mk_anno(2, "doc", "b/y", "b1", "yellow", "private", None),
      mk_anno(3, "doc", "a/x", "b2", "yellow", "private", None),
    ];
    let groups = group_annotations(list);
    assert_eq!(groups.len(), 2);
    // a/x 先出现，收集了 id 1 与 3
    let ax = groups.iter().find(|g| g.path == "a/x").unwrap();
    assert_eq!(ax.items.len(), 2);
    assert_eq!(ax.items[0].id, 1);
    assert_eq!(ax.items[1].id, 3);
    let by = groups.iter().find(|g| g.path == "b/y").unwrap();
    assert_eq!(by.items.len(), 1);
  }

  #[test]
  fn test_group_annotations_distinguishes_kind_when_path_collides() {
    // 同 path 但 kind 不同要分别在两个组里
    let list = vec![
      mk_anno(1, "course", "foo", "b1", "yellow", "private", None),
      mk_anno(2, "doc", "foo", "b1", "yellow", "private", None),
    ];
    let groups = group_annotations(list);
    assert_eq!(groups.len(), 2);
    assert_ne!(groups[0].kind, groups[1].kind);
  }
}

#[component]
fn AnnotationGroupCard(group: AnnoGroup) -> Element {
  let (icon, label) = kind_badge(&group.kind);
  let header_url = build_jump_url(&group.kind, &group.path, None);
  rsx! {
      div { class: card_class("rounded-2xl overflow-hidden"),
          // 资源头
          a { href: "{header_url}",
              class: "flex items-center justify-between gap-3 px-5 py-3 border-b border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-900/60 transition-colors",
              div { class: "flex items-center gap-2 min-w-0",
                  span { class: "text-base", "{icon}" }
                  span { class: "text-[10px] font-bold uppercase tracking-widest text-slate-400 dark:text-slate-500",
                      "{label}"
                  }
                  span { class: "text-sm font-medium text-slate-700 dark:text-slate-200 truncate",
                      "{group.path}"
                  }
              }
              span { class: "text-xs text-slate-400 dark:text-slate-500 flex-shrink-0",
                  "{group.items.len()} 条"
              }
          }
          // 标注条目
          ul {
              for anno in group.items.iter() {
                  AnnotationListItem {
                      kind: group.kind.clone(),
                      path: group.path.clone(),
                      anno: anno.clone(),
                  }
              }
          }
      }
  }
}

#[component]
fn AnnotationListItem(kind: String, path: String, anno: Annotation) -> Element {
  let url = build_jump_url(&kind, &path, Some(anno.id));
  let swatch = style_swatch_class(&anno.style);
  rsx! {
      li { class: "border-b last:border-b-0 border-slate-100 dark:border-slate-800/60",
          a { href: "{url}",
              class: "flex items-start gap-3 px-5 py-4 hover:bg-slate-50 dark:hover:bg-slate-900/60 transition-colors",
              // 颜色/样式快识
              span { class: "flex-shrink-0 w-2.5 h-2.5 rounded-full mt-1.5 {swatch}" }
              div { class: "flex-1 min-w-0",
                  // 选中文本快照
                  blockquote { class: "text-sm text-slate-800 dark:text-slate-100 leading-relaxed line-clamp-3 border-l-2 border-slate-300 dark:border-slate-700 pl-3",
                      "{anno.exact_text}"
                  }
                  if let Some(note) = anno.note.as_ref() {
                      if !note.is_empty() {
                          p { class: "mt-2 text-xs text-slate-500 dark:text-slate-400 italic",
                              "📝 {note}"
                          }
                      }
                  }
                  div { class: "mt-2 flex items-center gap-3 text-[11px] text-slate-400 dark:text-slate-500 flex-wrap",
                      span { "{anno.created_at}" }
                      span { "·" }
                      span { "{anno.style}" }
                      span { "·" }
                      span { class: "truncate", "#{anno.block_id}" }
                      {
                          let (icon, label) = visibility_label(&anno.visibility);
                          rsx! {
                              span { "·" }
                              Badge { variant: BadgeVariant::Secondary, class: "px-1.5 text-[11px] font-normal", "{icon} {label}" }
                          }
                      }
                      if let Some(name) = anno.author_nickname.as_ref() {
                          span { "·" }
                          span { class: "text-slate-500 dark:text-slate-400",
                              "作者: {name}"
                          }
                      }
                  }
              }
              span { class: "flex-shrink-0 self-center text-primary text-sm",
                  "跳转 →"
              }
          }
      }
  }
}
