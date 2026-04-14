use dioxus::prelude::*;
use pulldown_cmark::{Options, Parser, Event, Tag, CodeBlockKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rustineverything_module_podcast::podcast::{Episode, EPISODES};
use pulldown_latex::{Parser as LatexParser, Storage, push_mathml};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PostMetadata {
    pub title: String,
    pub description: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Props, Clone, PartialEq)]
pub struct MarkdownProps {
    pub content: String,
    pub blog_id: String, // 传入当前文章 ID，用于处理相对路径
}

pub fn parse_mdx(content: &str) -> (PostMetadata, String) {
    if !content.starts_with("---") {
        return (PostMetadata::default(), content.to_string());
    }
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return (PostMetadata::default(), content.to_string());
    }
    let metadata: PostMetadata = serde_yaml::from_str(parts[1]).unwrap_or_default();
    let body = parts[2].trim().to_string();
    (metadata, body)
}

#[component]
pub fn Markdown(props: MarkdownProps) -> Element {
    let (metadata, body) = parse_mdx(&props.content);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(&body, options);
    let mut it = parser.peekable();
    
    // 渲染流，传入 blog_id
    let elements = render_stream(&mut it, &props.blog_id);

    use_effect(move || {
        // 轮询等待 Prism 和语言包加载完成
        dioxus::document::eval(r#"(function check(){if(window.Prism&&Prism.languages.rust){Prism.highlightAll()}else{setTimeout(check,100)}})()"#);
    });

    rsx! {
        document::Title { "{metadata.title}" }
        document::Style { "
            math {{ font-size: 1.1em; }}
            .math-display math {{ font-size: 1.4em; }}
            .prose code::before, .prose code::after {{ content: none !important; }}
        " }
        
        div { class: "prose prose-slate dark:prose-invert max-w-none",
            {elements.into_iter()}
        }
    }
}

fn render_stream<'a>(it: &mut std::iter::Peekable<Parser<'a>>, blog_id: &str) -> Vec<Element> {
    let mut nodes = Vec::new();

    while let Some(event) = it.next() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                // 特殊处理代码块：收集原始文本用于 Copy 按钮
                let lang = match &kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    _ => "text".to_string(),
                };
                let mut code_text = String::new();
                loop {
                    match it.next() {
                        Some(Event::Text(t)) => code_text.push_str(&t),
                        Some(Event::End(_)) => break,
                        None => break,
                        _ => {}
                    }
                }
                nodes.push(render_code_block(lang, code_text));
            }
            Event::Start(tag) => {
                let children = render_stream(it, blog_id);
                nodes.push(render_tag(tag, children, blog_id));
            }
            Event::End(_) => break,
            Event::Text(text) => nodes.push(rsx! { "{text}" }),
            Event::Code(code) => nodes.push(rsx! {
                code { class: "bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded text-sm font-mono text-pink-600 dark:text-pink-400", "{code}" }
            }),
            Event::InlineMath(math) => {
                let mathml = latex_to_mathml_string(&math, false);
                nodes.push(rsx! { span { class: "math-inline mx-1", dangerous_inner_html: "{mathml}" } });
            },
            Event::DisplayMath(math) => {
                let mathml = latex_to_mathml_string(&math, true);
                nodes.push(rsx! { 
                    div { class: "math-display flex justify-center my-10 overflow-x-auto py-4 bg-slate-50/50 dark:bg-slate-900/30 rounded-2xl border border-slate-100 dark:border-slate-800", 
                        div { class: "px-6", dangerous_inner_html: "{mathml}" }
                    } 
                });
            },
            Event::SoftBreak => nodes.push(rsx! { " " }),
            Event::HardBreak => nodes.push(rsx! { br {} }),
            Event::Rule => nodes.push(rsx! { hr { class: "my-8 border-slate-200 dark:border-slate-800" } }),
            Event::Html(html) | Event::InlineHtml(html) => {
                let h = html.trim();
                if h.starts_with("<") {
                    if let Some(component) = render_mdx_registry(h) {
                        nodes.push(component);
                        continue;
                    }
                }
                nodes.push(rsx! { span { dangerous_inner_html: "{h}" } });
            }
            _ => {}
        }
    }
    nodes
}

