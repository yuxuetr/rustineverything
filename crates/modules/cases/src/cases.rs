use crate::server::{
  get_case, list_case_categories, list_case_tags, list_cases, Case, CaseSummary, TagSummary,
};
use dioxus::prelude::try_use_context;
use dioxus::prelude::*;
use rustineverything_core::i18n::Language;
use rustineverything_module_forum::forum::DiscussionPanel;
use rustineverything_widgets::Markdown;

fn use_language_ctx() -> Language {
  try_use_context::<Signal<Language>>().map(|s| s()).unwrap_or(Language::Zh)
}

fn tc(lang: Language, key: &str) -> &'static str {
  match (lang, key) {
        // 页面标题
        (Language::En, "case.title")          => "Rust Project Showcase",
        (Language::Zh, "case.title")          => "Rust 项目案例库",
        (Language::En, "case.subtitle")       => "Discover real-world Rust frontend, backend, fullstack, AI, Web3, CLI, embedded and tooling projects.",
        (Language::Zh, "case.subtitle")       => "发现真实世界中的 Rust 前端、后端、全栈、AI、Web3、CLI、嵌入式和工具项目。",
        (Language::En, "case.submit")         => "Submit Your Project",
        (Language::Zh, "case.submit")         => "提交你的项目",
        // 搜索
        (Language::En, "case.search")         => "Search by name, description, category or tag...",
        (Language::Zh, "case.search")         => "搜索项目名称、描述、分类或标签...",
        // 过滤
        (Language::En, "case.all")            => "All",
        (Language::Zh, "case.all")            => "全部",
        (Language::En, "case.filter")         => "Filter by Tag",
        (Language::Zh, "case.filter")         => "标签筛选",
        (Language::En, "case.no_tags")        => "No tags",
        (Language::Zh, "case.no_tags")        => "暂无标签",
        (Language::En, "case.loading")        => "Loading...",
        (Language::Zh, "case.loading")        => "加载中...",
        (Language::En, "case.clear")          => "Clear",
        (Language::Zh, "case.clear")          => "清除",
        // 卡片
        (Language::En, "case.detail")         => "Details",
        (Language::Zh, "case.detail")         => "详情",
        (Language::En, "case.featured")       => "⭐ Featured",
        (Language::Zh, "case.featured")       => "⭐ 精选",
        (Language::En, "case.featured_label") => "Featured",
        (Language::Zh, "case.featured_label") => "精选",
        // 无结果
        (Language::En, "case.empty")          => "No matching cases found.",
        (Language::Zh, "case.empty")          => "暂无匹配案例。",
        // 详情页
        (Language::En, "case.not_found")      => "Case not found",
        (Language::Zh, "case.not_found")      => "案例未找到",
        (Language::En, "case.back")           => "← Back to Showcase",
        (Language::Zh, "case.back")           => "← 返回案例库",
        (Language::En, "case.view_repo")      => "View Repo",
        (Language::Zh, "case.view_repo")      => "查看仓库",
        (Language::En, "case.visit_site")     => "Visit Site",
        (Language::Zh, "case.visit_site")     => "访问网站",
        (Language::En, "case.author_prefix")  => "Author: ",
        (Language::Zh, "case.author_prefix")  => "作者：",
        (Language::En, "case.no_readme")      => "No README provided for this case.",
        (Language::Zh, "case.no_readme")      => "该案例暂未提供 README.md。",
        (Language::En, "case.project_info")   => "Project Info",
        (Language::Zh, "case.project_info")   => "项目信息",
        (Language::En, "case.cat_field")      => "Category",
        (Language::Zh, "case.cat_field")      => "分类",
        (Language::En, "case.lang_field")     => "Language",
        (Language::Zh, "case.lang_field")     => "语言",
        (Language::En, "case.added_on")       => " · ★ {stars} · Added ",
        (Language::Zh, "case.added_on")       => " · ★ {stars} · 收录于 ",
        _ => "",
    }
}

