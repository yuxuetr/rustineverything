//! Phase 3.1：ThemePicker — Navbar 中的主题切换下拉。
//!
//! 通过 `list_available_themes` server fn 拉取插件目录下的可用主题，
//! 点击后调用 `set_user_theme` 写入 cookie 并 bump `ThemeVersion` Signal，
//! 触发上层 `theme_css` `use_resource` 重新请求合并 CSS。

use dioxus::prelude::*;

use crate::server::{list_available_themes, set_user_theme, ThemeInfo};

/// 全局主题版本号：每次切换主题 +1，App 组件中的 `use_resource` 依赖该值，
/// 使聚合 CSS 在用户切换主题后立即重取。
#[derive(Clone, Copy)]
pub struct ThemeVersion(pub Signal<u32>);

/// 在 App 根上挂载 `ThemeVersion` context，返回 Signal 以便上层订阅。
pub fn use_theme_version_provider() -> Signal<u32> {
    let sig = use_signal(|| 0u32);
    use_context_provider(|| ThemeVersion(sig));
    sig
}

/// 子组件读取当前 ThemeVersion Signal。
pub fn use_theme_version() -> Signal<u32> {
    use_context::<ThemeVersion>().0
}

/// 主题下拉。展示当前激活主题，点击展开列表，点击列表项写 cookie + bump version。
#[component]
pub fn ThemePicker() -> Element {
    let mut open = use_signal(|| false);
    let mut version = use_theme_version();

    // 拉取可用主题列表，依赖 version 以便切换后刷新激活态。
    let themes_res = use_resource(move || {
        let _v = version();
        async move { list_available_themes().await.unwrap_or_default() }
    });
    let themes: Vec<ThemeInfo> = themes_res.read().as_ref().cloned().unwrap_or_default();

    // 找到当前激活主题用于按钮 label
    let active_label = themes
        .iter()
        .find(|t| t.is_active)
        .map(|t| t.label.clone())
        .unwrap_or_else(|| "主题".to_string());

    // 不显示 picker 当只有 ≤ 1 个主题（无切换意义）
    if themes.len() <= 1 {
        return rsx! {};
    }

    // 在闭包中调用上衔函数：写 cookie 后 bump version 以迫使上层重拼 CSS。
    // 该闭包被后续多个 button.onclick 共享使用，所以需要 Copy + impl Fn。
    let switch = use_callback(move |filename: String| {
        spawn(async move {
            match set_user_theme(filename).await {
                Ok(_) => {
                    version.with_mut(|v| *v += 1);
                }
                Err(e) => {
                    tracing::error!(error = %e, "theme picker: set_user_theme failed");
                }
            }
        });
        open.set(false);
    });

    rsx! {
        div { class: "relative",
            button {
                onclick: move |_| open.set(!open()),
                class: "flex items-center gap-1 px-2 py-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 transition-colors text-xs font-semibold",
                title: "切换主题",
                svg {
                    class: "w-4 h-4",
                    fill: "none",
                    stroke: "currentColor",
                    view_box: "0 0 24 24",
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        stroke_width: "2",
                        d: "M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01"
                    }
                }
                span { class: "hidden sm:inline", "{active_label}" }
            }
            if open() {
                div { class: "absolute right-0 top-full mt-1 w-48 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-lg py-1 z-50",
                    div { class: "px-3 py-1.5 text-[10px] uppercase tracking-wider text-slate-400",
                        "主题"
                    }
                    for t in themes.iter() {
                        {
                            let is_active = t.is_active;
                            let filename = t.filename.clone();
                            let label = t.label.clone();
                            rsx! {
                                button {
                                    key: "{filename}",
                                    onclick: move |_| switch.call(filename.clone()),
                                    class: format_args!(
                                        "w-full text-left px-3 py-1.5 text-sm transition-colors flex items-center justify-between {}",
                                        if is_active {
                                            "text-blue-600 dark:text-blue-400 font-semibold bg-blue-50 dark:bg-blue-900/30"
                                        } else {
                                            "text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                                        }
                                    ),
                                    span { "{label}" }
                                    if is_active {
                                        span { class: "text-xs", "✓" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "my-1 border-t border-slate-100 dark:border-slate-800" }
                    button {
                        onclick: move |_| switch.call(String::new()),
                        class: "w-full text-left px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                        "重置为默认"
                    }
                }
            }
        }
    }
}
