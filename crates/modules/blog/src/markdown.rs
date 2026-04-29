use dioxus::prelude::*;
use pulldown_cmark::{Options, Parser, Event, Tag, CodeBlockKind, BlockQuoteKind};
use serde::{Deserialize, Serialize};
use rustineverything_module_podcast::podcast::PodcastCard;
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
    options.insert(Options::ENABLE_GFM);

    // 预处理：将 :::type 语法转换为 GFM alert 语法
    let body = convert_admonitions(&body);
    let parser = Parser::new_ext(&body, options);
    let mut it = parser.peekable();
    
    // 渲染流，传入 blog_id；同时按顶层块编号注入 data-block-id 供标注定错
    let mut block_idx: usize = 1;
    let elements = render_stream(&mut it, &props.blog_id, &mut block_idx, true);

    use_effect(move || {
        // 轮询等待 Prism 和语言包加载完成，同时触发 Mermaid 渲染
        dioxus::document::eval(r#"(function check(){if(window.Prism&&Prism.languages.rust){Prism.highlightAll()}else{setTimeout(check,100)}})()"#);
        dioxus::document::eval(r#"(function renderMermaid(){if(window.mermaid){mermaid.run({querySelector:'.mermaid'})}else{setTimeout(renderMermaid,200)}})()"#);
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

fn render_stream<'a>(
    it: &mut std::iter::Peekable<Parser<'a>>,
    blog_id: &str,
    block_idx: &mut usize,
    top: bool,
) -> Vec<Element> {
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
                let id = mint_block_id(top, block_idx);
                if lang == "mermaid" {
                    nodes.push(render_mermaid_block(code_text, id));
                } else {
                    nodes.push(render_code_block(lang, code_text, id));
                }
            }
            Event::Start(Tag::TableHead) => {
                // TableHead 内的 cell 需要渲染为 <th> 而非 <td>
                let mut header_cells = Vec::new();
                while let Some(event) = it.next() {
                    match event {
                        Event::Start(Tag::TableCell) => {
                            let cell_children = render_stream(it, blog_id, block_idx, false);
                            header_cells.push(rsx! {
                                th { class: "px-4 py-3 text-left text-sm font-semibold text-slate-700 dark:text-slate-300",
                                    {cell_children.into_iter()}
                                }
                            });
                        }
                        Event::End(_) => break,
                        _ => {}
                    }
                }
                nodes.push(rsx! {
                    thead { class: "bg-slate-50 dark:bg-slate-800/50",
                        tr { {header_cells.into_iter()} }
                    }
                });
            }
            Event::Start(tag) => {
                let id = mint_block_id(top, block_idx);
                let children = render_stream(it, blog_id, block_idx, false);
                nodes.push(render_tag(tag, children, blog_id, id));
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
                    div { class: "math-display flex justify-center my-10 overflow-x-auto py-8 bg-slate-50/50 dark:bg-slate-900/30 rounded-2xl border border-slate-100 dark:border-slate-800", 
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
                nodes.push(rsx! { span { "{h}" } });
            }
            _ => {}
        }
    }
    nodes
}

/// 在顶层块上递增生成 "b{N}" 序号；不在顶层返回 None
fn mint_block_id(top: bool, block_idx: &mut usize) -> Option<String> {
    if !top {
        return None;
    }
    let n = *block_idx;
    *block_idx += 1;
    Some(format!("b{}", n))
}

fn render_tag(tag: Tag, children: Vec<Element>, blog_id: &str, block_id: Option<String>) -> Element {
    let bid = block_id.unwrap_or_default();
    let has_bid = !bid.is_empty();
    match tag {
        Tag::Heading { level, .. } => {
            let l = level as u32;
            match l {
                1 => rsx! { h1 {
                    id: if has_bid { "{bid}" },
                    "data-block-id": if has_bid { "{bid}" },
                    {children.into_iter()}
                } },
                2 => rsx! { h2 {
                    id: if has_bid { "{bid}" },
                    "data-block-id": if has_bid { "{bid}" },
                    {children.into_iter()}
                } },
                _ => rsx! { h3 {
                    id: if has_bid { "{bid}" },
                    "data-block-id": if has_bid { "{bid}" },
                    {children.into_iter()}
                } },
            }
        }
        Tag::Paragraph => {
            // 顶层 paragraph 始终包 <p>以保证块锁点；非顶层且单子节点时保持原优化。
            if !has_bid && children.len() == 1 {
                return children.into_iter().next().unwrap();
            }
            rsx! { p {
                id: if has_bid { "{bid}" },
                "data-block-id": if has_bid { "{bid}" },
                {children.into_iter()}
            } }
        },
        Tag::List(None) => rsx! { ul {
            id: if has_bid { "{bid}" },
            "data-block-id": if has_bid { "{bid}" },
            class: "list-disc ml-6 my-4",
            {children.into_iter()}
        } },
        Tag::List(Some(start)) => rsx! { ol {
            id: if has_bid { "{bid}" },
            "data-block-id": if has_bid { "{bid}" },
            start: "{start}",
            class: "list-decimal ml-6 my-4",
            {children.into_iter()}
        } },
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
        Tag::BlockQuote(kind) => render_blockquote(kind, children, if has_bid { Some(bid.clone()) } else { None }),
        Tag::Table(_) => rsx! {
            div { class: "overflow-x-auto my-6",
                table {
                    id: if has_bid { "{bid}" },
                    "data-block-id": if has_bid { "{bid}" },
                    class: "min-w-full border-collapse",
                    {children.into_iter()}
                }
            }
        },
        Tag::TableRow => rsx! {
            tr { class: "border-b border-slate-200 dark:border-slate-800",
                {children.into_iter()}
            }
        },
        Tag::TableCell => rsx! {
            td { class: "px-4 py-3 text-sm", {children.into_iter()} }
        },
        _ => rsx! { span { {children.into_iter()} } },
    }
}

fn render_mdx_registry(html: &str) -> Option<Element> {
    let clean_html = html.trim();
    if clean_html.contains("<PodcastCard") {
        let id = extract_attr(clean_html, "id")?.parse::<i32>().ok()?;
        return Some(rsx! { PodcastCard { id: id } });
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

    // ── 文字样式组件：<Yellow text="..." /> 等 ──
    if let Some(el) = render_text_style_component(clean_html) {
        return Some(el);
    }

    None
}

/// 将 :::type 语法转换为 GFM alert 语法
fn convert_admonitions(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    let mut in_block = false;

    for line in body.lines() {
        let trimmed = line.trim();
        if !in_block {
            if let Some(kind) = trimmed.strip_prefix(":::").and_then(|rest| {
                let word = rest.trim().split_whitespace().next().unwrap_or("");
                match word.to_lowercase().as_str() {
                    "note" | "info" => Some("NOTE"),
                    "tip" | "success" => Some("TIP"),
                    "important" => Some("IMPORTANT"),
                    "warning" | "warn" => Some("WARNING"),
                    "caution" | "danger" | "error" => Some("CAUTION"),
                    _ if !word.is_empty() => Some("NOTE"), // 未知类型默认为 NOTE
                    _ => None,
                }
            }) {
                result.push_str(&format!("> [!{}]\n", kind));
                in_block = true;
                continue;
            }
        } else if trimmed == ":::" {
            in_block = false;
            result.push('\n');
            continue;
        }

        if in_block {
            result.push_str("> ");
            result.push_str(line);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

fn render_blockquote(
    kind: Option<BlockQuoteKind>,
    children: Vec<Element>,
    block_id: Option<String>,
) -> Element {
    let bid = block_id.unwrap_or_default();
    let has_bid = !bid.is_empty();
    match kind {
        Some(k) => {
            let (icon, label, left_color, border_color, bg_color, text_color) = match k {
                BlockQuoteKind::Note => (
                    "\u{1f4dd}", "NOTE",
                    "border-l-blue-500", "border-blue-200 dark:border-blue-800/60",
                    "bg-blue-50 dark:bg-blue-950/30", "text-blue-600 dark:text-blue-400",
                ),
                BlockQuoteKind::Tip => (
                    "\u{1f4a1}", "TIP",
                    "border-l-green-500", "border-green-200 dark:border-green-800/60",
                    "bg-green-50 dark:bg-green-950/30", "text-green-600 dark:text-green-400",
                ),
                BlockQuoteKind::Important => (
                    "\u{2757}", "IMPORTANT",
                    "border-l-purple-500", "border-purple-200 dark:border-purple-800/60",
                    "bg-purple-50 dark:bg-purple-950/30", "text-purple-600 dark:text-purple-400",
                ),
                BlockQuoteKind::Warning => (
                    "\u{26a0}\u{fe0f}", "WARNING",
                    "border-l-yellow-500", "border-yellow-200 dark:border-yellow-800/60",
                    "bg-yellow-50 dark:bg-yellow-950/30", "text-yellow-600 dark:text-yellow-400",
                ),
                BlockQuoteKind::Caution => (
                    "\u{1f6d1}", "CAUTION",
                    "border-l-red-500", "border-red-200 dark:border-red-800/60",
                    "bg-red-50 dark:bg-red-950/30", "text-red-600 dark:text-red-400",
                ),
            };
            rsx! {
                div {
                    id: if has_bid { "{bid}" },
                    "data-block-id": if has_bid { "{bid}" },
                    class: "not-prose my-6 rounded-lg border {border_color} border-l-4 {left_color} {bg_color} shadow-sm overflow-hidden",
                    div { class: "px-4 pt-3 pb-1 flex items-center gap-2",
                        span { class: "text-base", "{icon}" }
                        span { class: "text-xs font-bold tracking-wide uppercase {text_color}", "{label}" }
                    }
                    div { class: "px-4 pb-3 text-sm text-slate-700 dark:text-slate-300 leading-relaxed",
                        {children.into_iter()}
                    }
                }
            }
        }
        None => rsx! {
            blockquote {
                id: if has_bid { "{bid}" },
                "data-block-id": if has_bid { "{bid}" },
                class: "border-l-4 border-slate-300 dark:border-slate-700 bg-slate-50/50 dark:bg-slate-900/30 py-2 pl-6 pr-4 italic my-6 rounded-r-lg",
                {children.into_iter()}
            }
        },
    }
}

fn render_mermaid_block(code_text: String, block_id: Option<String>) -> Element {
    let bid = block_id.unwrap_or_default();
    let has_bid = !bid.is_empty();
    rsx! {
        div {
            id: if has_bid { "{bid}" },
            "data-block-id": if has_bid { "{bid}" },
            class: "not-prose my-6 flex justify-center overflow-x-auto rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6",
            pre { class: "mermaid", "{code_text}" }
        }
    }
}

fn render_code_block(lang: String, code_text: String, block_id: Option<String>) -> Element {
    let code_for_copy = code_text.clone();
    let bid = block_id.unwrap_or_default();
    let has_bid = !bid.is_empty();

    rsx! {
        div {
            id: if has_bid { "{bid}" },
            "data-block-id": if has_bid { "{bid}" },
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
                code { class: "language-{lang} text-sm text-slate-200", "{code_text}" }
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

/// 文字样式 MDX 组件
/// 用法：<Yellow text="高亮" /> <Green text="通过" /> <Underline text="重点" /> 等
fn render_text_style_component(html: &str) -> Option<Element> {
    // 颜色组件映射 (Mac Preview 标注色系)
    let color_map: &[(&str, &str)] = &[
        ("Yellow",  "#EAB308"),  // yellow-500
        ("Green",   "#22C55E"),  // green-500
        ("Blue",    "#3B82F6"),  // blue-500
        ("Pink",    "#EC4899"),  // pink-500
        ("Purple",  "#A855F7"),  // purple-500
    ];

    for (name, color) in color_map {
        if html.contains(&format!("<{}", name)) {
            let text = extract_attr(html, "text")?;
            let style = format!("color: {}; font-weight: 600", color);
            return Some(rsx! {
                span { style: "{style}", "{text}" }
            });
        }
    }

    // 下划线
    if html.contains("<Underline") {
        let text = extract_attr(html, "text")?;
        return Some(rsx! {
            span { style: "text-decoration: underline; text-decoration-thickness: 2px; text-underline-offset: 3px", "{text}" }
        });
    }

    // 删除线
    if html.contains("<Strikethrough") {
        let text = extract_attr(html, "text")?;
        return Some(rsx! {
            span { style: "text-decoration: line-through; text-decoration-thickness: 2px", "{text}" }
        });
    }

    None
}

fn extract_attr(html: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = html.find(&pattern)? + pattern.len();
    let end = html[start..].find("\"")?;
    Some(html[start..start + end].to_string())
}
