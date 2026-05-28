//! Wasm 板块的落地页与文章详情页。导航用 `<a href>`，避免对 app `Route` 的循环依赖。

use dioxus::prelude::*;
use rustineverything_widgets::{parse_mdx, Markdown};

use crate::server::{get_wasm_article, list_wasm_articles, ArticleSummary};
use crate::text::{
    matches_query, subtopic_label, BOARD_LABEL, BOARD_ROUTE, BOARD_TAGLINE, FEATURED_CRATES,
    SUBTOPICS,
};

#[component]
pub fn WasmIndexPage() -> Element {
    let articles_res = use_resource(|| async move { list_wasm_articles().await.unwrap_or_default() });
    let articles: Vec<ArticleSummary> = articles_res.read().as_ref().cloned().unwrap_or_default();

    let mut active_subtopic = use_signal(String::new);
    let mut query = use_signal(String::new);

    let q = query();
    let sub = active_subtopic();
    let filtered: Vec<ArticleSummary> = articles
        .iter()
        .filter(|a| sub.is_empty() || a.subtopic == sub)
        .filter(|a| matches_query(&a.title, &a.description, &a.tags, &q))
        .cloned()
        .collect();

    rsx! {
        section { class: "py-12 bg-white dark:bg-slate-950",
            div { class: "max-w-6xl mx-auto px-4 sm:px-6",
                div { class: "mb-10",
                    h1 { class: "text-3xl sm:text-4xl font-extrabold tracking-tight text-slate-900 dark:text-white", "{BOARD_LABEL}" }
                    p { class: "mt-3 text-lg text-slate-500 dark:text-slate-400 max-w-2xl", "{BOARD_TAGLINE}" }
                }

                div { class: "mb-6",
                    input {
                        r#type: "search",
                        class: "w-full max-w-md px-4 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white",
                        placeholder: "搜索文章 / 标签…",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }

                div { class: "flex flex-wrap gap-2 mb-8",
                    button {
                        class: if sub.is_empty() {
                            "px-3 py-1.5 rounded-full text-sm font-semibold bg-blue-600 text-white"
                        } else {
                            "px-3 py-1.5 rounded-full text-sm font-medium bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700"
                        },
                        onclick: move |_| active_subtopic.set(String::new()),
                        "全部"
                    }
                    for s in SUBTOPICS.iter() {
                        {
                            let slug = s.slug.to_string();
                            let is_active = sub == s.slug;
                            rsx! {
                                button {
                                    class: if is_active {
                                        "px-3 py-1.5 rounded-full text-sm font-semibold bg-blue-600 text-white"
                                    } else {
                                        "px-3 py-1.5 rounded-full text-sm font-medium bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700"
                                    },
                                    title: "{s.blurb}",
                                    onclick: move |_| active_subtopic.set(slug.clone()),
                                    "{s.label}"
                                }
                            }
                        }
                    }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-3 gap-8",
                    div { class: "lg:col-span-2",
                        match articles_res.read().as_ref() {
                            None => rsx! {
                                div { class: "flex items-center justify-center py-20",
                                    div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                                }
                            },
                            Some(_) if filtered.is_empty() => rsx! {
                                div { class: "py-16 text-center text-slate-400",
                                    "暂无文章。把 markdown 放到 assets/topics/wasm/<slug>/index.md 即可。"
                                }
                            },
                            Some(_) => rsx! {
                                div { class: "space-y-4",
                                    for a in filtered.iter() {
                                        ArticleCard { key: "{a.slug}", article: a.clone() }
                                    }
                                }
                            },
                        }
                    }

                    aside {
                        h2 { class: "text-sm font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 mb-4", "精选 crate" }
                        div { class: "space-y-3",
                            for c in FEATURED_CRATES.iter() {
                                a {
                                    href: "{c.url}",
                                    target: "_blank",
                                    rel: "noopener noreferrer",
                                    class: "block p-3 rounded-lg border border-slate-200 dark:border-slate-800 hover:border-blue-400 dark:hover:border-blue-600 transition-colors",
                                    div { class: "font-mono text-sm font-bold text-slate-900 dark:text-white", "{c.name}" }
                                    div { class: "text-xs text-slate-500 dark:text-slate-400 mt-0.5", "{c.blurb}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ArticleCard(article: ArticleSummary) -> Element {
    let href = format!("{}/{}", BOARD_ROUTE, article.slug);
    let sub = subtopic_label(&article.subtopic).unwrap_or("");
    rsx! {
        a {
            href: "{href}",
            class: "block p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/40 hover:shadow-md hover:border-blue-300 dark:hover:border-blue-700 transition-all",
            div { class: "flex items-center gap-2 mb-2 text-xs",
                if !sub.is_empty() {
                    span { class: "px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 font-medium", "{sub}" }
                }
                span { class: "text-slate-400", "{article.date}" }
            }
            h3 { class: "text-lg font-bold text-slate-900 dark:text-white", "{article.title}" }
            p { class: "mt-1 text-sm text-slate-600 dark:text-slate-400", "{article.description}" }
            if !article.tags.is_empty() {
                div { class: "mt-3 flex flex-wrap gap-1.5",
                    for tag in article.tags.iter() {
                        span { class: "text-xs px-2 py-0.5 rounded bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400", "#{tag}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn WasmArticlePage(slug: String) -> Element {
    let slug_for_res = slug.clone();
    let content_res = use_resource(move || {
        let s = slug_for_res.clone();
        async move { get_wasm_article(s).await }
    });

    rsx! {
        section { class: "py-12 bg-white dark:bg-slate-950",
            div { class: "max-w-4xl mx-auto px-4 sm:px-6",
                a {
                    href: "{BOARD_ROUTE}",
                    class: "inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-700 mb-8",
                    "← 返回 {BOARD_LABEL}"
                }
                div { class: "text-slate-700 dark:text-slate-200",
                    match content_res.read().as_ref() {
                        Some(Ok(content)) => {
                            let (_meta, _body) = parse_mdx(content);
                            rsx! {
                                Markdown { content: content.clone(), blog_id: slug.clone() }
                            }
                        }
                        Some(Err(e)) => rsx! {
                            div { class: "p-4 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 rounded-lg", "加载失败：{e}" }
                        },
                        None => rsx! {
                            div { class: "flex items-center justify-center py-20",
                                div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
                            }
                        },
                    }
                }
            }
        }
    }
}
