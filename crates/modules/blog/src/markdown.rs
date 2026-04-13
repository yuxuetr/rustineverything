use dioxus::prelude::*;
use pulldown_cmark::{Options, Parser, Event, Tag, CodeBlockKind};
use std::collections::HashMap;

#[derive(Props, Clone, PartialEq)]
pub struct MarkdownProps {
    pub content: String,
}

/// 解析 Markdown 元数据 (Frontmatter)
pub fn parse_markdown_metadata(content: &str) -> (HashMap<String, String>, String) {
    let mut metadata = HashMap::new();
    let mut body = String::new();
    let mut in_metadata = false;

    for line in content.lines() {
        if line == "---" {
            in_metadata = !in_metadata;
            continue;
        }
        if in_metadata {
            if let Some((key, value)) = line.split_once(':') {
                metadata.insert(key.trim().to_string(), value.trim().to_string());
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    (metadata, body)
}

#[component]
pub fn Markdown(props: MarkdownProps) -> Element {
    let (_, body) = parse_markdown_metadata(&props.content);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(&body, options);
    let mut it = parser.peekable();
    let elements = render_stream(&mut it);

    use_effect(move || {
        dioxus::document::eval("if (window.Prism) Prism.highlightAll();");
    });

    rsx! {
        // 关键：通过 Style 标签强制去除 prose 默认给 code 添加的反引号
        document::Style {
            "
            .prose code::before {{ content: none !important; }}
            .prose code::after {{ content: none !important; }}
            "
        }
        div { class: "prose prose-slate dark:prose-invert max-w-none 
                    prose-pre:bg-slate-900 prose-pre:p-0 prose-pre:rounded-lg",
            {elements.into_iter()}
        }
    }
}

fn render_stream<'a>(it: &mut std::iter::Peekable<Parser<'a>>) -> Vec<Element> {
    let mut nodes = Vec::new();

    while let Some(event) = it.next() {
        match event {
            Event::Start(tag) => {
                let children = render_stream(it);
                nodes.push(render_tag(tag, children));
            }
            Event::End(_) => break,
            Event::Text(text) => nodes.push(rsx! { "{text}" }),
            Event::Code(code) => nodes.push(rsx! {
                code { 
                    class: "bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded text-sm font-mono text-pink-600 dark:text-pink-400",
                    "{code}" 
                }
            }),
            Event::InlineMath(math) => nodes.push(rsx! {
                span { class: "math math-inline bg-blue-50 dark:bg-blue-900/20 px-1 rounded font-mono", "{math}" }
            }),
            Event::DisplayMath(math) => nodes.push(rsx! {
                div { class: "math math-display my-4 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg text-center font-mono", "{math}" }
            }),
            Event::SoftBreak => nodes.push(rsx! { " " }),
            Event::HardBreak => nodes.push(rsx! { br {} }),
            Event::Rule => nodes.push(rsx! { hr { class: "my-8 border-slate-200 dark:border-slate-800" } }),
            Event::Html(html) => nodes.push(rsx! { div { dangerous_inner_html: "{html}" } }),
            _ => {}
        }
    }
    nodes
}

fn render_tag(tag: Tag, children: Vec<Element>) -> Element {
    match tag {
        Tag::Heading { level, .. } => {
            let l = level as u32;
            match l {
                1 => rsx! { h1 { {children.into_iter()} } },
                2 => rsx! { h2 { {children.into_iter()} } },
                _ => rsx! { h3 { {children.into_iter()} } },
            }
        }
        Tag::Paragraph => rsx! { p { {children.into_iter()} } },
        Tag::List(None) => rsx! { ul { class: "list-disc ml-6 my-4", {children.into_iter()} } },
        Tag::List(Some(start)) => rsx! { ol { start: "{start}", class: "list-decimal ml-6 my-4", {children.into_iter()} } },
        Tag::Item => rsx! { li { class: "mb-1", {children.into_iter()} } },
        Tag::CodeBlock(kind) => {
            let lang = match kind {
                CodeBlockKind::Fenced(l) => l.to_string(),
                _ => "text".to_string(),
            };
            rsx! {
                pre { class: "rounded-lg overflow-hidden",
                    code { class: "language-{lang}", {children.into_iter()} }
                }
            }
        }
        Tag::Link { dest_url, .. } => {
            let url = dest_url.to_string();
            if let Some(media) = render_media_if_detected(&url) {
                media
            } else {
                rsx! { a { href: "{url}", class: "text-blue-600 dark:text-blue-400 underline", {children.into_iter()} } }
            }
        }
        Tag::Image { dest_url, title, .. } => {
            let url = dest_url.to_string();
            if let Some(media) = render_media_if_detected(&url) {
                media
            } else {
                rsx! { img { src: "{url}", title: "{title}", class: "rounded-xl shadow-md mx-auto my-6" } }
            }
        }
        Tag::BlockQuote(_) => rsx! { blockquote { class: "border-l-4 border-slate-300 dark:border-slate-700 pl-4 italic my-4", {children.into_iter()} } },
        Tag::Table(_) => rsx! { table { class: "min-w-full divide-y divide-slate-300 dark:divide-slate-700 my-6", {children.into_iter()} } },
        Tag::TableHead => rsx! { thead { class: "bg-slate-50 dark:bg-slate-900", {children.into_iter()} } },
        Tag::TableRow => rsx! { tr { {children.into_iter()} } },
        Tag::TableCell => rsx! { td { class: "px-3 py-2 text-sm", {children.into_iter()} } },
        _ => rsx! { span { {children.into_iter()} } },
    }
}

fn render_media_if_detected(url: &str) -> Option<Element> {
    if url.contains("youtube.com/watch") || url.contains("youtu.be") {
        let video_id = url.split('=').last().unwrap_or(url).split('/').last().unwrap_or(url);
        return Some(rsx! {
            div { class: "aspect-video my-6 overflow-hidden rounded-xl shadow-lg",
                iframe {
                    class: "w-full h-full",
                    src: "https://www.youtube.com/embed/{video_id}",
                    allow: "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture",
                    allowfullscreen: true
                }
            }
        });
    }
    if url.contains("bilibili.com/video") {
        let bvid = url.split('/').last().unwrap_or(url).split('?').next().unwrap_or(url);
        return Some(rsx! {
            div { class: "aspect-video my-6 overflow-hidden rounded-xl shadow-lg",
                iframe {
                    class: "w-full h-full border-0",
                    src: "//player.bilibili.com/player.html?bvid={bvid}&page=1&high_quality=1",
                    allowfullscreen: true
                }
            }
        });
    }
    if url.ends_with(".mp4") || url.ends_with(".webm") {
        Some(rsx! { video { class: "w-full rounded-xl my-6 shadow-md", controls: true, src: "{url}" } })
    } else if url.ends_with(".mp3") || url.ends_with(".m4a") || url.ends_with(".wav") {
        Some(rsx! { audio { class: "w-full my-4", controls: true, src: "{url}" } })
    } else {
        None
    }
}
