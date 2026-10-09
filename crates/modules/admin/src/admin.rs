use crate::server::{
  admin_approve_moderation, admin_bulk_approve_moderation, admin_bulk_reject_moderation,
  admin_delete_comment, admin_delete_reply, admin_delete_topic, admin_get_moderation_settings,
  admin_list_comments, admin_list_moderation_queue, admin_list_topics, admin_list_users,
  admin_overview, admin_reject_moderation, admin_set_moderation_settings, admin_set_user_role,
  AdminCommentRow, AdminTopicRow, AdminUserRow, ModerationQueueRow, ADMIN_PAGE_SIZE,
};
use app_core::session::{SessionUser, ALL_ROLES};
use app_core::settings::{LlmFailureAction, ModerationSettings, ModerationThresholdsConfig};
use dioxus::prelude::*;
use dioxus_shadcn::{
  button_class, use_density, AlertDialog, AlertDialogAction, AlertDialogActionVariant,
  AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter,
  AlertDialogHeader, AlertDialogOverlay, AlertDialogTitle, AlertDialogTrigger, Badge, BadgeVariant,
  Button, ButtonSize, ButtonVariant, Checkbox, DensityProvider, Input, NativeSelect,
  NativeSelectOption, Spinner, SpinnerSize, Switch, Tabs, TabsContent, TabsList, TabsTrigger,
  Textarea, UiDensity,
};

// =============================================================
// 共享上下文 hooks
// =============================================================

fn use_session_user_ctx() -> Option<Signal<Option<SessionUser>>> {
  try_use_context::<Signal<Option<SessionUser>>>()
}

/// 当前用户是否为 admin
pub fn is_current_user_admin() -> bool {
  use_session_user_ctx()
    .map(|s| s.read().as_ref().map(|u| u.is_admin()).unwrap_or(false))
    .unwrap_or(false)
}

// =============================================================
// 公共布局
// =============================================================

#[component]
pub fn ForbiddenPanel() -> Element {
  rsx! {
      section { class: "min-h-screen flex items-center justify-center bg-white dark:bg-slate-950",
          div { class: "max-w-md text-center px-4",
              div { class: "text-6xl mb-4", "🔒" }
              h1 { class: "text-2xl font-bold text-slate-900 dark:text-white mb-2",
                  "403 / 需要管理员权限"
              }
              p { class: "text-sm text-slate-500 dark:text-slate-400",
                  "你当前的账号没有访问后台的权限。如果你确认应当拥有该权限，请联系站点管理员或在数据库中将 role 调整为 admin。"
              }
              a { href: "/", class: button_class(ButtonVariant::Primary, ButtonSize::Md, UiDensity::Comfortable, "mt-6"),
                  "返回首页"
              }
          }
      }
  }
}

/// 管理后台公共外壳（侧边导航 + 内容区）。app 组合根可复用此 chrome
/// 渲染跨模块管理页（如课程权益）而无需让 admin 模块反向依赖业务模块。
#[component]
pub fn AdminShell(active: String, children: Element) -> Element {
  rsx! {
      section { class: "min-h-screen bg-slate-50 dark:bg-slate-950",
          div { class: "max-w-7xl mx-auto flex",
              aside { class: "shrink-0 w-56 border-r border-slate-200 dark:border-slate-800 px-4 py-6 sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto bg-white dark:bg-slate-950",
                  h2 { class: "text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-slate-400 mb-4 px-2",
                      "管理后台"
                  }
                  nav { class: "space-y-1",
                      AdminNavLink { href: "/admin", label: "概览".to_string(), key_id: "dashboard".to_string(), active: active.clone() }
                      AdminNavLink { href: "/admin/users", label: "用户".to_string(), key_id: "users".to_string(), active: active.clone() }
                      AdminNavLink { href: "/admin/comments", label: "评论".to_string(), key_id: "comments".to_string(), active: active.clone() }
                      AdminNavLink { href: "/admin/topics", label: "话题".to_string(), key_id: "topics".to_string(), active: active.clone() }
                      AdminNavLink { href: "/admin/moderation", label: "审核".to_string(), key_id: "moderation".to_string(), active: active.clone() }
                      AdminNavLink { href: "/admin/moderation/settings", label: "审核设置".to_string(), key_id: "moderation-settings".to_string(), active: active.clone() }
                      AdminNavLink { href: "/admin/entitlements", label: "课程权益".to_string(), key_id: "entitlements".to_string(), active: active.clone() }
                  }
              }
              div { class: "flex-1 min-w-0 px-6 lg:px-10 py-8",
                  DensityProvider { density: UiDensity::Compact, {children} }
              }
          }
      }
  }
}

#[component]
fn AdminNavLink(href: String, label: String, key_id: String, active: String) -> Element {
  let is_active = key_id == active;
  let class = if is_active {
    "block px-3 py-2 rounded-lg text-sm font-semibold bg-primary/10 text-primary"
  } else {
    "block px-3 py-2 rounded-lg text-sm font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
  };
  rsx! {
      a { href: "{href}", class: "{class}", "{label}" }
  }
}

#[component]
fn Loading() -> Element {
  rsx! {
      div { class: "flex items-center justify-center py-20",
          Spinner { size: SpinnerSize::Lg, class: "border-t-primary" }
      }
  }
}

// =============================================================
// /admin Dashboard
// =============================================================