fn category_label(lang: Language, c: &str) -> &'static str {
  match (lang, c) {
    (Language::En, "frontend") => "Frontend",
    (Language::Zh, "frontend") => "前端",
    (Language::En, "backend") => "Backend",
    (Language::Zh, "backend") => "后端",
    (Language::En, "fullstack") => "Fullstack",
    (Language::Zh, "fullstack") => "全栈",
    (_, "cli") => "CLI",
    (Language::En, "embedded") => "Embedded",
    (Language::Zh, "embedded") => "嵌入式",
    (_, "ai") => "AI",
    (_, "web3") => "Web3",
    (Language::En, "library") => "Library",
    (Language::Zh, "library") => "库/框架",
    (Language::En, "tool") => "Tool",
    (Language::Zh, "tool") => "工具",
    (Language::En, "desktop") => "Desktop",
    (Language::Zh, "desktop") => "桌面",
    (Language::En, _) => "Other",
    (Language::Zh, _) => "其他",
  }
}

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

// category_label is now language-aware (defined above)
fn category_emoji(c: &str) -> &'static str {
  match c {
    "frontend" => "🎨",
    "backend" => "⚙️",
    "fullstack" => "🚀",
    "cli" => "💻",
    "embedded" => "🔌",
    "ai" => "🤖",
    "web3" => "🔗",
    "library" => "📦",
    "tool" => "🛠️",
    "desktop" => "🖥️",
    _ => "📂",
  }
}
fn category_badge_class(c: &str) -> &'static str {
  match c {
    "frontend" => "bg-violet-100 dark:bg-violet-900/30 text-violet-700 dark:text-violet-300",
    "backend" => "bg-sky-100 dark:bg-sky-900/30 text-sky-700 dark:text-sky-300",
    "fullstack" => "bg-indigo-100 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-300",
    "cli" => "bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300",
    "embedded" => "bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300",
    "ai" => "bg-rose-100 dark:bg-rose-900/30 text-rose-700 dark:text-rose-300",
    "web3" => "bg-cyan-100 dark:bg-cyan-900/30 text-cyan-700 dark:text-cyan-300",
    "library" => "bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300",
    "tool" => "bg-teal-100 dark:bg-teal-900/30 text-teal-700 dark:text-teal-300",
    "desktop" => "bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300",
    _ => "bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300",
  }
}
fn placeholder_gradient(c: &str) -> &'static str {
  match c { "frontend"=>"bg-linear-to-br from-slate-800 via-slate-700 to-slate-600 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500","backend"=>"bg-linear-to-br from-slate-900 via-slate-800 to-slate-700 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500","fullstack"=>"bg-linear-to-br from-slate-800 via-slate-700 to-slate-500 dark:from-slate-600 dark:via-slate-500 dark:to-slate-400","cli"=>"bg-linear-to-br from-slate-900 via-slate-700 to-slate-600 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500","embedded"=>"bg-linear-to-br from-slate-800 via-slate-600 to-slate-500 dark:from-slate-600 dark:via-slate-500 dark:to-slate-400","ai"=>"bg-linear-to-br from-slate-900 via-slate-800 to-slate-600 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500","web3"=>"bg-linear-to-br from-slate-800 via-slate-700 to-slate-600 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500","library"=>"bg-linear-to-br from-slate-900 via-slate-700 to-slate-500 dark:from-slate-600 dark:via-slate-500 dark:to-slate-400","tool"=>"bg-linear-to-br from-slate-800 via-slate-600 to-slate-500 dark:from-slate-600 dark:via-slate-500 dark:to-slate-400","desktop"=>"bg-linear-to-br from-slate-900 via-slate-800 to-slate-700 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500",_=>"bg-linear-to-br from-slate-800 via-slate-700 to-slate-600 dark:from-slate-600 dark:via-slate-500 dark:to-slate-400" }
}
fn tag_color_class(t: &str) -> &'static str {
  match t {
    "axum" => "bg-sky-100 dark:bg-sky-900/30 text-sky-700 dark:text-sky-300",
    "actix" => "bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300",
    "dioxus" => "bg-indigo-100 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-300",
    "tokio" => "bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300",
    "sea-orm" => "bg-teal-100 dark:bg-teal-900/30 text-teal-700 dark:text-teal-300",
    "wasm" => "bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300",
    "opensource" => "bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300",
    "fullstack" => "bg-violet-100 dark:bg-violet-900/30 text-violet-700 dark:text-violet-300",
    "ai" => "bg-rose-100 dark:bg-rose-900/30 text-rose-700 dark:text-rose-300",
    _ => "bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300",
  }
}

