use crate::server::{
  create_topic, get_topic, list_my_topics, list_tags, list_topics, list_topics_by_ref, post_reply,
  NewTopicInput, Reply, TagSummary, TopicDetail, TopicRef, TopicSummary,
};
use dioxus::prelude::*;
use rustineverything_core::i18n::Language;
use rustineverything_core::session::SessionUser;
use rustineverything_widgets::Markdown;

// =============================================================
// 共享上下文 hooks（与 app crate 的 use_session_user / use_auth_modal 桥接）
// =============================================================

fn use_session_user_ctx() -> Option<Signal<Option<SessionUser>>> {
  try_use_context::<Signal<Option<SessionUser>>>()
}

fn use_auth_modal_ctx() -> Option<Signal<bool>> {
  try_use_context::<Signal<bool>>()
}

fn use_language_ctx() -> Language {
  try_use_context::<Signal<Language>>().map(|s| s()).unwrap_or(Language::Zh)
}

/// Forum-scoped static translations.
fn tf(lang: Language, key: &str) -> &'static str {
  match (lang, key) {
    (Language::En, "forum.title") => "Forum",
    (_, "forum.title") => "论坛",
    (Language::En, "forum.subtitle") => "Discuss, share, and grow together",
    (_, "forum.subtitle") => "交流、分享、共同进步",
    (Language::En, "forum.my_topics") => "My Topics",
    (_, "forum.my_topics") => "我的话题",
    (Language::En, "forum.post_topic") => "Post Topic",
    (_, "forum.post_topic") => "发起话题",
    (Language::En, "forum.popular_tags") => "Popular Tags",
    (_, "forum.popular_tags") => "热门标签",
    (Language::En, "forum.loading") => "Loading...",
    (_, "forum.loading") => "加载中...",
    (Language::En, "forum.no_tags") => "No tags yet",
    (_, "forum.no_tags") => "暂无标签",
    (Language::En, "forum.no_topics") => "No topics yet — be the first to post!",
    (_, "forum.no_topics") => "还没有话题，第一个发表的就是你！",
    (Language::En, "forum.back_all") => "← All Topics",
    (_, "forum.back_all") => "← 全部话题",
    (Language::En, "forum.my_topics_empty") => "You haven't posted any topics yet",
    (_, "forum.my_topics_empty") => "你还没有发表过话题",
    (Language::En, "forum.post_first") => "Post Your First Topic",
    (_, "forum.post_first") => "发起第一个话题",
    (Language::En, "forum.replies") => "replies",
    (_, "forum.replies") => "回复",
    (Language::En, "forum.write_reply") => "Write a Reply",
    (_, "forum.write_reply") => "撰写回复",
    (Language::En, "forum.post_reply") => "Post Reply",
    (_, "forum.post_reply") => "发表回复",
    (Language::En, "forum.submitting") => "Submitting...",
    (_, "forum.submitting") => "提交中...",
    (Language::En, "forum.login_to_reply") => "Log in to reply to topics",
    (_, "forum.login_to_reply") => "登录后才能回复话题",
    (Language::En, "forum.login") => "Log In",
    (_, "forum.login") => "登录",
    (Language::En, "forum.please_login") => "Please Log In",
    (_, "forum.please_login") => "请先登录",
    (Language::En, "forum.login_to_post") => "Log in to post a topic",
    (_, "forum.login_to_post") => "登录后才能发起话题",
    (Language::En, "forum.title_label") => "Title",
    (_, "forum.title_label") => "标题",
    (Language::En, "forum.tag_label") => "Tag",
    (_, "forum.tag_label") => "标签",
    (Language::En, "forum.edit") => "Edit",
    (_, "forum.edit") => "编辑",
    (Language::En, "forum.preview") => "Preview",
    (_, "forum.preview") => "预览",
    (Language::En, "forum.publish_topic") => "Publish Topic",
    (_, "forum.publish_topic") => "发布话题",
    (Language::En, "forum.publishing") => "Publishing...",
    (_, "forum.publishing") => "发布中...",
    (Language::En, "forum.new_topic_title") => "New Topic",
    (_, "forum.new_topic_title") => "发起话题",
    (Language::En, "forum.ref_note") => "This topic will be linked to the content above",
    (_, "forum.ref_note") => "本话题将关联以上原文",
    (Language::En, "forum.topics_under_tag") => "topics under this tag",
    (_, "forum.topics_under_tag") => "个话题",
    _ => "—",
  }
}