#[component]
pub fn AdminDashboardPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  // 重构 B6 评估：admin 后台全部保留 use_resource，**不** 迁移到 use_server_future。
  // 理由：后台鉴权（非 admin 渲染 403）、纯交互管理面板，不需 SEO / 首屏预渲染，
  // 且不应被公共缓存。
  let res = use_resource(|| async move { admin_overview().await.ok() });
  let overview = res.read().as_ref().cloned().flatten();

  rsx! {
      AdminShell { active: "dashboard".to_string(),
          h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-6", "概览" }

          match overview {
              None => rsx! { Loading {} },
              Some(data) => rsx! {
                  div { class: "grid grid-cols-2 md:grid-cols-3 gap-4 mb-8",
                      StatCard { label: "用户".to_string(), value: data.user_count, icon: "👥".to_string() }
                      StatCard { label: "管理员".to_string(), value: data.admin_count, icon: "🛡️".to_string() }
                      StatCard { label: "评论".to_string(), value: data.comment_count, icon: "💬".to_string() }
                      StatCard { label: "话题".to_string(), value: data.topic_count, icon: "📌".to_string() }
                      StatCard { label: "回复".to_string(), value: data.reply_count, icon: "↪️".to_string() }
                      StatCard { label: "标注".to_string(), value: data.annotation_count, icon: "✏️".to_string() }
                  }

                  div { class: "rounded-xl border border-slate-200 dark:border-slate-800 p-6 bg-white dark:bg-slate-900/40",
                      h2 { class: "text-lg font-bold text-slate-900 dark:text-white mb-2", "下一步" }
                      ul { class: "list-disc pl-5 text-sm text-slate-600 dark:text-slate-400 space-y-1",
                          li { "用户页:调整角色" }
                          li { "评论页:删除违规评论" }
                          li { "话题页:管理论坛内容" }
                          li { "审核页:复核被标记的内容" }
                      }
                  }
              },
          }
      }
  }
}

#[component]
fn StatCard(label: String, value: i64, icon: String) -> Element {
  rsx! {
      div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 p-5",
          div { class: "flex items-center justify-between mb-2",
              span { class: "text-xs uppercase tracking-wider text-slate-500 dark:text-slate-400", "{label}" }
              span { class: "text-2xl", "{icon}" }
          }
          div { class: "text-3xl font-extrabold text-slate-900 dark:text-white", "{value}" }
      }
  }
}

// =============================================================
// /admin/users
// =============================================================

#[component]
pub fn AdminUsersPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  let mut page = use_signal(|| 0u32);
  let mut error = use_signal::<Option<String>>(|| None);
  let mut bump = use_signal(|| 0u32); // 用于触发数据刷新

  let res = use_resource(move || {
    let p = page();
    let _ = bump();
    async move { admin_list_users(Some(p)).await.ok() }
  });
  let data = res.read().as_ref().cloned().flatten();

  let total = data.as_ref().map(|d| d.total).unwrap_or(0);
  let total_pages = compute_total_pages(total, ADMIN_PAGE_SIZE);

  rsx! {
      AdminShell { active: "users".to_string(),
          div { class: "flex items-center justify-between mb-6",
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white", "用户" }
              span { class: "text-sm text-slate-500", "共 {total} 个用户" }
          }

          if let Some(err) = error() {
              div { class: "mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400 rounded-lg",
                  "{err}"
              }
          }

          match data {
              None => rsx! { Loading {} },
              Some(p) if p.items.is_empty() => rsx! {
                  div { class: "py-16 text-center text-slate-500", "没有用户" }
              },
              Some(p) => rsx! {
                  div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 overflow-hidden",
                      div { class: "divide-y divide-slate-100 dark:divide-slate-800",
                          for u in p.items.iter() {
                              UserRow {
                                  key: "{u.id}",
                                  user: u.clone(),
                                  on_role_changed: move |msg: Result<(), String>| {
                                      match msg {
                                          Ok(()) => {
                                              error.set(None);
                                              bump.with_mut(|n| *n = n.wrapping_add(1));
                                          }
                                          Err(e) => error.set(Some(e)),
                                      }
                                  }
                              }
                          }
                      }
                  }
                  Pagination { page: page(), total_pages, on_change: move |new_page: u32| page.set(new_page) }
              },
          }
      }
  }
}

#[component]
fn UserRow(user: AdminUserRow, on_role_changed: EventHandler<Result<(), String>>) -> Element {
  let mut submitting = use_signal(|| false);
  let user_id = user.id;

  rsx! {
      div { class: "flex items-center gap-4 px-5 py-3",
          // Avatar
          div { class: "flex-none",
              if let Some(ref a) = user.avatar_url {
                  img { src: "{a}", class: "w-10 h-10 rounded-full object-cover", alt: "{user.nickname}" }
              } else {
                  div { class: "w-10 h-10 rounded-full bg-primary text-primary-foreground flex items-center justify-center font-bold",
                      "{user.nickname.chars().next().unwrap_or('U')}"
                  }
              }
          }
          // Info
          div { class: "flex-1 min-w-0",
              div { class: "flex items-center gap-2 mb-0.5",
                  span { class: "font-semibold text-slate-900 dark:text-white", "{user.nickname}" }
                  span { class: "text-xs text-slate-400", "#{user.id}" }
              }
              div { class: "text-xs text-slate-500 dark:text-slate-400 flex items-center gap-2 flex-wrap",
                  span { "{user.created_at}" }
                  if !user.providers.is_empty() {
                      span { "·" }
                      for p in user.providers.iter() {
                          Badge { variant: BadgeVariant::Secondary, class: "uppercase tracking-wide font-medium",
                              "{p}"
                          }
                      }
                  }
              }
          }
          // Role select
          div { class: "shrink-0",
              NativeSelect {
                  class: "w-auto",
                  disabled: submitting(),
                  "aria-label": "角色",
                  on_value_change: move |new_role: String| {
                      let on_role_changed = on_role_changed;
                      spawn(async move {
                          submitting.set(true);
                          let res = admin_set_user_role(user_id, new_role).await;
                          match res {
                              Ok(_) => on_role_changed.call(Ok(())),
                              Err(e) => on_role_changed.call(Err(format!("更新失败: {}", e))),
                          }
                          submitting.set(false);
                      });
                  },
                  for r in ALL_ROLES.iter() {
                      NativeSelectOption { value: r.to_string(), selected: user.role == *r, "{r}" }
                  }
              }
          }
      }
  }
}

