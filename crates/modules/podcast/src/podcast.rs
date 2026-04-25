use dioxus::prelude::*;
use crate::server::{list_episodes, get_episode_by_id, Episode};

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

/// Podcast 主页面：从 server 动态加载节目列表
#[component]
pub fn PodcastPage() -> Element {
    let episodes_res = use_resource(move || async move {
        list_episodes().await.unwrap_or_default()
    });

    let episodes = episodes_res.read().as_ref().cloned().unwrap_or_default();
    let mut current_episode = use_signal::<Option<Episode>>(|| None);
    let mut is_playing = use_signal(|| false);

    // 加载完成后默认选中第一个节目
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

    rsx! {
        section { class: "py-12 min-h-screen bg-[var(--color-bg)] transition-colors duration-300",
            LocalContainer {
                LocalSectionTitle {
                    title: "Rust 深度播客".to_string(),
                    subtitle: Some("探索技术边界，聆听思维回响".to_string())
                }

                match (current_episode.read().clone(), episodes.is_empty()) {
                    (None, true) => rsx! {
                        div { class: "flex items-center justify-center py-20",
                            div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                        }
                    },
                    (None, false) => rsx! {
                        div { class: "text-center text-slate-500 py-20", "暂无节目" }
                    },
                    (Some(active), _) => rsx! {
                        div { class: "grid grid-cols-1 lg:grid-cols-3 gap-8 mt-8",
                            // 左侧：当前播放
                            div { class: "lg:col-span-2 space-y-6",
                                div { class: "sticky top-24",
                                    div { class: "relative overflow-hidden rounded-2xl bg-slate-900 shadow-xl border border-[var(--color-border)]",
                                        div { class: "relative p-8 md:p-10",
                                            if let Some(ref guest) = active.guest {
                                                div { class: "text-xs font-bold text-blue-400 uppercase tracking-widest mb-2",
                                                    "嘉宾: {guest}"
                                                }
                                            }
                                            h2 { class: "mt-2 text-2xl md:text-3xl font-bold tracking-tight text-white", "{active.title}" }
                                            p { class: "mt-4 text-base md:text-lg leading-relaxed text-slate-300", "{active.description}" }
                                            div { class: "mt-3 flex items-center gap-3 text-sm text-slate-400",
                                                span { "{active.date}" }
                                                span { "·" }
                                                span { "{active.duration}" }
                                            }
                                            if !active.tags.is_empty() {
                                                div { class: "mt-3 flex flex-wrap gap-2",
                                                    for tag in active.tags.iter() {
                                                        span { class: "text-xs px-2 py-0.5 rounded-full bg-slate-800 text-slate-300", "#{tag}" }
                                                    }
                                                }
                                            }
                                            div { class: "mt-8",
                                                audio {
                                                    class: "w-full focus:outline-none",
                                                    controls: true,
                                                    autoplay: true,
                                                    src: "{active.url}",
                                                    onplay: move |_| is_playing.set(true),
                                                    onpause: move |_| is_playing.set(false),
                                                    "Your browser does not support the audio element."
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 右侧：节目列表
                            div { class: "space-y-4",
                                h3 { class: "text-lg font-semibold text-[var(--color-text)] px-1", "更多节目" }
                                div { class: "space-y-3",
                                    for episode in episodes.iter() {
                                        {
                                            let ep_clone = episode.clone();
                                            let active_id = active.id;
                                            rsx! {
                                                button {
                                                    key: "{episode.id}",
                                                    onclick: move |_| current_episode.set(Some(ep_clone.clone())),
                                                    class: format_args!(
                                                        "w-full text-left group relative flex items-start gap-4 rounded-xl p-4 transition-all {}",
                                                        if active_id == episode.id { "bg-blue-50 dark:bg-blue-900/20 ring-1 ring-blue-200 dark:ring-blue-800" } else { "hover:bg-slate-50 dark:hover:bg-slate-900/50 border border-slate-200 dark:border-slate-800" }
                                                    ),
                                                    div { class: "flex-auto min-w-0",
                                                        h4 { class: "text-sm font-medium line-clamp-2 text-[var(--color-text)]", "{episode.title}" }
                                                        div { class: "mt-1 flex items-center gap-2 text-xs text-slate-500",
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