// =============================================================
// 局部布局
// =============================================================

#[component]
fn LocalContainer(children: Element) -> Element {
  rsx! { div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8", {children} } }
}

#[component]
fn Spinner() -> Element {
  rsx! {
      div { class: "flex items-center justify-center py-20",
          div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
      }
  }
}

#[component]
fn EmptyState(message: String) -> Element {
  rsx! {
      div { class: "text-center text-slate-500 dark:text-slate-400 py-16",
          "{message}"
      }
  }
}

// =============================================================
// Tag badge & Reference card
// =============================================================

#[component]
fn TagBadge(tag: String) -> Element {
  let href = format!("/topics/tag/{}", tag);
  rsx! {
      a {
          href: "{href}",
          class: "inline-flex items-center text-xs px-2 py-0.5 rounded-full bg-blue-50 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 hover:bg-blue-100 dark:hover:bg-blue-900/50 transition-colors",
          "#{tag}"
      }
  }
}

#[component]
fn ReferenceCard(reference: TopicRef) -> Element {
  let TopicRef { kind, path, title } = reference;
  let (label, href) = ref_link_for(&kind, &path);
  rsx! {
      a {
          href: "{href}",
          class: "block rounded-xl border border-slate-200 dark:border-slate-700 bg-slate-50/50 dark:bg-slate-900/40 px-4 py-3 hover:border-blue-300 dark:hover:border-blue-700 transition-colors",
          div { class: "flex items-center gap-2 text-xs text-slate-500 dark:text-slate-400 mb-1",
              span { class: "px-1.5 py-0.5 rounded bg-slate-200 dark:bg-slate-700 font-medium uppercase tracking-wide",
                  "{label}"
              }
              span { class: "truncate", "{path}" }
          }
          div { class: "text-sm font-semibold text-slate-800 dark:text-slate-100 truncate",
              "📎 {title}"
          }
      }
  }
}

fn ref_link_for(kind: &str, path: &str) -> (&'static str, String) {
  match kind {
    "blog" => ("BLOG", format!("/blog/{}", path)),
    "doc" => ("DOC", format!("/docs/{}", path)),
    "course" => ("COURSE", format!("/course/{}", path)),
    "case" => ("CASE", format!("/case/{}", path)),
    "lesson" => {
      // path = "<slug>/<chapter>/<lesson>"
      ("LESSON", format!("/course/{}", path))
    }
    _ => ("REF", format!("/{}", path)),
  }
}

// =============================================================
// Topic Card（列表项）
// =============================================================

#[component]
fn TopicCard(topic: TopicSummary) -> Element {
  let lang = use_language_ctx();
  let TopicSummary {
    id,
    title,
    tag,
    author,
    author_avatar,
    reply_count,
    last_reply_at,
    created_at,
    reference,
    ..
  } = topic;
  let detail_href = format!("/topics/{}", id);
  let when = last_reply_at.unwrap_or_else(|| created_at.clone());
  let reply_label = format!("{} {}", reply_count, tf(lang, "forum.replies"));
  rsx! {
      div { class: "group rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 p-5 hover:border-blue-300 dark:hover:border-blue-700 hover:shadow-sm transition-all",
          div { class: "flex gap-4",
              // Avatar
              div { class: "flex-none",
                  if let Some(ref a) = author_avatar {
                      img { src: "{a}", class: "w-10 h-10 rounded-full object-cover", alt: "{author}" }
                  } else {
                      div { class: "w-10 h-10 rounded-full bg-blue-600 text-white flex items-center justify-center font-bold",
                          "{author.chars().next().unwrap_or('U')}"
                      }
                  }
              }
              // Body
              div { class: "flex-1 min-w-0",
                  div { class: "flex items-center gap-2 mb-1 flex-wrap",
                      TagBadge { tag: tag.clone() }
                      if reference.is_some() {
                          span { class: "text-xs text-slate-400", "📎" }
                      }
                  }
                  a { href: "{detail_href}",
                      class: "block text-base font-semibold text-slate-900 dark:text-white group-hover:text-blue-600 transition-colors line-clamp-2",
                      "{title}"
                  }
                  if let Some(ref r) = reference {
                      div { class: "mt-1 text-xs text-slate-500 dark:text-slate-400 truncate",
                          "📎 {r.title}"
                      }
                  }
                  div { class: "mt-2 flex items-center gap-3 text-xs text-slate-500 dark:text-slate-400",
                      span { "{author}" }
                      span { "·" }
                      span { "{reply_label}" }
                      span { "·" }
                      span { "{when}" }
                  }
              }
          }
      }
  }
}

// =============================================================
// /topics  index page
// =============================================================

#[component]
pub fn TopicsIndexPage() -> Element {
  let topics_res =
    use_resource(|| async move { list_topics(None, Some(0)).await.unwrap_or_default() });
  let tags_res = use_resource(|| async move { list_tags().await.unwrap_or_default() });

  let topics = topics_res.read().as_ref().cloned();
  let tags = tags_res.read().as_ref().cloned();

  let lang = use_language_ctx();
  let session_user = use_session_user_ctx();
  let is_logged_in = session_user.as_ref().map(|s| s.read().is_some()).unwrap_or(false);

  rsx! {
      section { class: "py-10 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              // Hero
              div { class: "flex items-start justify-between mb-8 flex-wrap gap-4",
                  div {
                      h1 { class: "text-3xl font-extrabold text-slate-900 dark:text-white", "{tf(lang, \"forum.title\")}" }
                      p { class: "mt-2 text-slate-600 dark:text-slate-400", "{tf(lang, \"forum.subtitle\")}" }
                  }
                  div { class: "flex items-center gap-3",
                      if is_logged_in {
                          a { href: "/me/topics",
                              class: "inline-flex items-center px-4 py-2 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 text-sm font-semibold hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                              "{tf(lang, \"forum.my_topics\")}"
                          }
                      }
                      a { href: "/topics/new",
                          class: "inline-flex items-center rounded-md btn-flow px-4 py-2 text-sm font-semibold transition-all",
                          "{tf(lang, \"forum.post_topic\")}"
                      }
                  }
              }

              // Two columns: tags on left, topics on right
              div { class: "grid grid-cols-1 lg:grid-cols-[16rem_1fr] gap-8",
                  // Tag cloud (left)
                  aside { class: "lg:sticky lg:top-20 lg:self-start",
                      div { class: "rounded-xl border border-slate-300 dark:border-slate-700 p-5 bg-slate-100 dark:bg-slate-800",
                          h3 { class: "text-base font-extrabold text-slate-900 dark:text-slate-100 mb-3", "{tf(lang, \"forum.popular_tags\")}" }
                          match tags {
                              None => rsx! { div { class: "text-sm text-slate-400", "{tf(lang, \"forum.loading\")}" } },
                              Some(list) if list.is_empty() => rsx! {
                                  div { class: "text-sm text-slate-400", "{tf(lang, \"forum.no_tags\")}" }
                              },
                              Some(list) => rsx! {
                                  div { class: "flex flex-wrap gap-2",
                                      for t in list.into_iter() {
                                          TagCloudLink { key: "{t.tag}", tag: t }
                                      }
                                  }
                              },
                          }
                      }
                  }
                  // Topics list (right)
                  div {
                      match topics {
                          None => rsx! { Spinner {} },
                          Some(list) if list.is_empty() => rsx! {
                              EmptyState { message: tf(lang, "forum.no_topics").to_string() }
                          },
                          Some(list) => rsx! {
                              div { class: "space-y-4",
                                  for t in list.into_iter() {
                                      TopicCard { key: "{t.id}", topic: t }
                                  }
                              }
                          },
                      }
                  }
              }
          }
      }
  }
}

#[component]
fn TagCloudLink(tag: TagSummary) -> Element {
  let href = format!("/topics/tag/{}", tag.tag);
  rsx! {
      a {
          href: "{href}",
          class: "inline-flex items-center gap-1 text-xs px-2.5 py-1 rounded-full bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-200 hover:border-blue-400 hover:text-blue-600 transition-colors",
          span { "#{tag.tag}" }
          span { class: "text-slate-400", "{tag.topic_count}" }
      }
  }
}

// =============================================================
// /topics/tag/:tag
// =============================================================

#[component]
pub fn TopicsByTagPage(tag: String) -> Element {
  let lang = use_language_ctx();
  let tag_for_res = tag.clone();
  let res = use_resource(move || {
    let t = tag_for_res.clone();
    async move { list_topics(Some(t), Some(0)).await.unwrap_or_default() }
  });
  let topics = res.read().as_ref().cloned();

  rsx! {
      section { class: "py-10 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              div { class: "mb-6",
                  a { href: "/topics", class: "text-sm text-blue-600 hover:underline", "{tf(lang, \"forum.back_all\")}" }
              }
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-2",
                  "#{tag}"
              }
              if let Some(ref list) = topics {
                  p { class: "text-sm text-slate-500 dark:text-slate-400 mb-6",
                      if lang == Language::En {
                          "{list.len()} {tf(lang, \"forum.topics_under_tag\")}"
                      } else {
                          "该标签下共 {list.len()} {tf(lang, \"forum.topics_under_tag\")}"
                      }
                  }
              }

              match topics {
                  None => rsx! { Spinner {} },
                  Some(list) if list.is_empty() => rsx! {
                      EmptyState { message: format!("#{}", tag) }
                  },
                  Some(list) => rsx! {
                      div { class: "space-y-4",
                          for t in list.into_iter() {
                              TopicCard { key: "{t.id}", topic: t }
                          }
                      }
                  },
              }
          }
      }
  }
}

