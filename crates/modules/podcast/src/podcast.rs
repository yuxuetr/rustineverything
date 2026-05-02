use dioxus::prelude::*;
use dioxus::prelude::try_use_context;
use crate::server::{list_episodes, get_episode_by_id, Episode};
use rustineverything_core::i18n::Language;

/// 从 context 获取当前语言，默认中文
fn use_language_ctx() -> Language {
    try_use_context::<Signal<Language>>()
        .map(|s| s())
        .unwrap_or(Language::Zh)
}

/// 播客页面静态翻译
fn tp(lang: Language, key: &str) -> &'static str {
    match (lang, key) {
        (Language::En, "podcast.title")      => "Rust Deep Podcast",
        (Language::Zh, "podcast.title")      => "Rust 深度播客",
        (Language::En, "podcast.subtitle")   => "Explore the frontiers of technology, listen to ideas that resonate",
        (Language::Zh, "podcast.subtitle")   => "探索技术边界，聆听思维回响",
        (Language::En, "podcast.more")       => "Episodes",
        (Language::Zh, "podcast.more")       => "节目列表",
        (Language::En, "podcast.guest")      => "Guest",
        (Language::Zh, "podcast.guest")      => "嘉宾",
        (Language::En, "podcast.empty")      => "No episodes yet",
        (Language::Zh, "podcast.empty")      => "暂无节目",
        (Language::En, "podcast.all")        => "All",
        (Language::Zh, "podcast.all")        => "全部",
        (Language::En, "podcast.no_results") => "No episodes match this tag",
        (Language::Zh, "podcast.no_results") => "没有匹配该标签的节目",
        _ => "",
    }
}