// =============================================================
// /admin/comments
// =============================================================

#[component]
pub fn AdminCommentsPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  let mut page = use_signal(|| 0u32);
  let mut error = use_signal::<Option<String>>(|| None);
  let mut bump = use_signal(|| 0u32);

  let res = use_resource(move || {
    let p = page();
    let _ = bump();
    async move { admin_list_comments(Some(p)).await.ok() }
  });
  let data = res.read().as_ref().cloned().flatten();
  let total = data.as_ref().map(|d| d.total).unwrap_or(0);
  let total_pages = compute_total_pages(total, ADMIN_PAGE_SIZE);

  rsx! {
      AdminShell { active: "comments".to_string(),
          div { class: "flex items-center justify-between mb-6",
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white", "评论" }
              span { class: "text-sm text-slate-500", "共 {total} 条" }
          }

          if let Some(err) = error() {
              div { class: "mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400 rounded-lg",
                  "{err}"
              }
          }

          match data {
              None => rsx! { Loading {} },
              Some(p) if p.items.is_empty() => rsx! {
                  div { class: "py-16 text-center text-slate-500", "没有评论" }
              },
              Some(p) => rsx! {
                  div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 overflow-hidden",
                      div { class: "divide-y divide-slate-100 dark:divide-slate-800",
                          for c in p.items.iter() {
                              CommentRow {
                                  key: "{c.id}",
                                  comment: c.clone(),
                                  on_deleted: move |msg: Result<(), String>| {
                                      match msg {
                                          Ok(()) => {
                                              error.set(None);
                                              bump.with_mut(|n| *n = n.wrapping_add(1));
                                          }
                                          Err(e) => error.set(Some(e)),
                                      }
                                  }
                              }
                          }
                      }
                  }
                  Pagination { page: page(), total_pages, on_change: move |new_page: u32| page.set(new_page) }
              },
          }
      }
  }
}

#[component]
fn CommentRow(comment: AdminCommentRow, on_deleted: EventHandler<Result<(), String>>) -> Element {
  let mut submitting = use_signal(|| false);
  let id = comment.id;
  rsx! {
      div { class: "px-5 py-3 flex items-start gap-4",
          div { class: "flex-1 min-w-0",
              div { class: "flex items-center gap-2 mb-1 text-xs text-slate-500 flex-wrap",
                  span { class: "font-semibold text-slate-700 dark:text-slate-200", "{comment.author}" }
                  span { "·" }
                  a { href: "/blog/{comment.blog_id}", class: "text-primary hover:underline truncate max-w-xs",
                      "{comment.blog_id}"
                  }
                  span { "·" }
                  span { "{comment.created_at}" }
                  span { "·" }
                  span { class: "text-slate-400", "#{comment.id}" }
              }
              div { class: "text-sm text-slate-700 dark:text-slate-200 whitespace-pre-wrap break-words line-clamp-3",
                  "{comment.content}"
              }
          }
          div { class: "shrink-0",
              ConfirmDelete {
                  title: "删除这条评论？".to_string(),
                  description: format!("评论 #{} 将被永久删除，无法恢复。", comment.id),
                  busy: submitting(),
                  on_confirm: move |_| {
                      let on_deleted = on_deleted;
                      spawn(async move {
                          submitting.set(true);
                          match admin_delete_comment(id).await {
                              Ok(()) => on_deleted.call(Ok(())),
                              Err(e) => on_deleted.call(Err(format!("删除失败: {}", e))),
                          }
                          submitting.set(false);
                      });
                  },
              }
          }
      }
  }
}

// =============================================================
// /admin/topics
// =============================================================

#[component]
pub fn AdminTopicsPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  let mut page = use_signal(|| 0u32);
  let mut error = use_signal::<Option<String>>(|| None);
  let mut bump = use_signal(|| 0u32);

  let res = use_resource(move || {
    let p = page();
    let _ = bump();
    async move { admin_list_topics(Some(p)).await.ok() }
  });
  let data = res.read().as_ref().cloned().flatten();
  let total = data.as_ref().map(|d| d.total).unwrap_or(0);
  let total_pages = compute_total_pages(total, ADMIN_PAGE_SIZE);

  rsx! {
      AdminShell { active: "topics".to_string(),
          div { class: "flex items-center justify-between mb-6",
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white", "话题" }
              span { class: "text-sm text-slate-500", "共 {total} 个" }
          }

          if let Some(err) = error() {
              div { class: "mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400 rounded-lg",
                  "{err}"
              }
          }

          match data {
              None => rsx! { Loading {} },
              Some(p) if p.items.is_empty() => rsx! {
                  div { class: "py-16 text-center text-slate-500", "没有话题" }
              },
              Some(p) => rsx! {
                  div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 overflow-hidden",
                      div { class: "divide-y divide-slate-100 dark:divide-slate-800",
                          for t in p.items.iter() {
                              TopicRow {
                                  key: "{t.id}",
                                  topic: t.clone(),
                                  on_deleted: move |msg: Result<(), String>| {
                                      match msg {
                                          Ok(()) => {
                                              error.set(None);
                                              bump.with_mut(|n| *n = n.wrapping_add(1));
                                          }
                                          Err(e) => error.set(Some(e)),
                                      }
                                  }
                              }
                          }
                      }
                  }
                  Pagination { page: page(), total_pages, on_change: move |new_page: u32| page.set(new_page) }
              },
          }

          div { class: "mt-8 rounded-xl border border-amber-200 dark:border-amber-800 bg-amber-50/50 dark:bg-amber-900/20 p-4 text-sm text-amber-800 dark:text-amber-300",
              "提示:删除话题会级联删除所有回复。如需仅删除单条回复,请到话题详情页用管理员账号操作 `admin_delete_reply` 接口(下一期 PR 提供前端入口)。"
          }
      }
  }
}