// =============================================================
// /me/topics
// =============================================================

#[component]
pub fn MyTopicsPage() -> Element {
  let lang = use_language_ctx();
  let res = use_resource(|| async move { list_my_topics().await.unwrap_or_default() });
  let topics = res.read().as_ref().cloned();

  rsx! {
      section { class: "py-10 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-6", "{tf(lang, \"forum.my_topics\")}" }
              match topics {
                  None => rsx! { Spinner {} },
                  Some(list) if list.is_empty() => rsx! {
                      div { class: "text-center py-16",
                          p { class: "text-slate-500 dark:text-slate-400 mb-4", "{tf(lang, \"forum.my_topics_empty\")}" }
                          a { href: "/topics/new",
                              class: "inline-flex items-center px-4 py-2 rounded-lg btn-flow text-sm font-semibold transition-all",
                              "{tf(lang, \"forum.post_first\")}"
                          }
                      }
                  },
                  Some(list) => rsx! {
                      div { class: "space-y-4",
                          for t in list.into_iter() {
                              TopicCard { key: "{t.id}", topic: t }
                          }
                      }
                  },
              }
          }
      }
  }
}

// =============================================================
// /topics/:id  Detail page
// =============================================================

#[component]
pub fn TopicDetailPage(id: i32) -> Element {
  let detail_res = use_resource(move || async move { get_topic(id).await.ok().flatten() });
  let detail = detail_res.read().as_ref().cloned();

  let mut detail_state = use_signal::<Option<TopicDetail>>(|| None);
  use_effect(move || {
    if let Some(Some(d)) = detail_res.read().as_ref().cloned() {
      detail_state.set(Some(d));
    }
  });

  let current = detail_state.read().clone().or_else(|| detail.flatten());

  let lang = use_language_ctx();
  rsx! {
      section { class: "py-10 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              div { class: "mb-6",
                  a { href: "/topics", class: "text-sm text-blue-600 hover:underline", "{tf(lang, \"forum.back_all\")}" }
              }
              match current {
                  None => rsx! { Spinner {} },
                  Some(d) => rsx! {
                      TopicDetailBody {
                          detail: d,
                          on_replied: move |new_detail: TopicDetail| {
                              detail_state.set(Some(new_detail));
                          }
                      }
                  },
              }
          }
      }
  }
}

