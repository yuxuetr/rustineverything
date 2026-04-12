use dioxus::prelude::*;

// 模块内部简单的视图包装，减少对外部依赖
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

#[derive(Clone, PartialEq)]
pub struct Episode {
  pub id: i32,
  pub title: &'static str,
  pub desc: &'static str,
  pub duration: &'static str,
  pub date: &'static str,
  pub url: Asset,
}

pub const EPISODES: [Episode; 3] = [
    Episode {
        id: 1,
        title: "神经网络大揭秘：从零件到架构，深度解析AI模型核心原理",
        desc: "深入探讨神经网络的内部工作机制，从基础的神经元模型到复杂的深度学习架构，为你揭开AI核心原理的面纱。",
        url: asset!("/assets/audio/神经网络大揭秘：从零件到架构，深度解析AI模型核心原理.m4a"),
        duration: "24:15",
        date: "2024-01-15",
    },
    Episode {
        id: 2,
        title: "编程范式螺旋上升：C、OOP到Rust的平衡",
        desc: "回顾编程语言的发展历程，探讨从过程式编程到面向对象，再到Rust所代表的现代系统编程范式的演进与平衡。",
        url: asset!("/assets/audio/编程范式螺旋上升：C、OOP到Rust的平衡.m4a"),
        duration: "32:40",
        date: "2024-01-08",
    },
    Episode {
        id: 3,
        title: "用Rust打造工业级AI智能体",
        desc: "实战分享：如何利用Rust的高性能和安全性优势，构建稳定、高效 of 工业级AI Agent system。",
        url: asset!("/assets/audio/用Rust打造工业级AI智能体.m4a"),
        duration: "28:50",
        date: "2024-01-01",
    },
];

#[component]
pub fn PodcastPage() -> Element {
  let mut current_episode = use_signal(|| EPISODES[0].clone());
  let mut is_playing = use_signal(|| false);

  rsx! {
      section { class: "py-12 min-h-screen bg-[var(--color-bg)] transition-colors duration-300",
          LocalContainer {
              LocalSectionTitle {
                  title: "Rust 深度播客".to_string(),
                  subtitle: Some("探索技术边界，聆听思维回响".to_string())
              }

              div { class: "grid grid-cols-1 lg:grid-cols-3 gap-8 mt-8",
                  // ... 其余逻辑保持不变 ...
                  div { class: "lg:col-span-2 space-y-6",
                      div { class: "sticky top-24",
                          div { class: "relative overflow-hidden rounded-2xl bg-slate-900 shadow-xl border border-[var(--color-border)]",
                              div { class: "relative p-8 md:p-10",
                                  h2 { class: "mt-6 text-2xl md:text-3xl font-bold tracking-tight text-white",
                                      "{current_episode().title}"
                                  }
                                  p { class: "mt-4 text-base md:text-lg leading-relaxed text-slate-300",
                                      "{current_episode().desc}"
                                  }
                                  div { class: "mt-8",
                                      audio {
                                          class: "w-full focus:outline-none",
                                          controls: true,
                                          autoplay: true,
                                          src: "{current_episode().url}",
                                          onplay: move |_| is_playing.set(true),
                                          onpause: move |_| is_playing.set(false),
                                          "Your browser does not support the audio element."
                                      }
                                  }
                              }
                          }
                      }
                  }

                  div { class: "space-y-4",
                      h3 { class: "text-lg font-semibold text-[var(--color-text)] px-1", "更多节目" }
                      div { class: "space-y-3",
                          for episode in EPISODES {
                              {
                                  let episode_clone = episode.clone();
                                  rsx! {
                                      button {
                                          onclick: move |_| current_episode.set(episode_clone.clone()),
                                          class: format_args!(
                                              "w-full text-left group relative flex items-start gap-4 rounded-xl p-4 transition-all {}",
                                              if current_episode().id == episode.id {
                                                  "bg-blue-50 dark:bg-blue-900/20 ring-1 ring-blue-200 dark:ring-blue-800"
                                              } else {
                                                  "hover:bg-slate-50 dark:hover:bg-slate-900/50 border border-slate-200 dark:border-slate-800"
                                              }
                                          ),
                                          div { class: "flex-none mt-1",
                                              div { class: format_args!(
                                                  "flex h-10 w-10 items-center justify-center rounded-full {}",
                                                  if current_episode().id == episode.id {
                                                      "bg-blue-600 text-white shadow-md shadow-blue-500/20"
                                                  } else {
                                                      "bg-slate-100 dark:bg-slate-800 text-slate-500 group-hover:bg-blue-600 group-hover:text-white transition-colors"
                                                  }
                                              ),
                                                  if current_episode().id == episode.id && is_playing() {
                                                      svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M10 9v6m4-6v6" } }
                                                  } else {
                                                      svg { class: "w-5 h-5 ml-0.5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" } }
                                                  }
                                              }
                                          }
                                          div { class: "flex-auto min-w-0",
                                              h4 { class: format_args!(
                                                  "text-sm font-medium line-clamp-2 {}",
                                                  if current_episode().id == episode.id {
                                                      "text-blue-700 dark:text-blue-300"
                                                  } else {
                                                      "text-slate-700 dark:text-slate-300 group-hover:text-blue-600 dark:group-hover:text-blue-400"
                                                  }
                                              ),
                                                  "{episode.title}"
                                              }
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
          }
      }
  }
}
