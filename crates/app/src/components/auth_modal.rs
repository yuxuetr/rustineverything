use dioxus::prelude::*;
use dioxus::document::eval;
use rustineverything_core::AuthProviderDisplay;

use crate::i18n::{t, use_i18n, Language};
use crate::server::get_auth_providers;

#[component]
pub fn AuthModal(show: Signal<bool>) -> Element {
    let lang = use_i18n();

    // Fetch available providers from server (plugin-driven)
    let providers = use_resource(move || async move {
        get_auth_providers().await.unwrap_or_default()
    });

    let close_modal = move |_| {
        show.set(false);
    };

    let stop_propagation = move |e: Event<MouseData>| {
        e.stop_propagation();
    };

    if !show() {
        return rsx! {};
    }

    let provider_list = providers.read();
    let provider_list = provider_list.as_ref().cloned().unwrap_or_default();

    rsx! {
        // Full-screen overlay: backdrop + centered flex container
        div {
            class: "fixed inset-0 z-[100] bg-black/50 backdrop-blur-sm flex items-center justify-center p-4",
            style: "margin:0; top:0; left:0; width:100vw; height:100vh;",
            onclick: close_modal,

            // Modal panel
            div {
                class: "relative w-full rounded-2xl bg-white dark:bg-slate-900 shadow-2xl p-8 animate-[fadeInUp_0.2s_ease-out]",
                style: "max-width: 28rem;",
                onclick: stop_propagation,

                // Close button
                button {
                    class: "absolute top-4 right-4 p-1 rounded-lg text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors",
                    onclick: close_modal,
                    svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24",
                        path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M6 18L18 6M6 6l12 12" }
                    }
                }

                // Header
                div { class: "text-center mb-8",
                    h2 { class: "text-2xl font-bold text-slate-900 dark:text-white mb-2",
                        "{t(lang(), \"auth.sign_in\")}"
                    }
                    p { class: "text-sm text-slate-500 dark:text-slate-400",
                        "{t(lang(), \"auth.sign_in_desc\")}"
                    }
                }

                // Provider buttons (dynamic from plugins)
                div { class: "flex flex-col gap-3",
                    if provider_list.is_empty() {
                        p { class: "text-center text-sm text-slate-400 py-4",
                            "Loading..."
                        }
                    }

                    for provider in provider_list.iter() {
                        {render_provider_button(provider, lang())}
                    }
                }

                // Divider
                div { class: "flex items-center gap-3 my-6",
                    div { class: "flex-1 h-px bg-slate-200 dark:bg-slate-700" }
                }

                // Terms
                p { class: "text-center text-xs text-slate-400 dark:text-slate-500",
                    "{t(lang(), \"auth.terms\")}"
                }
            }
        }

        // Animation keyframes
        document::Style { "
            @keyframes fadeInUp {{
                from {{ opacity: 0; transform: translateY(16px) scale(0.98); }}
                to {{ opacity: 1; transform: translateY(0) scale(1); }}
            }}
        " }
    }
}

fn render_provider_button(provider: &AuthProviderDisplay, lang: Language) -> Element {
    let provider_id = provider.provider_id.clone();
    let display_name = provider.display_name.clone();
    let icon_svg = provider.icon_svg.clone();
    let brand_color = provider.brand_color.clone();

    // Determine text color based on brand color brightness
    let text_color = if is_light_color(&brand_color) {
        "rgb(55, 65, 81)"  // gray-700
    } else {
        "white"
    };

    let border = if is_light_color(&brand_color) {
        "border: 1px solid #d1d5db;"
    } else {
        ""
    };

    let btn_style = format!(
        "background-color: {}; color: {}; {}",
        brand_color, text_color, border
    );

    let label = if lang == Language::En {
        format!("{} {}", t(lang, "auth.continue_with"), display_name)
    } else {
        format!("{} {}", display_name, t(lang, "auth.continue_with"))
    };

    rsx! {
        button {
            key: "{provider_id}",
            class: "flex items-center justify-center gap-3 w-full px-4 py-3 rounded-xl text-sm font-semibold transition-all duration-150 cursor-pointer hover:opacity-90",
            style: "{btn_style}",
            onclick: move |_| {
                let provider_id = provider_id.clone();
                spawn(async move {
                    if let Ok(url) = crate::server::get_login_url(provider_id).await {
                        let _ = eval(&format!("window.location.href = '{}'", url));
                    }
                });
            },

            svg {
                class: "w-5 h-5 shrink-0",
                fill: "currentColor",
                view_box: "0 0 24 24",
                path { d: "{icon_svg}" }
            }

            span { "{label}" }
        }
    }
}

/// Simple heuristic to determine if a hex color is "light"
fn is_light_color(hex: &str) -> bool {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return false;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32;
    // Relative luminance
    (0.299 * r + 0.587 * g + 0.114 * b) > 186.0
}