#[component]
fn TopicDetailBody(detail: TopicDetail, on_replied: EventHandler<TopicDetail>) -> Element {
  let TopicDetail {
    id,
    title,
    tag,
    content,
    author,
    author_avatar,
    created_at,
    reference,
    replies,
    ..
  } = detail;
  let lang = use_language_ctx();
  let blog_id = format!("topic:{}", id);
  let replies_heading = if lang == Language::En {
    format!("{} {}", replies.len(), tf(lang, "forum.replies"))
  } else {
    format!("{} 条{}", replies.len(), tf(lang, "forum.replies"))
  };
  rsx! {
      article {
          // 标题区
          div { class: "mb-6",
              h1 { class: "text-2xl md:text-3xl font-extrabold text-slate-900 dark:text-white mb-3",
                  "{title}"
              }
              div { class: "flex items-center gap-3 text-sm text-slate-500 dark:text-slate-400 flex-wrap",
                  TagBadge { tag: tag.clone() }
                  if let Some(ref a) = author_avatar {
                      img { src: "{a}", class: "w-6 h-6 rounded-full object-cover", alt: "{author}" }
                  }
                  span { "{author}" }
                  span { "·" }
                  span { "{created_at}" }
              }
          }
          // 引用卡片
          if let Some(r) = reference.clone() {
              div { class: "mb-6",
                  ReferenceCard { reference: r }
              }
          }
          // 正文
          div { class: "prose prose-slate dark:prose-invert max-w-none mb-10",
              Markdown { content: content.clone(), blog_id: blog_id.clone(), untrusted: true }
          }
          // 回复
          div { class: "border-t border-slate-200 dark:border-slate-800 pt-8",
              h2 { class: "text-lg font-bold text-slate-900 dark:text-white mb-6",
                  "{replies_heading}"
              }
              div { class: "space-y-6 mb-10",
                  for r in replies.iter() {
                      ReplyItem { key: "{r.id}", reply: r.clone(), parent_blog_id: blog_id.clone() }
                  }
              }
              ReplyComposer { topic_id: id, on_replied }
          }
      }
  }
}