fn submit_case_url() -> String {
  "https://github.com/yuxuetr/rustineverything.app/issues/new?template=add-case.yml".to_string()
}

#[component]
pub fn CasesIndexPage() -> Element {
  let lang = use_language_ctx();
  let mut query = use_signal(String::new);
  let mut selected_category = use_signal::<Option<String>>(|| None);
  let mut selected_tag = use_signal::<Option<String>>(|| None);

  let cases_res = use_resource(move || {
    let q = query();
    let category = selected_category();
    let tag = selected_tag();
    async move {
      let tags = tag.map(|value| vec![value]);
      match list_cases(tags, category, Some(q)).await {
        Ok(cases) => cases,
        Err(_) => Vec::new(),
      }
    }
  });
  let tags_res = use_resource(|| async move {
    match list_case_tags().await {
      Ok(tags) => tags,
      Err(_) => Vec::new(),
    }
  });
  let categories_res = use_resource(|| async move {
    match list_case_categories().await {
      Ok(categories) => categories,
      Err(_) => Vec::new(),
    }
  });

  let cases = cases_res.read().as_ref().cloned();
  let tags = tags_res.read().as_ref().cloned();
  let categories = categories_res.read().as_ref().cloned();
  let submit_url = submit_case_url();

  rsx! {
      section { class: "py-12 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              div { class: "flex flex-col lg:flex-row lg:items-end lg:justify-between gap-6 mb-8",
                  div {
                      p { class: "text-sm font-semibold uppercase tracking-widest text-flow mb-2", "Showcase" }
                      h1 { class: "text-3xl md:text-4xl font-extrabold text-slate-900 dark:text-white", "{tc(lang, \"case.title\")}" }
                      p { class: "mt-3 max-w-2xl text-slate-600 dark:text-slate-400",
                          "{tc(lang, \"case.subtitle\")}"
                      }
                  }
                  a {
                      href: "{submit_url}",
                      target: "_blank",
                      rel: "noopener noreferrer",
                      class: "shrink-0 inline-flex items-center gap-2 rounded-xl btn-flow px-5 py-2.5 text-sm font-semibold shadow-lg shadow-slate-500/20 hover:shadow-slate-500/40 transition-all",
                      svg { class: "w-4 h-4", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                          path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M12 4v16m8-8H4" }
                      }
                      "{tc(lang, \"case.submit\")}"
                  }
              }

              div { class: "mb-6",
                  input {
                      r#type: "search",
                      value: "{query}",
                      placeholder: "{tc(lang, \"case.search\")}",
                      class: "w-full rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 px-4 py-3 text-sm text-slate-900 dark:text-slate-100 outline-hidden focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20",
                      oninput: move |e| query.set(e.value()),
                  }
              }

              div { class: "mb-8 flex flex-wrap gap-2",
                  CategoryChip {
                      label: tc(lang, "case.all").to_string(),
                      value: None,
                      count: None,
                      current: selected_category(),
                      on_select: move |value: Option<String>| selected_category.set(value),
                  }
                  match categories {
                      Some(list) => rsx! {
                          for category in list.iter() {
                              CategoryChip {
                                  key: "{category.category}",
                                  label: category_label(lang, &category.category).to_string(),
                                  value: Some(category.category.clone()),
                                  count: Some(category.count),
                                  current: selected_category(),
                                  on_select: move |value: Option<String>| selected_category.set(value),
                              }
                          }
                      },
                      None => rsx! {},
                  }
              }

              div { class: "mb-6 flex flex-wrap items-center gap-2",
                  h2 { class: "text-sm font-bold text-slate-900 dark:text-white mr-2", "{tc(lang, \"case.filter\")}" }
                  match tags {
                      Some(list) if list.is_empty() => rsx! {
                          p { class: "text-sm text-slate-500 dark:text-slate-400", "{tc(lang, \"case.no_tags\")}" }
                      },
                      Some(list) => rsx! {
                          for tag in list.iter() {
                              TagChip {
                                  key: "{tag.tag}",
                                  tag: tag.clone(),
                                  current: selected_tag(),
                                  on_select: move |value: Option<String>| selected_tag.set(value),
                              }
                          }
                      },
                      None => rsx! { p { class: "text-sm text-slate-400", "{tc(lang, \"case.loading\")}" } },
                  }
                  if selected_tag().is_some() {
                      button {
                          class: "text-xs text-blue-600 dark:text-blue-400 hover:underline ml-1",
                          onclick: move |_| selected_tag.set(None),
                          "{tc(lang, \"case.clear\")}"
                      }
                  }
              }

              match cases {
                  None => rsx! { Spinner {} },
                  Some(list) if list.is_empty() => rsx! {
                      div { class: "rounded-2xl border border-dashed border-slate-300 dark:border-slate-700 py-20 text-center text-slate-500 dark:text-slate-400",
                          "{tc(lang, \"case.empty\")}"
                      }
                  },
                  Some(list) => rsx! {
                      div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6",
                          for case in list.iter() {
                              CaseCard { key: "{case.slug}", case: case.clone() }
                          }
                      }
                  },
              }
          }
      }
  }
}