#[component]
fn TopicRow(topic: AdminTopicRow, on_deleted: EventHandler<Result<(), String>>) -> Element {
  let mut submitting = use_signal(|| false);
  let id = topic.id;
  let when = topic.last_reply_at.clone().unwrap_or_else(|| topic.created_at.clone());
  rsx! {
      div { class: "px-5 py-3 flex items-start gap-4",
          div { class: "flex-1 min-w-0",
              div { class: "flex items-center gap-2 mb-1 text-xs text-slate-500 flex-wrap",
                  Badge { variant: BadgeVariant::Secondary, "#{topic.tag}" }
                  span { "·" }
                  span { class: "font-semibold text-slate-700 dark:text-slate-200", "{topic.author}" }
                  span { "·" }
                  span { "{when}" }
                  span { "·" }
                  span { class: "text-slate-400", "#{topic.id}" }
                  span { "·" }
                  span { "{topic.reply_count} 回复" }
              }
              a { href: "/topics/{topic.id}",
                  class: "block text-sm font-semibold text-slate-900 dark:text-white truncate hover:text-primary",
                  "{topic.title}"
              }
          }
          div { class: "shrink-0",
              ConfirmDelete {
                  title: "删除这个话题？".to_string(),
                  description: format!("「{}」及其 {} 条回复将被永久删除，无法恢复。", topic.title, topic.reply_count),
                  busy: submitting(),
                  on_confirm: move |_| {
                      let on_deleted = on_deleted;
                      spawn(async move {
                          submitting.set(true);
                          match admin_delete_topic(id).await {
                              Ok(()) => on_deleted.call(Ok(())),
                              Err(e) => on_deleted.call(Err(format!("删除失败: {}", e))),
                          }
                          submitting.set(false);
                      });
                  },
              }
          }
      }
  }
}

/// 删除类按钮：先弹确认框，确认后才调用 `on_confirm`。删除不可恢复。
/// 默认是表格行内的 ghost「删除」；审核队列用实心红色按钮并换文案。
#[component]
fn ConfirmDelete(
  title: String,
  description: String,
  busy: bool,
  on_confirm: EventHandler<()>,
  #[props(default = "删除".to_string())] label: String,
  #[props(default = ButtonVariant::Ghost)] variant: ButtonVariant,
  /// 与 `busy` 不同：只禁用、不显示「...」（如批量操作未选中任何行）。
  #[props(default)]
  disabled: bool,
) -> Element {
  let density = use_density();
  let extra =
    if variant == ButtonVariant::Ghost { "text-destructive hover:bg-destructive/10" } else { "" };
  rsx! {
      AlertDialog {
          AlertDialogTrigger {
              class: button_class(variant, ButtonSize::Sm, density, extra),
              disabled: busy || disabled,
              if busy { "..." } else { "{label}" }
          }
          AlertDialogOverlay {}
          AlertDialogContent {
              AlertDialogHeader {
                  AlertDialogTitle { "{title}" }
                  AlertDialogDescription { "{description}" }
              }
              AlertDialogFooter {
                  AlertDialogCancel { "取消" }
                  AlertDialogAction {
                      variant: AlertDialogActionVariant::Destructive,
                      onclick: move |_| on_confirm.call(()),
                      "{label}"
                  }
              }
          }
      }
  }
}

/// 暴露给 reply 删除入口（暂未在 UI 调用，但保留以便后续 PR 接入并保证函数被使用）
#[allow(dead_code)]
async fn delete_reply_from_admin(id: i32) -> Result<(), String> {
  admin_delete_reply(id).await.map_err(|e| e.to_string())
}

// =============================================================
// 分页控件
// =============================================================

fn compute_total_pages(total: i64, page_size: u32) -> u32 {
  if total <= 0 || page_size == 0 {
    return 1;
  }
  let ps = page_size as i64;
  let n = (total + ps - 1) / ps;
  if n < 1 {
    1
  } else if n > u32::MAX as i64 {
    u32::MAX
  } else {
    n as u32
  }
}

#[component]
fn Pagination(page: u32, total_pages: u32, on_change: EventHandler<u32>) -> Element {
  if total_pages <= 1 {
    return rsx! { div {} };
  }
  let prev_disabled = page == 0;
  let next_disabled = page + 1 >= total_pages;
  rsx! {
      div { class: "mt-6 flex items-center justify-center gap-2",
          Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              disabled: prev_disabled,
              onclick: move |_| {
                  if page > 0 { on_change.call(page - 1); }
              },
              "上一页"
          }
          span { class: "text-sm text-slate-500", "第 {page + 1} / {total_pages} 页" }
          Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              disabled: next_disabled,
              onclick: move |_| {
                  if page + 1 < total_pages { on_change.call(page + 1); }
              },
              "下一页"
          }
      }
  }
}

// =============================================================
// /admin/moderation （Phase 4.5）
// =============================================================