#[component]
fn ReplyItem(reply: Reply, parent_blog_id: String) -> Element {
  let Reply { content, author, author_avatar, created_at, .. } = reply;
  rsx! {
      div { class: "flex gap-4",
          div { class: "flex-none",
              if let Some(ref a) = author_avatar {
                  img { src: "{a}", class: "w-10 h-10 rounded-full object-cover", alt: "{author}" }
              } else {
                  div { class: "w-10 h-10 rounded-full bg-slate-200 dark:bg-slate-800 flex items-center justify-center text-slate-500",
                      "{author.chars().next().unwrap_or('U')}"
                  }
              }
          }
          div { class: "flex-1 min-w-0",
              div { class: "flex items-center justify-between mb-1",
                  h4 { class: "text-sm font-bold text-slate-900 dark:text-white", "{author}" }
                  span { class: "text-xs text-slate-500", "{created_at}" }
              }
              div { class: "prose prose-sm prose-slate dark:prose-invert max-w-none",
                  Markdown { content: content.clone(), blog_id: parent_blog_id.clone(), untrusted: true }
              }
          }
      }
  }
}

// =============================================================
// Reply Composer
// =============================================================

#[component]
fn ReplyComposer(topic_id: i32, on_replied: EventHandler<TopicDetail>) -> Element {
  let lang = use_language_ctx();
  let session_user = use_session_user_ctx();
  let auth_modal = use_auth_modal_ctx();
  let is_logged_in = session_user.as_ref().map(|s| s.read().is_some()).unwrap_or(false);

  let mut content = use_signal(String::new);
  let mut submitting = use_signal(|| false);
  let mut error = use_signal::<Option<String>>(|| None);

  let handle_submit = move |_| {
    if content().trim().is_empty() || submitting() {
      return;
    }
    let body = content();
    let on_replied = on_replied.clone();
    spawn(async move {
      submitting.set(true);
      error.set(None);
      match post_reply(topic_id, body).await {
        Ok(d) => {
          content.set(String::new());
          on_replied.call(d);
        }
        Err(e) => {
          error.set(Some(format!("发表失败: {}", e)));
        }
      }
      submitting.set(false);
    });
  };

  if !is_logged_in {
    return rsx! {
        div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/40 p-6 text-center",
            p { class: "text-sm text-slate-500 dark:text-slate-400 mb-3", "{tf(lang, \"forum.login_to_reply\")}" }
            if let Some(mut auth) = auth_modal {
                button {
                    onclick: move |_| auth.set(true),
                    class: "px-4 py-2 rounded-lg bg-blue-600 text-white text-sm font-semibold hover:bg-blue-700 transition-colors",
                    "{tf(lang, \"forum.login\")}"
                }
            }
        }
    };
  }

  rsx! {
      div { class: "rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/40 overflow-hidden",
          div { class: "px-5 py-3 border-b border-slate-100 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-800/30",
              span { class: "text-sm font-medium text-slate-700 dark:text-slate-300", "{tf(lang, \"forum.write_reply\")}" }
          }
          div { class: "px-5 py-4",
              textarea {
                  class: "w-full h-32 bg-transparent border-0 focus:ring-0 text-sm text-slate-700 dark:text-slate-200 placeholder-slate-400 resize-vertical",
                  placeholder: "Markdown...",
                  value: "{content}",
                  oninput: move |e| content.set(e.value()),
              }
          }
          if let Some(err) = error() {
              div { class: "px-5 py-2 bg-red-50 dark:bg-red-900/20 text-xs text-red-700 dark:text-red-400",
                  "{err}"
              }
          }
          div { class: "px-5 py-3 bg-slate-50/50 dark:bg-slate-800/30 flex justify-end border-t border-slate-100 dark:border-slate-800",
              button {
                  class: format_args!("px-5 py-2 rounded-lg font-semibold text-sm transition-all {}",
                      if submitting() { "bg-slate-200 text-slate-400 cursor-not-allowed" }
                      else { "btn-flow" }
                  ),
                  disabled: submitting(),
                  onclick: move |evt| handle_submit(evt),
                  if submitting() { "{tf(lang, \"forum.submitting\")}" } else { "{tf(lang, \"forum.post_reply\")}" }
              }
          }
      }
  }
}