#[component]
fn CategoryChip(
  label: String,
  value: Option<String>,
  count: Option<usize>,
  current: Option<String>,
  on_select: EventHandler<Option<String>>,
) -> Element {
  let active = current == value;
  let emoji = value.as_deref().map(category_emoji).unwrap_or("🌐");
  let cls = if active {
    "inline-flex items-center gap-1.5 rounded-full btn-flow px-4 py-2 text-xs font-bold shadow-md shadow-slate-500/20 transition-all"
  } else {
    "inline-flex items-center gap-1.5 rounded-full border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 px-4 py-2 text-xs font-semibold text-slate-600 dark:text-slate-300 hover:border-blue-400 hover:text-blue-600 hover:shadow-sm transition-all"
  };
  rsx! {
      button {
          class: "{cls}",
          onclick: move |_| on_select.call(value.clone()),
          span { "{emoji}" }
          span { "{label}" }
          if let Some(n) = count {
              span { class: if active { "bg-white/20 px-1.5 py-0.5 rounded-full text-[10px]" } else { "bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded-full text-[10px]" }, "{n}" }
          }
      }
  }
}

#[component]
fn TagChip(
  tag: TagSummary,
  current: Option<String>,
  on_select: EventHandler<Option<String>>,
) -> Element {
  let active = current.as_deref() == Some(tag.tag.as_str());
  let base = tag_color_class(&tag.tag);
  let selected = if active { None } else { Some(tag.tag.clone()) };
  if active {
    rsx! {
        button {
            class: "inline-flex items-center gap-1 rounded-lg bg-linear-to-r from-slate-800 to-slate-900 dark:from-slate-100 dark:to-slate-200 px-3 py-1.5 text-xs font-bold text-white dark:text-slate-900 shadow-md transition-all",
            onclick: move |_| on_select.call(selected.clone()),
            "#{tag.tag}"
            span { class: "bg-white/20 px-1 py-0.5 rounded text-[10px]", "{tag.count}" }
        }
    }
  } else {
    rsx! {
        button {
            class: "inline-flex items-center gap-1 rounded-lg {base} px-3 py-1.5 text-xs font-medium hover:brightness-95 hover:shadow-sm transition-all",
            onclick: move |_| on_select.call(selected.clone()),
            "#{tag.tag}"
            span { class: "opacity-50 text-[10px]", "{tag.count}" }
        }
    }
  }
}