#[component]
pub fn AdminModerationPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  let mut filter = use_signal(|| "pending".to_string()); // pending / approved / rejected / all
  let mut error = use_signal::<Option<String>>(|| None);
  let mut bump = use_signal(|| 0u32);
  // 批量选择：保存被勾选的队列行 id。
  let mut selected = use_signal(std::collections::HashSet::<i64>::new);
  let mut bulk_busy = use_signal(|| false);

  let res = use_resource(move || {
    let f = filter();
    let _ = bump();
    async move {
      let arg = if f == "all" { None } else { Some(f) };
      admin_list_moderation_queue(arg, Some(200)).await.ok()
    }
  });
  let rows: Vec<ModerationQueueRow> = res.read().as_ref().cloned().flatten().unwrap_or_default();
  let pending_ids: Vec<i64> = rows.iter().filter(|r| r.status == "pending").map(|r| r.id).collect();
  let selected_count = selected().len();
  let all_pending_selected =
    !pending_ids.is_empty() && pending_ids.iter().all(|id| selected().contains(id));
  let some_pending_selected =
    !all_pending_selected && pending_ids.iter().any(|id| selected().contains(id));

  // 批量操作完成后的统一收尾：清空选择 + 刷新列表。
  let mut finish_bulk = move |result: Result<u64, String>| {
    bulk_busy.set(false);
    match result {
      Ok(_) => {
        selected.with_mut(|s| s.clear());
        error.set(None);
        bump.with_mut(|n| *n = n.wrapping_add(1));
      }
      Err(e) => error.set(Some(e)),
    }
  };

  rsx! {
      AdminShell { active: "moderation".to_string(),
          div { class: "flex items-center justify-between mb-6",
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white", "审核队列" }
              span { class: "text-sm text-slate-500", "共 {rows.len()} 条" }
          }

          Tabs { value: filter(), on_value_change: move |v: String| filter.set(v),
              TabsList { class: "mb-4",
                  TabsTrigger { value: "pending", "待复核" }
                  TabsTrigger { value: "approved", "已通过" }
                  TabsTrigger { value: "rejected", "已拒绝" }
                  TabsTrigger { value: "all", "全部" }
              }
              // 四个页签共用一张表，按当前页签取数，所以只挂当前页签的面板。
              TabsContent { value: filter(),

                  if let Some(err) = error() {
                      div { class: "mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400 rounded-lg",
                          "{err}"
                      }
                  }

                  // 批量操作栏：仅在有待复核行时显示。
                  if !pending_ids.is_empty() {
                      div { class: "flex items-center gap-3 mb-4 px-4 py-2 rounded-lg bg-slate-50 dark:bg-slate-800/60 text-sm flex-wrap",
                          label { class: "flex items-center gap-2 cursor-pointer",
                              Checkbox {
                                  checked: all_pending_selected,
                                  indeterminate: some_pending_selected,
                                  on_checked_change: {
                                      let pending_ids = pending_ids.clone();
                                      move |select_all: bool| {
                                          selected.with_mut(|s| {
                                              for id in &pending_ids {
                                                  if select_all { s.insert(*id); } else { s.remove(id); }
                                              }
                                          });
                                      }
                                  },
                              }
                              span { class: "text-slate-600 dark:text-slate-300", "全选待复核" }
                          }
                          span { class: "text-slate-500", "已选 {selected_count} 条" }
                          div { class: "flex-1" }
                          Button {
                              size: ButtonSize::Sm,
                              class: "bg-success text-success-foreground hover:bg-success/90",
                              disabled: selected_count == 0 || bulk_busy(),
                              onclick: move |_| {
                                  let ids: Vec<i64> = selected().iter().copied().collect();
                                  spawn(async move {
                                      bulk_busy.set(true);
                                      match admin_bulk_approve_moderation(ids).await {
                                          Ok(n) => finish_bulk(Ok(n)),
                                          Err(e) => finish_bulk(Err(format!("批量通过失败: {}", e))),
                                      }
                                  });
                              },
                              "批量通过"
                          }
                          ConfirmDelete {
                              title: format!("批量拒绝 {selected_count} 条？"),
                              description: format!("选中的 {selected_count} 条内容将被永久删除，无法恢复。"),
                              label: "批量拒绝（删除内容）".to_string(),
                              variant: ButtonVariant::Destructive,
                              busy: bulk_busy(),
                              disabled: selected_count == 0,
                              on_confirm: move |_| {
                                  let ids: Vec<i64> = selected().iter().copied().collect();
                                  spawn(async move {
                                      bulk_busy.set(true);
                                      match admin_bulk_reject_moderation(ids).await {
                                          Ok(n) => finish_bulk(Ok(n)),
                                          Err(e) => finish_bulk(Err(format!("批量拒绝失败: {}", e))),
                                      }
                                  });
                              },
                          }
                      }
                  }

                  match res.read().as_ref() {
                      None => rsx! { Loading {} },
                      Some(_) if rows.is_empty() => rsx! {
                          div { class: "py-16 text-center text-slate-500", "暂无记录" }
                      },
                      Some(_) => rsx! {
                          div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 overflow-hidden",
                              div { class: "divide-y divide-slate-100 dark:divide-slate-800",
                                  for r in rows.iter() {
                                      ModerationQueueRowView {
                                          key: "{r.id}",
                                          row: r.clone(),
                                          selected: selected().contains(&r.id),
                                          on_toggle: move |id: i64| {
                                              selected.with_mut(|s| {
                                                  if !s.insert(id) {
                                                      s.remove(&id);
                                                  }
                                              });
                                          },
                                          on_done: move |msg: Result<(), String>| {
                                              match msg {
                                                  Ok(()) => {
                                                      error.set(None);
                                                      bump.with_mut(|n| *n = n.wrapping_add(1));
                                                  }
                                                  Err(e) => error.set(Some(e)),
                                              }
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
}

#[component]
fn ModerationQueueRowView(
  row: ModerationQueueRow,
  selected: bool,
  on_toggle: EventHandler<i64>,
  on_done: EventHandler<Result<(), String>>,
) -> Element {
  let mut submitting = use_signal(|| false);
  let id = row.id;

  // 状态徽章颜色
  let (status_variant, status_label) = match row.status.as_str() {
    "pending" => (BadgeVariant::Warning, "待复核"),
    "approved" => (BadgeVariant::Success, "已通过"),
    "rejected" => (BadgeVariant::Destructive, "已拒绝"),
    _ => (BadgeVariant::Secondary, row.status.as_str()),
  };

  let kind_label = match row.kind.as_str() {
    "comment" => "评论",
    "topic" => "话题",
    "reply" => "回复",
    "annotation" => "标注",
    other => other,
  };
  let author =
    row.user_nickname.clone().unwrap_or_else(|| format!("用户#{}", row.user_id.unwrap_or(0)));
  let score_pct = ((row.score * 100.0).round() as i32).clamp(0, 100);

  let is_pending = row.status == "pending";

  rsx! {
      div { class: "px-5 py-4",
          // 头部：勾选 / 状态徽章 / 类型 / 路径 / 作者历史 / 时间 / 评分
          div { class: "flex items-center gap-2 mb-2 text-xs flex-wrap",
              if is_pending {
                  Checkbox {
                      checked: selected,
                      on_checked_change: move |_| on_toggle.call(id),
                      "aria-label": "选择此条",
                  }
              }
              Badge { variant: status_variant, "{status_label}" }
              Badge { variant: BadgeVariant::Secondary, "{kind_label}" }
              span { class: "text-slate-500 truncate max-w-xs", "{row.ref_path}" }
              span { class: "text-slate-400", "·" }
              span { class: "text-slate-500", "{author}" }
              // 作者历史违规：累计命中 > 1 时提示，已被拒绝（确认违规）单独红标
              if row.user_history_total > 1 {
                  Badge { variant: BadgeVariant::Warning, "历史 {row.user_history_total} 次命中" }
              }
              if row.user_history_rejected > 0 {
                  Badge { variant: BadgeVariant::Destructive, "{row.user_history_rejected} 次确认违规" }
              }
              span { class: "text-slate-400", "·" }
              span { class: "text-slate-500", "{row.created_at}" }
              span { class: "text-slate-400", "·" }
              span { class: "font-mono text-slate-600 dark:text-slate-300", "score {score_pct}%" }
          }

          // 理由
          if !row.reason.is_empty() {
              div { class: "mb-2 text-sm text-slate-600 dark:text-slate-300",
                  span { class: "text-xs uppercase text-slate-400 mr-2", "理由" }
                  span { "{row.reason}" }
              }
          }

          // 内容
          div { class: "text-sm text-slate-800 dark:text-slate-100 whitespace-pre-wrap break-words border border-slate-100 dark:border-slate-800 rounded-md px-3 py-2 bg-slate-50/60 dark:bg-slate-900/60",
              "{row.content}"
          }

          // 图片
          if !row.images.is_empty() {
              div { class: "flex flex-wrap gap-2 mt-2",
                  for url in row.images.iter() {
                      img {
                          src: "{url}",
                          class: "h-20 w-20 object-cover rounded-md border border-slate-200 dark:border-slate-700",
                          alt: ""
                      }
                  }
              }
          }

          // 操作 + 复核者信息
          div { class: "mt-3 flex items-center justify-between gap-3 flex-wrap",
              div { class: "text-xs text-slate-500",
                  if let Some(ref reviewer) = row.reviewer_nickname {
                      if let Some(ref at) = row.reviewed_at {
                          span { "复核者：{reviewer} · {at}" }
                      } else {
                          span { "复核者：{reviewer}" }
                      }
                  }
              }
              if is_pending {
                  div { class: "flex gap-2",
                      Button {
                          size: ButtonSize::Sm,
                          class: "bg-success text-success-foreground hover:bg-success/90",
                          disabled: submitting(),
                          onclick: move |_| {
                              let on_done = on_done;
                              spawn(async move {
                                  submitting.set(true);
                                  match admin_approve_moderation(id).await {
                                      Ok(()) => on_done.call(Ok(())),
                                      Err(e) => on_done.call(Err(format!("通过失败: {}", e))),
                                  }
                                  submitting.set(false);
                              });
                          },
                          "通过"
                      }
                      ConfirmDelete {
                          title: "拒绝并删除这条内容？".to_string(),
                          description: format!("{author} 的这条{kind_label}将被永久删除，无法恢复。"),
                          label: "拒绝（删除内容）".to_string(),
                          variant: ButtonVariant::Destructive,
                          busy: submitting(),
                          on_confirm: move |_| {
                              let on_done = on_done;
                              spawn(async move {
                                  submitting.set(true);
                                  match admin_reject_moderation(id).await {
                                      Ok(()) => on_done.call(Ok(())),
                                      Err(e) => on_done.call(Err(format!("拒绝失败: {}", e))),
                                  }
                                  submitting.set(false);
                              });
                          },
                      }
                  }
              }
          }
      }
  }
}

// =============================================================
// /admin/moderation/settings — Phase 308：审核阈值在线编辑
// =============================================================

/// 把 `Option<f32>` 转成输入框文本（None / NaN → 空串）。
fn opt_f32_to_input(v: Option<f32>) -> String {
  match v {
    Some(x) if x.is_finite() => format!("{}", x),
    _ => String::new(),
  }
}

/// 把输入框文本反解为 `Option<f32>`：空串 → None；非合法浮点 → 返回 Err。
fn input_to_opt_f32(raw: &str) -> Result<Option<f32>, String> {
  let t = raw.trim();
  if t.is_empty() {
    return Ok(None);
  }
  t.parse::<f32>().map(Some).map_err(|_| format!("非合法数字：{}", t))
}

/// 按行拆分 textarea 文本，去重空行 + trim，得到 Vec<String>。
fn lines_to_vec(text: &str) -> Vec<String> {
  text.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Vec<String> 拼回多行文本，便于初始化 textarea。
fn vec_to_lines(items: &[String]) -> String {
  items.join("\n")
}

#[component]
pub fn AdminModerationSettingsPage() -> Element {
  if !is_current_user_admin() {
    return rsx! { ForbiddenPanel {} };
  }

  // 强制刷新计数：保存成功后 bump 让 use_resource 重跑。
  let mut bump = use_signal(|| 0u32);
  let res = use_resource(move || {
    let _ = bump();
    async move { admin_get_moderation_settings().await.ok() }
  });

  // 当前从服务端拿到的设置（None = 加载中 / 错误）
  let current: Option<ModerationSettings> = res.read().as_ref().cloned().flatten();

  // 表单本地状态：初始化时填入 current 的值
  let mut enabled = use_signal(|| false);
  let mut flag_text = use_signal(String::new);
  let mut block_text = use_signal(String::new);
  let mut llm_review = use_signal(|| false);
  let mut on_llm_failure = use_signal(LlmFailureAction::default);
  let mut blocklist_text = use_signal(String::new);
  // 一次性把 server 值灌进表单（仅当本地表单 untouched / current 刚到达）
  let mut form_inited = use_signal(|| false);
  if !form_inited() {
    if let Some(s) = &current {
      enabled.set(s.enabled);
      let th = s.thresholds.as_ref();
      flag_text.set(opt_f32_to_input(th.and_then(|t| t.flag_above)));
      block_text.set(opt_f32_to_input(th.and_then(|t| t.block_above)));
      llm_review.set(s.llm_review);
      on_llm_failure.set(s.on_llm_failure);
      blocklist_text.set(vec_to_lines(&s.url_blocklist));
      form_inited.set(true);
    }
  }

  let mut saving = use_signal(|| false);
  let mut error = use_signal::<Option<String>>(|| None);
  let mut saved_ok = use_signal(|| false);

  let mut save = move |_| {
    error.set(None);
    saved_ok.set(false);
    // 客户端先做与 server 端一致的解析 + 校验，给即时反馈
    let flag_parsed = match input_to_opt_f32(&flag_text()) {
      Ok(v) => v,
      Err(e) => {
        error.set(Some(format!("flag_above 解析失败：{}", e)));
        return;
      }
    };
    let block_parsed = match input_to_opt_f32(&block_text()) {
      Ok(v) => v,
      Err(e) => {
        error.set(Some(format!("block_above 解析失败：{}", e)));
        return;
      }
    };
    let thresholds = match (flag_parsed, block_parsed) {
      (None, None) => None,
      (f, b) => Some(ModerationThresholdsConfig { flag_above: f, block_above: b }),
    };
    let settings = ModerationSettings {
      enabled: enabled(),
      llm_review: llm_review(),
      on_llm_failure: on_llm_failure(),
      thresholds,
      url_blocklist: lines_to_vec(&blocklist_text()),
    };

    spawn(async move {
      saving.set(true);
      match admin_set_moderation_settings(settings).await {
        Ok(()) => {
          saved_ok.set(true);
          // 服务端值可能因 normalize 略微变化（trim 等），重拉一次。
          bump.with_mut(|n| *n = n.wrapping_add(1));
          form_inited.set(false); // 让初始化逻辑再灌一次
        }
        Err(e) => error.set(Some(format!("保存失败：{}", e))),
      }
      saving.set(false);
    });
  };

  let label_cls = "block text-sm font-semibold text-slate-700 dark:text-slate-300 mb-1";
  let help_cls = "text-xs text-slate-500 dark:text-slate-400 mt-1";

  rsx! {
      AdminShell { active: "moderation-settings".to_string(),
          h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-2",
              "审核设置"
          }
          p { class: "text-sm text-slate-500 dark:text-slate-400 mb-6",
              "改动会原子写回 assets/site.json 并立即热重载审核流水线（无需重启进程）。"
          }

          if res.read().is_none() {
              Loading {}
          } else if current.is_none() {
              div { class: "rounded-lg border border-red-300 bg-red-50 text-red-800 p-4",
                  "加载审核设置失败：请检查 server 日志。"
              }
          } else {
              form {
                  class: "space-y-6 max-w-2xl",
                  onsubmit: move |evt| { evt.prevent_default(); save(()); },

                  // enabled 总开关
                  div { class: "flex items-center gap-3 p-4 rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40",
                      Switch {
                          id: "moderation-enabled",
                          checked: enabled(),
                          on_checked_change: move |on| enabled.set(on),
                      }
                      label { r#for: "moderation-enabled", class: "text-sm font-semibold text-slate-800 dark:text-slate-200",
                          "启用内容审核流水线"
                      }
                      span { class: "text-xs text-slate-500 dark:text-slate-400 ml-auto",
                          "默认关闭。关闭 = 所有提交直接 Allow，零开销。"
                      }
                  }

                  // 阈值
                  div { class: "grid grid-cols-1 sm:grid-cols-2 gap-4",
                      div {
                          label { r#for: "flag-above", class: "{label_cls}", "flag_above (0.0–1.0)" }
                          Input {
                              id: "flag-above",
                              r#type: "number",
                              step: "0.01",
                              min: "0",
                              max: "1",
                              value: flag_text(),
                              on_value_change: move |v: String| flag_text.set(v),
                          }
                          p { class: "{help_cls}",
                              "评分 ≥ 此值即标记 Flag（入审核队列）。留空 = 用默认 0.5。"
                          }
                      }
                      div {
                          label { r#for: "block-above", class: "{label_cls}", "block_above (0.0–1.0)" }
                          Input {
                              id: "block-above",
                              r#type: "number",
                              step: "0.01",
                              min: "0",
                              max: "1",
                              value: block_text(),
                              on_value_change: move |v: String| block_text.set(v),
                          }
                          p { class: "{help_cls}",
                              "评分 ≥ 此值即直接拒绝（Block）。留空 = 用默认 0.9。需 ≥ flag_above。"
                          }
                      }
                  }

                  // LLM 审核开关
                  div { class: "flex items-center gap-3 p-4 rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40",
                      Switch {
                          id: "moderation-llm-review",
                          checked: llm_review(),
                          on_checked_change: move |on| llm_review.set(on),
                      }
                      label { r#for: "moderation-llm-review", class: "text-sm font-semibold text-slate-800 dark:text-slate-200",
                          "LLM 审核"
                      }
                      span { class: "text-xs text-slate-500 dark:text-slate-400 ml-auto",
                          "需配置 OPENAI_LLM_* 或 ANTHROPIC_LLM_* 环境变量，未配置时跳过。"
                      }
                  }

                  // LLM 审核失败时的处理
                  div {
                      p { class: "{label_cls}", "LLM 审核失败时" }
                      NativeSelect {
                          class: "w-auto",
                          "aria-label": "LLM 审核失败时",
                          on_value_change: move |v: String| {
                              on_llm_failure.set(if v == "reject" { LlmFailureAction::Reject } else { LlmFailureAction::Review });
                          },
                          NativeSelectOption { value: "review", selected: on_llm_failure() == LlmFailureAction::Review, "送人工复核（照常发布，进审核队列）" }
                          NativeSelectOption { value: "reject", selected: on_llm_failure() == LlmFailureAction::Reject, "拒绝提交（提示用户稍后再试）" }
                      }
                      p { class: "{help_cls}",
                          "模型调用出错或回复无法解析时生效。不提供放行：否则把审核打挂就能绕过。"
                      }
                  }

                  // url_blocklist
                  div {
                      label { r#for: "blocklist", class: "{label_cls}", "URL 域名黑名单（一行一个）" }
                      Textarea {
                          id: "blocklist",
                          rows: "5",
                          class: "font-mono",
                          placeholder: "scam.com\n*.phishing.example",
                          value: blocklist_text(),
                          on_value_change: move |v: String| blocklist_text.set(v),
                      }
                      p { class: "{help_cls}",
                          "命中即 Block（score = 1.0），不走 LLM。支持通配 *.example.com；只填 host，勿带 https://。"
                      }
                  }

                  // 保存 + 状态
                  div { class: "flex items-center gap-3",
                      Button {
                          r#type: "submit",
                          disabled: saving(),
                          if saving() { "保存中…" } else { "保存并热重载" }
                      }
                      if saved_ok() {
                          span { class: "text-sm text-green-600 dark:text-green-400 font-medium",
                              "✓ 已保存并重载"
                          }
                      }
                      if let Some(e) = error() {
                          span { class: "text-sm text-red-600 dark:text-red-400", "{e}" }
                      }
                  }
              }
          }
      }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn total_pages_zero_or_negative() {
    assert_eq!(compute_total_pages(0, 50), 1);
    assert_eq!(compute_total_pages(-5, 50), 1);
  }

  #[test]
  fn total_pages_basic() {
    assert_eq!(compute_total_pages(50, 50), 1);
    assert_eq!(compute_total_pages(51, 50), 2);
    assert_eq!(compute_total_pages(99, 50), 2);
    assert_eq!(compute_total_pages(100, 50), 2);
    assert_eq!(compute_total_pages(101, 50), 3);
  }

  #[test]
  fn total_pages_zero_size_safe() {
    // 防御:page_size 为 0 时返回 1,避免除零
    assert_eq!(compute_total_pages(123, 0), 1);
  }

  // ── Phase 308：moderation 表单 helpers ──

  #[test]
  fn opt_f32_to_input_handles_none_and_finite() {
    assert_eq!(opt_f32_to_input(None), "");
    assert_eq!(opt_f32_to_input(Some(0.5)), "0.5");
    assert_eq!(opt_f32_to_input(Some(0.0)), "0");
    // NaN / Inf 都视作 None：避免输入框出现 NaN 文本
    assert_eq!(opt_f32_to_input(Some(f32::NAN)), "");
    assert_eq!(opt_f32_to_input(Some(f32::INFINITY)), "");
  }

  #[test]
  fn input_to_opt_f32_handles_empty_and_valid() {
    assert_eq!(input_to_opt_f32("").unwrap(), None);
    assert_eq!(input_to_opt_f32("   ").unwrap(), None);
    assert_eq!(input_to_opt_f32("0.5").unwrap(), Some(0.5));
    assert_eq!(input_to_opt_f32("  0.9  ").unwrap(), Some(0.9));
  }

  #[test]
  fn input_to_opt_f32_rejects_garbage() {
    assert!(input_to_opt_f32("abc").is_err());
    assert!(input_to_opt_f32("0.5x").is_err());
  }

  #[test]
  fn lines_to_vec_strips_blank_and_whitespace() {
    let raw = "  scam.com\n\n*.evil.example\n   \nfoo.bar  \n";
    assert_eq!(
      lines_to_vec(raw),
      vec!["scam.com".to_string(), "*.evil.example".to_string(), "foo.bar".to_string()]
    );
  }

  #[test]
  fn lines_to_vec_empty_input_returns_empty() {
    assert!(lines_to_vec("").is_empty());
    assert!(lines_to_vec("   \n\n   ").is_empty());
  }

  #[test]
  fn vec_to_lines_round_trips_with_lines_to_vec() {
    let items = vec!["a.com".to_string(), "b.com".to_string()];
    assert_eq!(lines_to_vec(&vec_to_lines(&items)), items);
  }
}