// =============================================================
// /topics/new  with optional ?ref_kind=&ref_path=
// =============================================================

#[component]
pub fn NewTopicPage() -> Element {
  let lang = use_language_ctx();
  let session_user = use_session_user_ctx();
  let auth_modal = use_auth_modal_ctx();
  let is_logged_in = session_user.as_ref().map(|s| s.read().is_some()).unwrap_or(false);

  let mut title = use_signal(String::new);
  let mut tag_value = use_signal(String::new);
  let mut content = use_signal(String::new);
  let mut is_preview = use_signal(|| false);
  let mut submitting = use_signal(|| false);
  let mut error = use_signal::<Option<String>>(|| None);

  // 引用 query 参数（?ref_kind=&ref_path=）
  let mut ref_kind = use_signal::<Option<String>>(|| None);
  let mut ref_path = use_signal::<Option<String>>(|| None);

  use_effect(move || {
    spawn(async move {
      let script = r#"
                const p = new URLSearchParams(window.location.search);
                const k = p.get('ref_kind');
                const r = p.get('ref_path');
                dioxus.send([k, r]);
            "#;
      let mut e = dioxus::document::eval(script);
      if let Ok(arr) = e.recv::<(Option<String>, Option<String>)>().await {
        let (k, p) = arr;
        if let Some(ref kind) = k {
          if !kind.is_empty() {
            ref_kind.set(Some(kind.clone()));
            if tag_value().is_empty() {
              tag_value.set(format!("from-{}", kind));
            }
          }
        }
        if let Some(ref pp) = p {
          if !pp.is_empty() {
            ref_path.set(Some(pp.clone()));
          }
        }
      }
    });
  });

  // 已有 tag 自动补全
  let tags_res = use_resource(|| async move { list_tags().await.unwrap_or_default() });
  let existing_tags = tags_res.read().as_ref().cloned().unwrap_or_default();

  let handle_submit = move |_| {
    if submitting() {
      return;
    }
    let payload = NewTopicInput {
      title: title(),
      tag: tag_value(),
      content: content(),
      ref_kind: ref_kind(),
      ref_path: ref_path(),
    };
    spawn(async move {
      submitting.set(true);
      error.set(None);
      match create_topic(payload).await {
        Ok(summary) => {
          let url = format!("/topics/{}", summary.id);
          let _ = dioxus::document::eval(&format!("window.location.href = '{}';", url));
        }
        Err(e) => {
          error.set(Some(format!("创建失败: {}", e)));
        }
      }
      submitting.set(false);
    });
  };

  if !is_logged_in {
    return rsx! {
        section { class: "py-10 min-h-screen bg-white dark:bg-slate-950",
            LocalContainer {
                div { class: "max-w-md mx-auto rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/40 p-8 text-center",
                    h2 { class: "text-xl font-bold text-slate-900 dark:text-white mb-2", "{tf(lang, \"forum.please_login\")}" }
                    p { class: "text-sm text-slate-500 dark:text-slate-400 mb-4", "{tf(lang, \"forum.login_to_post\")}" }
                    if let Some(mut a) = auth_modal {
                        button {
                            onclick: move |_| a.set(true),
                            class: "px-4 py-2 rounded-lg bg-blue-600 text-white text-sm font-semibold hover:bg-blue-700 transition-colors",
                            "{tf(lang, \"forum.login\")}"
                        }
                    }
                }
            }
        }
    };
  }

  let kind_clone = ref_kind();
  let path_clone = ref_path();
  let has_ref = kind_clone.is_some() && path_clone.is_some();

  rsx! {
      section { class: "py-10 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              div { class: "mb-6",
                  a { href: "/topics", class: "text-sm text-blue-600 hover:underline", "{tf(lang, \"forum.back_all\")}" }
              }
              h1 { class: "text-2xl font-extrabold text-slate-900 dark:text-white mb-6", "{tf(lang, \"forum.new_topic_title\")}" }

              if has_ref {
                  div { class: "mb-6",
                      ReferenceCard {
                          reference: TopicRef {
                              kind: kind_clone.clone().unwrap_or_default(),
                              path: path_clone.clone().unwrap_or_default(),
                              title: path_clone.clone().unwrap_or_default(),
                          }
                      }
                      p { class: "mt-2 text-xs text-slate-500", "{tf(lang, \"forum.ref_note\")}" }
                  }
              }

              // 标题
              div { class: "mb-4",
                  label { class: "block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5", "{tf(lang, \"forum.title_label\")}" }
                  input {
                      r#type: "text",
                      class: "w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                      value: "{title}",
                      placeholder: if lang == Language::En { "Summarize your question or topic" } else { "一句话概括你的问题或讨论方向" },
                      oninput: move |e| title.set(e.value())
                  }
              }
              // Tag
              div { class: "mb-4",
                  label { class: "block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5", "{tf(lang, \"forum.tag_label\")}" }
                  input {
                      r#type: "text",
                      class: "w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                      value: "{tag_value}",
                      list: "forum-tags",
                      placeholder: if lang == Language::En { "e.g. rust / dioxus / wasm (letters, digits, - _)" } else { "如 rust / dioxus / wasm（仅字母数字与 - _）" },
                      oninput: move |e| tag_value.set(e.value())
                  }
                  datalist { id: "forum-tags",
                      for t in existing_tags.iter() {
                          option { value: "{t.tag}" }
                      }
                  }
              }
              // 正文
              div { class: "mb-4",
                  div { class: "flex items-center gap-4 mb-2 text-xs font-medium",
                      button {
                          class: format_args!("pb-1 border-b-2 transition-colors {}",
                              if !is_preview() { "text-blue-600 border-blue-600" } else { "border-transparent text-slate-500 hover:text-slate-700" }),
                          onclick: move |_| is_preview.set(false),
                          "{tf(lang, \"forum.edit\")}"
                      }
                      button {
                          class: format_args!("pb-1 border-b-2 transition-colors {}",
                              if is_preview() { "text-blue-600 border-blue-600" } else { "border-transparent text-slate-500 hover:text-slate-700" }),
                          onclick: move |_| is_preview.set(true),
                          "{tf(lang, \"forum.preview\")}"
                      }
                  }
                  if !is_preview() {
                      textarea {
                          class: "w-full h-64 px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono",
                          value: "{content}",
                          placeholder: "Markdown...",
                          oninput: move |e| content.set(e.value())
                      }
                  } else {
                      div { class: "prose prose-slate dark:prose-invert max-w-none min-h-[16rem] p-4 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50/50 dark:bg-slate-900/40",
                          Markdown { content: content(), blog_id: "topic:preview".to_string(), untrusted: true }
                      }
                  }
              }

              if let Some(err) = error() {
                  div { class: "mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400 rounded-lg",
                      "{err}"
                  }
              }

              div { class: "flex justify-end",
                  button {
                      class: format_args!("px-5 py-2 rounded-lg font-semibold text-sm transition-all {}",
                          if submitting() { "bg-slate-200 text-slate-400 cursor-not-allowed" }
                          else { "btn-flow" }),
                      disabled: submitting(),
                      onclick: move |evt| handle_submit(evt),
                      if submitting() { "{tf(lang, \"forum.publishing\")}" } else { "{tf(lang, \"forum.publish_topic\")}" }
                  }
              }
          }
      }
  }
}