#[component]
fn CaseCard(case: CaseSummary) -> Element {
  let lang = use_language_ctx();
  let href = format!("/case/{}", case.slug);
  let cat_label = category_label(lang, &case.category);
  let emoji = category_emoji(&case.category);
  let grad = placeholder_gradient(&case.category);
  let badge = category_badge_class(&case.category);
  rsx! {
      article { class: "group overflow-hidden rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/60 shadow-sm hover:-translate-y-1 hover:shadow-xl hover:border-blue-300 dark:hover:border-blue-700 transition-all duration-300",
          a { href: "{href}", class: "block",
              div { class: "aspect-[16/9] {grad} overflow-hidden relative",
                  if let Some(cover) = case.cover_url.as_ref() {
                      img { src: "{cover}", alt: "{case.name}", loading: "lazy", class: "h-full w-full object-cover group-hover:scale-105 transition-transform duration-500" }
                  } else {
                      div { class: "h-full w-full flex flex-col items-center justify-center gap-2",
                          span { class: "text-4xl drop-shadow-lg", "{emoji}" }
                          span { class: "text-sm font-bold text-white/80 tracking-wide", "{cat_label}" }
                      }
                  }
                  if case.favorite {
                      div { class: "absolute top-3 right-3 bg-rose-500 text-white text-[10px] font-bold px-2 py-0.5 rounded-full shadow-lg", "{tc(lang, \"case.featured\")}" }
                  }
              }
          }
          div { class: "p-5",
              div { class: "mb-3 flex items-center gap-2 flex-wrap",
                  span { class: "rounded-full px-2.5 py-1 text-[11px] font-bold {badge}", "{emoji} {cat_label}" }
                  span { class: "rounded-full bg-slate-100 dark:bg-slate-800 px-2 py-0.5 text-[10px] font-semibold text-slate-500 dark:text-slate-400 uppercase tracking-wider", "{case.language}" }
              }
              a { href: "{href}", class: "block",
                  h2 { class: "text-lg font-bold text-slate-900 dark:text-white group-hover:text-blue-600 transition-colors line-clamp-2", "{case.name}" }
              }
              p { class: "mt-2 text-sm leading-relaxed text-slate-600 dark:text-slate-400 line-clamp-2", "{case.description}" }
              if !case.tags.is_empty() {
                  div { class: "mt-3 flex flex-wrap gap-1.5",
                      for tag in case.tags.iter().take(4) {
                          span { class: "rounded-md px-2 py-0.5 text-[11px] font-medium {tag_color_class(tag)}", "#{tag}" }
                      }
                      if case.tags.len() > 4 {
                          span { class: "rounded-md bg-slate-100 dark:bg-slate-800 px-2 py-0.5 text-[11px] text-slate-500", "+{case.tags.len() - 4}" }
                      }
                  }
              }
              div { class: "mt-4 border-t border-slate-100 dark:border-slate-800 pt-4" }
              div { class: "flex items-center justify-between gap-2",
                  div { class: "flex items-center gap-1 text-xs text-slate-500 dark:text-slate-400",
                      svg { class: "w-3.5 h-3.5 text-amber-500", fill: "currentColor", view_box: "0 0 20 20",
                          path { d: "M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" }
                      }
                      span { class: "font-semibold", "{case.stars}" }
                  }
                  div { class: "flex items-center gap-2",
                      a { href: "{case.repo}", target: "_blank", rel: "noopener noreferrer", title: "GitHub",
                          class: "inline-flex items-center gap-1 text-xs font-semibold text-slate-500 hover:text-slate-900 dark:hover:text-white transition-colors",
                          svg { class: "w-3.5 h-3.5", fill: "currentColor", view_box: "0 0 24 24",
                              path { d: "M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" }
                          }
                          "Repo"
                      }
                      if let Some(site) = case.website.as_ref() {
                          a { href: "{site}", target: "_blank", rel: "noopener noreferrer",
                              class: "inline-flex items-center gap-1 text-xs font-semibold text-slate-500 hover:text-slate-900 dark:hover:text-white transition-colors",
                              "🔗 Site"
                          }
                      }
                      a { href: "{href}",
                          class: "inline-flex items-center gap-1 rounded-lg btn-flow px-3 py-1.5 text-[11px] font-bold transition-colors",
                          "{tc(lang, \"case.detail\")}"
                          svg { class: "w-3 h-3", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                              path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M9 5l7 7-7 7" }
                          }
                      }
                  }
              }
          }
      }
  }
}