#[component]
fn LocalContainer(children: Element) -> Element {
    rsx! { div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8", {children} } }
}

#[component]
fn LocalSectionTitle(title: String, subtitle: Option<String>) -> Element {
    rsx! {
        div { class: "text-center mb-10",
            h2 { class: "text-3xl font-bold tracking-tight text-[var(--color-text)] sm:text-4xl", "{title}" }
            if let Some(s) = subtitle {
                p { class: "mt-4 text-lg leading-8 text-[var(--color-text-muted)]", "{s}" }
            }
        }
    }
}

const PAGE_SIZE: usize = 8;

/// Podcast 主页面：从 server 动态加载节目列表
#[component]
pub fn PodcastPage() -> Element {
    let lang = use_language_ctx();

    let episodes_res = use_resource(move || async move {
        list_episodes().await.unwrap_or_default()
    });

    let episodes = episodes_res.read().as_ref().cloned().unwrap_or_default();
    let mut current_episode = use_signal::<Option<Episode>>(|| None);
    let mut is_playing = use_signal(|| false);
    let mut active_tag = use_signal::<Option<String>>(|| None);
    let mut current_page = use_signal(|| 0usize);

    // 加载完成后默认选中第一个节目（不自动播放）
    {
        let eps_for_default = episodes.clone();
        use_effect(use_reactive!(|eps_for_default| {
            if current_episode.read().is_none() {
                if let Some(first) = eps_for_default.first().cloned() {
                    current_episode.set(Some(first));
                }
            }
        }));
    }

    // 汇总所有 tag（有序去重）
    let all_tags: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        for ep in episodes.iter() {
            for t in ep.tags.iter() {
                seen.insert(t.clone());
            }
        }
        seen.into_iter().collect()
    };

    // 按 tag 过滤节目
    let filtered: Vec<Episode> = match active_tag() {
        Some(ref tag) => episodes.iter().filter(|e| e.tags.contains(tag)).cloned().collect(),
        None => episodes.clone(),
    };

    // 分页计算（tag 变化时自动限定页码范围）
    let total_pages = ((filtered.len() + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
    let safe_page = current_page().min(total_pages - 1);
    let paged: Vec<Episode> = filtered.iter()
        .skip(safe_page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect();

    rsx! {
        section { class: "py-12 bg-[var(--color-bg)] transition-colors duration-300",
            LocalContainer {
                LocalSectionTitle {
                    title: tp(lang, "podcast.title").to_string(),
                    subtitle: Some(tp(lang, "podcast.subtitle").to_string())
                }

                match (current_episode.read().clone(), episodes.is_empty()) {
                    (None, true) => rsx! {
                        div { class: "flex items-center justify-center py-20",
                            div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                        }
                    },
                    (None, false) => rsx! {
                        div { class: "text-center text-slate-500 py-20", "{tp(lang, \"podcast.empty\")}" }
                    },
                    (Some(active), _) => rsx! {
                        // 2列布局：左=播放器+标签（sticky），右=节目列表（sticky）
                        div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6 mt-10 lg:items-start",

                            // ── 左列：播放器卡片 + 标签过滤（树物协公，内容有限）──
                            div { class: "lg:sticky lg:top-20 flex flex-col gap-4",

                                // 播放器卡片
                                div { class: "overflow-hidden rounded-2xl bg-slate-900 shadow-2xl",
                                    div { class: "p-8 md:p-10",
                                        if let Some(ref guest) = active.guest {
                                            div { class: "text-xs font-bold text-blue-400 uppercase tracking-widest mb-3",
                                                "{tp(lang, \"podcast.guest\")}: {guest}"
                                            }
                                        }
                                        h2 { class: "text-2xl md:text-3xl font-bold tracking-tight text-white leading-snug",
                                            "{active.title}"
                                        }
                                        p { class: "mt-4 text-base leading-relaxed text-slate-300",
                                            "{active.description}"
                                        }
                                        div { class: "mt-4 flex items-center gap-3 text-sm text-slate-400",
                                            span { "{active.date}" }
                                            span { "·" }
                                            span { "{active.duration}" }
                                        }
                                        if !active.tags.is_empty() {
                                            div { class: "mt-4 flex flex-wrap gap-2",
                                                for tag in active.tags.iter() {
                                                    span { class: "text-xs px-2.5 py-1 rounded-full bg-slate-800 text-slate-300",
                                                        "#{tag}"
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "mt-8",
                                            audio {
                                                class: "w-full focus:outline-none",
                                                controls: true,
                                                src: "{active.url}",
                                                onplay: move |_| is_playing.set(true),
                                                onpause: move |_| is_playing.set(false),
                                                "Your browser does not support the audio element."
                                            }
                                        }
                                    }
                                }

                                // 标签过滤面板（播放器下方）
                                if !all_tags.is_empty() {
                                    div { class: "rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 p-5",
                                        p { class: "text-xs font-semibold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-3",
                                            if lang == Language::Zh { "标签筛选" } else { "Filter by tag" }
                                        }
                                        div { class: "flex flex-wrap gap-2",
                                            // "全部" chip
                                            {
                                                let is_all = active_tag().is_none();
                                                rsx! {
                                                    button {
                                                        onclick: move |_| active_tag.set(None),
                                                        class: format_args!(
                                                            "inline-flex items-center gap-1 text-xs px-3 py-1 rounded-full font-medium transition-colors {}",
                                                            if is_all {
                                                                "bg-blue-600 text-white"
                                                            } else {
                                                                "bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-300 border border-slate-200 dark:border-slate-700 hover:border-blue-400 dark:hover:border-blue-500 hover:text-blue-600"
                                                            }
                                                        ),
                                                        "{tp(lang, \"podcast.all\")}"
                                                        span { class: "opacity-60", "{episodes.len()}" }
                                                    }
                                                }
                                            }
                                            // 各 tag chip
                                            for tag in all_tags.iter() {
                                                {
                                                    let t = tag.clone();
                                                    let t2 = tag.clone();
                                                    let count = episodes.iter().filter(|e| e.tags.contains(&t)).count();
                                                    let is_active = active_tag().as_deref() == Some(tag.as_str());
                                                    rsx! {
                                                        button {
                                                            key: "{t}",
                                                            onclick: move |_| active_tag.set(Some(t.clone())),
                                                            class: format_args!(
                                                                "inline-flex items-center gap-1 text-xs px-3 py-1 rounded-full font-medium transition-colors {}",
                                                                if is_active {
                                                                    "bg-blue-600 text-white"
                                                                } else {
                                                                    "bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-300 border border-slate-200 dark:border-slate-700 hover:border-blue-400 dark:hover:border-blue-500 hover:text-blue-600"
                                                                }
                                                            ),
                                                            "#{t2}"
                                                            span { class: "opacity-60", "{count}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // ── 右列：节目列表（占满原列表+标签列合并宽度）──
                            div { class: "lg:sticky lg:top-20 rounded-2xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/50 flex flex-col",

                                div { class: "flex items-center justify-between px-5 pt-5 pb-3",
                                    h3 { class: "text-sm font-semibold text-[var(--color-text)]",
                                        "{tp(lang, \"podcast.more\")}"
                                    }
                                    span { class: "text-xs text-slate-400 tabular-nums",
                                        "{filtered.len()} / {episodes.len()}"
                                    }
                                }
                                div { class: "border-t border-slate-200 dark:border-slate-800" }

                                div { class: "divide-y divide-slate-200 dark:divide-slate-800",
                                    if paged.is_empty() {
                                        div { class: "text-center text-slate-400 text-sm py-10",
                                            "{tp(lang, \"podcast.no_results\")}"
                                        }
                                    }
                                    for episode in paged.iter() {
                                        {
                                            let ep_clone = episode.clone();
                                            let active_id = active.id;
                                            let is_active_ep = active_id == episode.id;
                                            rsx! {
                                                button {
                                                    key: "{episode.id}",
                                                    onclick: move |_| current_episode.set(Some(ep_clone.clone())),
                                                    class: format_args!(
                                                        "w-full text-left px-5 py-4 transition-all {}",
                                                        if is_active_ep {
                                                            "bg-orange-50 dark:bg-blue-900/20"
                                                        } else {
                                                            "hover:bg-white dark:hover:bg-slate-800/60"
                                                        }
                                                    ),
                                                    div { class: "flex items-start gap-3",
                                                        div { class: format_args!(
                                                            "mt-1.5 shrink-0 w-2 h-2 rounded-full {}",
                                                            if is_active_ep { "bg-blue-500 animate-pulse" } else { "bg-slate-300 dark:bg-slate-600" }
                                                        )}
                                                        div { class: "flex-auto min-w-0",
                                                            h4 { class: format_args!(
                                                                "text-sm font-medium line-clamp-2 leading-snug {}",
                                                                if is_active_ep { "text-blue-600 dark:text-blue-400" } else { "text-[var(--color-text)]" }
                                                            ),
                                                                "{episode.title}"
                                                            }
                                                            div { class: "mt-1 flex items-center gap-2 text-xs text-slate-400",
                                                                span { "{episode.date}" }
                                                                span { "·" }
                                                                span { "{episode.duration}" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if total_pages > 1 {
                                    div { class: "flex items-center justify-between px-5 py-3 border-t border-slate-200 dark:border-slate-800",
                                        button {
                                            disabled: safe_page == 0,
                                            onclick: move |_| { if current_page() > 0 { current_page.set(current_page() - 1); } },
                                            class: format_args!(
                                                "px-3 py-1 rounded-lg text-sm transition-colors {}",
                                                if safe_page == 0 { "text-slate-300 dark:text-slate-600 cursor-not-allowed" }
                                                else { "text-slate-600 dark:text-slate-300 hover:bg-white dark:hover:bg-slate-800" }
                                            ),
                                            "←"
                                        }
                                        span { class: "text-xs text-slate-400 tabular-nums",
                                            "{safe_page + 1} / {total_pages}"
                                        }
                                        button {
                                            disabled: safe_page + 1 >= total_pages,
                                            onclick: move |_| { if current_page() + 1 < total_pages { current_page.set(current_page() + 1); } },
                                            class: format_args!(
                                                "px-3 py-1 rounded-lg text-sm transition-colors {}",
                                                if safe_page + 1 >= total_pages { "text-slate-300 dark:text-slate-600 cursor-not-allowed" }
                                                else { "text-slate-600 dark:text-slate-300 hover:bg-white dark:hover:bg-slate-800" }
                                            ),
                                            "→"
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

/// 可在 Markdown 中嵌入的 PodcastCard 组件
#[component]
pub fn PodcastCard(id: i32) -> Element {
    let episode_res = use_resource(move || async move {
        get_episode_by_id(id).await.ok().flatten()
    });

    let state = episode_res.read().clone();
    match state {
        Some(Some(ep)) => rsx! {
            div { class: "not-prose my-8 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-sm flex flex-col md:flex-row gap-6 items-center",
                div { class: "flex-1 w-full",
                    div { class: "text-xs font-bold text-blue-600 uppercase tracking-widest mb-2", "Featured Podcast" }
                    h4 { class: "text-xl font-extrabold text-slate-900 dark:text-white mb-2", "{ep.title}" }
                    div { class: "text-sm text-slate-500 mb-4", "{ep.date} · {ep.duration}" }
                    audio { class: "w-full h-10", controls: true, src: "{ep.url}" }
                }
            }
        },
        Some(None) => rsx! {
            div { class: "not-prose my-8 p-4 rounded-lg border border-amber-200 bg-amber-50 text-sm text-amber-800",
                "找不到 ID 为 {id} 的播客节目"
            }
        },
        None => rsx! {
            div { class: "not-prose my-8 p-4 rounded-lg border border-slate-200 bg-slate-50 text-sm text-slate-500",
                "加载中..."
            }
        },
    }
}