// =============================================================
// DiscussionPanel — 嵌入到 Blog/Doc/Lesson 等源资源页面底部
// =============================================================

#[component]
pub fn DiscussionPanel(resource_kind: String, resource_path: String) -> Element {
  let kind_for_res = resource_kind.clone();
  let path_for_res = resource_path.clone();
  let res = use_resource(move || {
    let k = kind_for_res.clone();
    let p = path_for_res.clone();
    async move { list_topics_by_ref(k, p).await.unwrap_or_default() }
  });
  let topics = res.read().as_ref().cloned();

  let new_href = format!(
    "/topics/new?ref_kind={}&ref_path={}",
    urlencode(&resource_kind),
    urlencode(&resource_path)
  );

  rsx! {
      section { class: "mt-12 border-t border-slate-200 dark:border-slate-800 pt-8",
          div { class: "flex items-center justify-between mb-5",
              h3 { class: "text-lg font-bold text-slate-900 dark:text-white", "💬 关联讨论" }
              a { href: "{new_href}",
                  class: "inline-flex items-center px-3 py-1.5 rounded-lg btn-flow text-xs font-semibold transition-all",
                  "发起讨论"
              }
          }
          match topics {
              None => rsx! { div { class: "text-sm text-slate-400", "加载中..." } },
              Some(list) if list.is_empty() => rsx! {
                  div { class: "text-sm text-slate-500 dark:text-slate-400 py-4 text-center",
                      "还没有围绕本页的讨论，欢迎发起一个！"
                  }
              },
              Some(list) => rsx! {
                  div { class: "space-y-3",
                      for t in list.into_iter() {
                          DiscussionMiniRow { key: "{t.id}", topic: t }
                      }
                  }
              },
          }
      }
  }
}