#[component]
pub fn CaseDetailPage(slug: String) -> Element {
  let lang = use_language_ctx();
  let slug_for_res = slug.clone();
  let case_res = use_resource(move || {
    let s = slug_for_res.clone();
    async move {
      match get_case(s).await {
        Ok(case) => case,
        Err(_) => None,
      }
    }
  });
  let case = case_res.read().as_ref().cloned();

  rsx! {
      section { class: "py-12 min-h-screen bg-white dark:bg-slate-950",
          LocalContainer {
              match case {
                  None => rsx! { Spinner {} },
                  Some(None) => rsx! {
                      div { class: "py-20 text-center",
                          h1 { class: "text-2xl font-bold text-slate-900 dark:text-white", "{tc(lang, \"case.not_found\")}" }
                          p { class: "mt-3 text-slate-500 dark:text-slate-400", "\"{slug}\" " }
                          a { href: "/case", class: "mt-6 inline-flex rounded-lg bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-700",
                              "{tc(lang, \"case.back\")}"
                          }
                      }
                  },
                  Some(Some(case)) => rsx! { CaseDetailBody { case } },
              }
          }
      }
  }
}

#[component]
fn CaseDetailBody(case: Case) -> Element {
  let lang = use_language_ctx();
  let category = category_label(lang, &case.category);
  let initial = match case.name.chars().next() {
    Some(ch) => ch.to_string(),
    None => "R".to_string(),
  };
  let markdown_id = format!("case:{}", case.slug);
  rsx! {
      div { class: "max-w-5xl mx-auto",
          a { href: "/case", class: "text-sm font-semibold text-blue-600 dark:text-blue-400 hover:underline", "{tc(lang, \"case.back\")}" }
          div { class: "mt-6 overflow-hidden rounded-3xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50",
              div { class: "grid grid-cols-1 lg:grid-cols-[1.2fr_1fr] gap-0",
                  div { class: "p-8 md:p-10",
                      div { class: "mb-4 flex flex-wrap items-center gap-2",
                          span { class: "rounded-full bg-orange-100 dark:bg-orange-900/30 px-3 py-1 text-xs font-semibold text-orange-700 dark:text-orange-300",
                              "{category}"
                          }
                          span { class: "rounded-full bg-slate-200 dark:bg-slate-800 px-3 py-1 text-xs font-semibold text-slate-700 dark:text-slate-300",
                              "{case.language}"
                          }
                          if case.favorite {
                              span { class: "rounded-full bg-rose-100 dark:bg-rose-900/30 px-3 py-1 text-xs font-semibold text-rose-700 dark:text-rose-300",
                                  "{tc(lang, \"case.featured_label\")}"
                              }
                          }
                      }
                      h1 { class: "text-3xl md:text-4xl font-extrabold text-slate-900 dark:text-white", "{case.name}" }
                      p { class: "mt-4 text-lg leading-8 text-slate-600 dark:text-slate-400", "{case.description}" }
                      div { class: "mt-6 flex flex-wrap gap-2",
                          for tag in case.tags.iter() {
                              span { class: "rounded-full bg-white dark:bg-slate-800 px-2.5 py-1 text-xs text-slate-600 dark:text-slate-300",
                                  "#{tag}"
                              }
                          }
                      }
                      div { class: "mt-8 flex flex-wrap gap-3",
                          a {
                              href: "{case.repo}",
                              target: "_blank",
                              rel: "noopener noreferrer",
                              class: "inline-flex rounded-lg bg-slate-900 dark:bg-white px-4 py-2 text-sm font-semibold text-white dark:text-slate-900 hover:opacity-90",
                              "{tc(lang, \"case.view_repo\")}"
                          }
                          if let Some(site) = case.website.as_ref() {
                              a {
                                  href: "{site}",
                                  target: "_blank",
                                  rel: "noopener noreferrer",
                                  class: "inline-flex rounded-lg border border-slate-300 dark:border-slate-700 px-4 py-2 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:border-blue-400",
                                  "{tc(lang, \"case.visit_site\")}"
                              }
                          }
                      }
                      div { class: "mt-5 text-sm text-slate-500 dark:text-slate-400",
                          "{tc(lang, \"case.author_prefix\")}"
                          if let Some(author_url) = case.author_url.as_ref() {
                              a { href: "{author_url}", target: "_blank", rel: "noopener noreferrer", class: "hover:text-blue-600", "{case.author}" }
                          } else {
                              span { "{case.author}" }
                          }
                          span { " · ★ {case.stars} · {case.date_added}" }
                      }
                  }
                  div { class: "min-h-72 bg-linear-to-br from-slate-800 via-slate-700 to-slate-600 dark:from-slate-700 dark:via-slate-600 dark:to-slate-500",
                      if let Some(cover) = case.cover_url.as_ref() {
                          img { src: "{cover}", alt: "{case.name}", class: "h-full w-full object-cover" }
                      } else {
                          div { class: "h-full min-h-72 flex items-center justify-center text-7xl font-black text-white/90",
                              "{initial}"
                          }
                      }
                  }
              }
          }

          div { class: "mt-10 grid grid-cols-1 lg:grid-cols-[1fr_20rem] gap-8",
              article { class: "min-w-0 rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900/30 p-6 md:p-8",
                  if let Some(readme) = case.readme_md.as_ref() {
                      Markdown { content: readme.clone(), blog_id: markdown_id }
                  } else {
                      div { class: "text-slate-500 dark:text-slate-400", "{tc(lang, \"case.no_readme\")}" }
                  }
              }
              aside { class: "lg:sticky lg:top-20 lg:self-start space-y-4",
                  div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-900/40 p-5",
                      h2 { class: "text-sm font-bold text-slate-900 dark:text-white mb-3", "{tc(lang, \"case.project_info\")}" }
                      dl { class: "space-y-3 text-sm",
                          div {
                              dt { class: "text-slate-500 dark:text-slate-400", "{tc(lang, \"case.cat_field\")}" }
                              dd { class: "font-semibold text-slate-900 dark:text-white", "{category}" }
                          }
                          div {
                              dt { class: "text-slate-500 dark:text-slate-400", "{tc(lang, \"case.lang_field\")}" }
                              dd { class: "font-semibold text-slate-900 dark:text-white", "{case.language}" }
                          }
                          div {
                              dt { class: "text-slate-500 dark:text-slate-400", "Stars" }
                              dd { class: "font-semibold text-slate-900 dark:text-white", "{case.stars}" }
                          }
                      }
                  }
              }
          }

          div { class: "mt-10",
              DiscussionPanel {
                  resource_kind: "case".to_string(),
                  resource_path: case.slug,
              }
          }
      }
  }
}