fn render_tag(tag: Tag, children: Vec<Element>, blog_id: &str) -> Element {
    match tag {
        Tag::Heading { level, .. } => {
            let l = level as u32;
            match l {
                1 => rsx! { h1 { {children.into_iter()} } },
                2 => rsx! { h2 { {children.into_iter()} } },
                _ => rsx! { h3 { {children.into_iter()} } },
            }
        }
        Tag::Paragraph => {
            if children.len() == 1 {
                return children.into_iter().next().unwrap();
            }
            rsx! { p { {children.into_iter()} } }
        },
        Tag::List(None) => rsx! { ul { class: "list-disc ml-6 my-4", {children.into_iter()} } },
        Tag::List(Some(start)) => rsx! { ol { start: "{start}", class: "list-decimal ml-6 my-4", {children.into_iter()} } },
        Tag::Item => rsx! { li { class: "mb-1", {children.into_iter()} } },
        Tag::CodeBlock(_) => {
            // 已在 render_stream 中特殊处理，此分支不应触达
            rsx! { pre { {children.into_iter()} } }
        }
        Tag::Link { dest_url, .. } => rsx! { a { href: "{dest_url}", class: "text-blue-600 dark:text-blue-400 underline decoration-blue-500/30 hover:decoration-blue-500 transition-all", {children.into_iter()} } },
        
        // --- 核心：处理图片相对路径 ---
        Tag::Image { dest_url, title, .. } => {
            let url = dest_url.to_string();
            let src = if url.starts_with("http") || url.starts_with("/") {
                url 
            } else {
                // 处理 ID 为 "1" 的特殊情况，映射到 welcome 目录
                let folder = if blog_id == "1" { "welcome" } else { blog_id };
                format!("/posts/{}/{}", folder, url)
            };
            rsx! {
                figure { class: "my-8",
                    img { src: "{src}", class: "rounded-2xl shadow-xl mx-auto border border-slate-200 dark:border-slate-800" }
                    if !title.is_empty() {
                        figcaption { class: "text-center text-sm text-slate-500 mt-3 italic", "{title}" }
                    }
                }
            }
        }
        Tag::BlockQuote(_) => rsx! { blockquote { class: "border-l-4 border-blue-500 bg-blue-50/50 dark:bg-blue-900/10 py-2 pl-6 pr-4 italic my-6 rounded-r-lg", {children.into_iter()} } },
        _ => rsx! { span { {children.into_iter()} } },
    }
}

fn render_mdx_registry(html: &str) -> Option<Element> {
    let clean_html = html.trim();
    if clean_html.contains("<PodcastCard") {
        let id = extract_attr(clean_html, "id")?.parse::<i32>().ok()?;
        if let Some(episode) = EPISODES.iter().find(|e| e.id == id) {
            return Some(rsx! {
                div { class: "not-prose my-8 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-sm flex flex-col md:flex-row gap-6 items-center",
                    div { class: "flex-1 w-full",
                        div { class: "text-xs font-bold text-blue-600 uppercase tracking-widest mb-2", "Featured Podcast" }
                        h4 { class: "text-xl font-extrabold text-slate-900 dark:text-white mb-2", "{episode.title}" }
                        div { class: "text-sm text-slate-500 mb-4", "{episode.date} · {episode.duration}" }
                        audio { class: "w-full h-10", controls: true, src: "{episode.url}" }
                    }
                }
            });
        }
    }
    if clean_html.contains("<YouTube") {
        let id = extract_attr(clean_html, "id")?;
        return Some(rsx! {
            div { class: "not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800",
                iframe { class: "w-full h-full", src: "https://www.youtube.com/embed/{id}", allowfullscreen: true }
            }
        });
    }
    if clean_html.contains("<Bilibili") {
        let id = extract_attr(clean_html, "id")?;
        return Some(rsx! {
            div { class: "not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800",
                iframe { class: "w-full h-full border-0", src: "//player.bilibili.com/player.html?bvid={id}&page=1&high_quality=1", allowfullscreen: true }
            }
        });
    }
    None
}

fn render_code_block(lang: String, code_text: String) -> Element {
    let code_for_copy = code_text.clone();
    // 对代码文本进行 HTML 转义，用于 dangerous_inner_html
    let escaped = code_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    rsx! {
        div {
            class: "not-prose relative my-6",
            style: "position:relative",
            button {
                class: "absolute z-10 px-2.5 py-1 text-xs font-medium text-slate-400 bg-slate-800/80 border border-slate-700 rounded-md hover:text-white hover:bg-slate-700 transition-all cursor-pointer",
                style: "position:absolute;right:0.75rem;top:0.75rem",
                onclick: move |_| {
                    let json_str = serde_json::to_string(&code_for_copy).unwrap_or_default();
                    let js = format!("navigator.clipboard.writeText({json}).then(()=>{{let b=document.activeElement;if(b){{b.textContent='Copied!';setTimeout(()=>b.textContent='Copy',1500)}}}})" , json = json_str);
                    dioxus::document::eval(&js);
                },
                "Copy"
            }
            pre { class: "rounded-xl p-4 bg-slate-900 overflow-x-auto shadow-inner",
                code { class: "language-{lang} text-sm text-slate-200", dangerous_inner_html: "{escaped}" }
            }
        }
    }
}

/// 使用 pulldown-latex 将 LaTeX 转换为 MathML
fn latex_to_mathml_string(latex: &str, display: bool) -> String {
    let storage = Storage::new();
    let parser = LatexParser::new(latex, &storage);
    let mut mathml = String::new();
    let config = pulldown_latex::RenderConfig {
        display_mode: if display {
            pulldown_latex::config::DisplayMode::Block
        } else {
            pulldown_latex::config::DisplayMode::Inline
        },
        ..Default::default()
    };
    match push_mathml(&mut mathml, parser, config) {
        Ok(()) => mathml,
        Err(e) => {
            eprintln!("[Math] LaTeX render error: {e}");
            format!("<code>{}</code>", latex.replace('<', "&lt;").replace('>', "&gt;"))
        }
    }
}

fn extract_attr(html: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = html.find(&pattern)? + pattern.len();
    let end = html[start..].find("\"")?;
    Some(html[start..start + end].to_string())
}