#[component]
fn DiscussionMiniRow(topic: TopicSummary) -> Element {
  let TopicSummary { id, title, tag, author, reply_count, last_reply_at, created_at, .. } = topic;
  let href = format!("/topics/{}", id);
  let when = last_reply_at.unwrap_or(created_at);
  rsx! {
      a { href: "{href}",
          class: "flex items-center justify-between p-3 rounded-lg border border-slate-200 dark:border-slate-800 hover:border-blue-300 dark:hover:border-blue-700 hover:bg-blue-50/30 dark:hover:bg-blue-900/10 transition-colors",
          div { class: "flex-1 min-w-0",
              div { class: "flex items-center gap-2 mb-0.5",
                  TagBadge { tag: tag.clone() }
                  span { class: "text-xs text-slate-400", "{author} · {when}" }
              }
              div { class: "text-sm font-medium text-slate-800 dark:text-slate-200 truncate", "{title}" }
          }
          div { class: "shrink-0 ml-3 text-xs text-slate-500 dark:text-slate-400", "{reply_count} 回复" }
      }
  }
}

/// 极简 URL 编码（仅处理论坛 path 片段中可能出现的 reserved 字符）
fn urlencode(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for ch in s.chars() {
    match ch {
      '0'..='9' | 'a'..='z' | 'A'..='Z' | '-' | '_' | '.' | '~' | '/' => out.push(ch),
      _ => {
        let mut buf = [0u8; 4];
        for b in ch.encode_utf8(&mut buf).as_bytes() {
          out.push_str(&format!("%{:02X}", b));
        }
      }
    }
  }
  out
}
