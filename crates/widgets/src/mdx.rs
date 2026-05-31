//! MDX 渲染管道（Phase 2.1 从 `crates/modules/blog/src/markdown.rs` 迁移过来）。
//!
//! 与原版的差异：
//! 1. 不再直接 `use module_podcast::podcast::PodcastCard`，
//!    改为通过 [`crate::registry`] 查表渲染，避免 widgets crate 依赖具体业务模块。
//! 2. 其余渲染逻辑（GFM / 数学 / Mermaid / 代码块 + Copy / 颜色组件 / 标注 block-id）
//!    保持与原版一致，welcome 示例和现有快照仍可像素级回归。

use dioxus::prelude::*;
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag};
use pulldown_latex::{push_mathml, Parser as LatexParser, Storage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MDX frontmatter 元数据。
///
/// Phase 2.3 扩展：增加 image / author / canonical / date / tags
/// 5 个字段以驱动 SEO 注入。旧字段 (title / description / keywords)
/// 保持不变，以便存量 MDX frontmatter 无需修改即可加载。
///
/// 所有新字段都是 Optional，反序列化 frontmatter 时走 default，
/// 不会因为缺失而报错。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PostMetadata {
  pub title: String,
  pub description: Option<String>,
  pub keywords: Option<String>,
  /// 社交分享封面。推荐是绝对 URL；相对路径也允许，调用方负责拼接。
  #[serde(default)]
  pub image: Option<String>,
  /// 作者。JSON-LD 中作为 `author.name` 用。
  #[serde(default)]
  pub author: Option<String>,
  /// 手动覆盖 canonical URL；不提供时 [`crate::seo::inject_seo`]
  /// 从 `base_url + path` 自动推导。
  #[serde(default)]
  pub canonical: Option<String>,
  /// ISO 8601 发布日期字符串（如 `2024-01-15`）。JSON-LD 中
  /// 填入 `datePublished`。未提供时不输出该字段。
  #[serde(default)]
  pub date: Option<String>,
  /// 文章标签。JSON-LD `keywords` + Open Graph `article:tag`。
  #[serde(default)]
  pub tags: Vec<String>,
}

#[derive(Props, Clone, PartialEq)]
pub struct MarkdownProps {
  pub content: String,
  /// 当前文章 / 章节 / 案例 ID，用于解析图片相对路径。
  pub blog_id: String,
  /// Phase 4.2：来自用户的不受信内容（评论 / 话题 / 标注 等）。
  /// 默认 `false`（站点作者内容，原样渲染）。`true` 时会在 cmark
  /// 解析前调用 [`crate::sanitize::sanitize_user_html`] 剥离危险标签。
  #[props(default = false)]
  pub untrusted: bool,
}

/// 解析 frontmatter + 正文。frontmatter 为可选 YAML（`---` 分隔）。
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
  // Phase 4.2：用户内容先经 sanitize_user_html 剥离危险 HTML，
  // 站点作者内容保持原状（信任站点编辑者）。
  let raw_content = if props.untrusted {
    crate::sanitize::sanitize_user_html(&props.content)
  } else {
    props.content.clone()
  };
  let (metadata, body) = parse_mdx(&raw_content);

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

  // 渲染流，传入 blog_id；同时按顶层块编号注入 data-block-id 供标注定位
  let mut block_idx: usize = 1;
  let elements = render_stream(&mut it, &props.blog_id, &mut block_idx, true);

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

      // Phase 1A.5：hydration-safe 触发器。用 `document::Script` 而不是
      // `dioxus::document::eval`：前者声明式渲染 <script> 节点，SSR 时进
      // HTML 流由浏览器原生执行；客户端再渲染时 Dioxus 重新挂载该节点同样
      // 触发执行；desktop / mobile 后端则把它作为普通节点忽略，不再依赖
      // 仅 web 可用的 `eval` JS 注入。轮询是为了等 `/js/prism.min.js`
      // 与 `/js/mermaid.min.js`（由 main.rs 的 document::Script 加载）就绪。
      document::Script { {MARKDOWN_REHIGHLIGHT_SCRIPT} }
  }
}

/// Phase 1A.5：每次 Markdown 组件挂载时执行，等待 Prism / Mermaid 全局
/// 对象就绪后触发高亮与 Mermaid 图渲染。轮询步长 100 / 200ms，避免主线程
/// 阻塞；Prism / Mermaid 的 `run` 都是幂等的，重复触发安全。
const MARKDOWN_REHIGHLIGHT_SCRIPT: &str = r#"
(function rehighlight(){
  if(window.Prism && window.Prism.languages && window.Prism.languages.rust){
    window.Prism.highlightAll();
  } else {
    setTimeout(rehighlight, 100);
  }
})();
(function rerunMermaid(){
  if(window.mermaid && typeof window.mermaid.run === 'function'){
    try { window.mermaid.run({querySelector: '.mermaid'}); } catch(e) {}
  } else {
    setTimeout(rerunMermaid, 200);
  }
})();
"#;

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
            }
            Event::DisplayMath(math) => {
                let mathml = latex_to_mathml_string(&math, true);
                nodes.push(rsx! {
                    div { class: "math-display flex justify-center my-10 overflow-x-auto py-8 bg-slate-50/50 dark:bg-slate-900/30 rounded-2xl border border-slate-100 dark:border-slate-800",
                        div { class: "px-6", dangerous_inner_html: "{mathml}" }
                    }
                });
            }
            Event::SoftBreak => nodes.push(rsx! { " " }),
            Event::HardBreak => nodes.push(rsx! { br {} }),
            Event::Rule => nodes.push(rsx! { hr { class: "my-8 border-slate-200 dark:border-slate-800" } }),
            Event::Html(html) | Event::InlineHtml(html) => {
                let h = html.trim();
                if h.starts_with('<') {
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

fn render_tag(
  tag: Tag,
  children: Vec<Element>,
  blog_id: &str,
  block_id: Option<String>,
) -> Element {
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
        return children.into_iter().next().unwrap_or_else(|| rsx! { span {} });
      }
      rsx! { p {
          id: if has_bid { "{bid}" },
          "data-block-id": if has_bid { "{bid}" },
          {children.into_iter()}
      } }
    }
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
    Tag::Link { dest_url, .. } => {
      rsx! { a { href: "{dest_url}", class: "text-blue-600 dark:text-blue-400 underline decoration-blue-500/30 hover:decoration-blue-500 transition-all", {children.into_iter()} } }
    }

    // --- 核心：处理图片相对路径 ---
    Tag::Image { dest_url, title, .. } => {
      let url = dest_url.to_string();
      let src = if url.starts_with("http") || url.starts_with('/') {
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
    Tag::BlockQuote(kind) => {
      render_blockquote(kind, children, if has_bid { Some(bid.clone()) } else { None })
    }
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

/// Phase 2.2: 纯注册表查询。内置 9 个组件（YouTube / Bilibili / 五色
/// 高亮 / Underline / Strikethrough）由 [`crate::components::register_default_components`]
/// 在启动期预注册；podcast 等业务模块走自己的 `register_components`。
/// 未注册的标签返回 None，调用方（`render_stream`）会降级为
/// 占位 span，保证整篇文章仍可渲染。
fn render_mdx_registry(html: &str) -> Option<Element> {
  let clean_html = html.trim();
  let name = detect_registered_tag(clean_html)?;
  let attrs = parse_attrs(clean_html);
  crate::registry::render(&name, &attrs)
}

/// 从 `<Tag ... />` 提取标签名（首个标签字符序列直到空白 / `/` / `>`）。
fn detect_registered_tag(html: &str) -> Option<String> {
  let after_lt = html.strip_prefix('<')?;
  let end =
    after_lt.find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(after_lt.len());
  let name = &after_lt[..end];
  if name.is_empty() || !name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
    // 注册表只匹配大写开头的自定义组件，避免和原生 HTML 标签冲突。
    return None;
  }
  Some(name.to_string())
}

/// 解析 `<Tag a="b" c='d' />` 形式中的属性。仅支持 `key="value"` / `key='value'`。
fn parse_attrs(html: &str) -> HashMap<String, String> {
  let mut attrs = HashMap::new();
  let bytes = html.as_bytes();
  let mut i = 0usize;
  // 先跳过标签名
  while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' && bytes[i] != b'\n' {
    i += 1;
  }
  while i < bytes.len() {
    // skip whitespace
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n') {
      i += 1;
    }
    if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' {
      break;
    }
    // read key
    let key_start = i;
    while i < bytes.len()
      && bytes[i] != b'='
      && bytes[i] != b' '
      && bytes[i] != b'\t'
      && bytes[i] != b'/'
      && bytes[i] != b'>'
    {
      i += 1;
    }
    let key = match std::str::from_utf8(&bytes[key_start..i]) {
      Ok(s) => s.to_string(),
      Err(_) => break,
    };
    if i >= bytes.len() || bytes[i] != b'=' {
      // bare attribute (key without value) — skip
      continue;
    }
    i += 1; // skip '='
    if i >= bytes.len() {
      break;
    }
    let quote = bytes[i];
    if quote != b'"' && quote != b'\'' {
      // 未闭合属性，整体放弃
      break;
    }
    i += 1;
    let val_start = i;
    while i < bytes.len() && bytes[i] != quote {
      i += 1;
    }
    let value = match std::str::from_utf8(&bytes[val_start..i]) {
      Ok(s) => s.to_string(),
      Err(_) => break,
    };
    if i < bytes.len() {
      i += 1; // skip closing quote
    }
    attrs.insert(key, value);
  }
  attrs
}

/// 将 :::type 语法转换为 GFM alert 语法
fn convert_admonitions(body: &str) -> String {
  let mut result = String::with_capacity(body.len());
  let mut in_block = false;

  for line in body.lines() {
    let trimmed = line.trim();
    if !in_block {
      if let Some(kind) = trimmed.strip_prefix(":::").and_then(|rest| {
        let word = rest.split_whitespace().next().unwrap_or("");
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
          "\u{1f4dd}",
          "NOTE",
          "border-l-blue-500",
          "border-blue-200 dark:border-blue-800/60",
          "bg-blue-50 dark:bg-blue-950/30",
          "text-blue-600 dark:text-blue-400",
        ),
        BlockQuoteKind::Tip => (
          "\u{1f4a1}",
          "TIP",
          "border-l-green-500",
          "border-green-200 dark:border-green-800/60",
          "bg-green-50 dark:bg-green-950/30",
          "text-green-600 dark:text-green-400",
        ),
        BlockQuoteKind::Important => (
          "\u{2757}",
          "IMPORTANT",
          "border-l-purple-500",
          "border-purple-200 dark:border-purple-800/60",
          "bg-purple-50 dark:bg-purple-950/30",
          "text-purple-600 dark:text-purple-400",
        ),
        BlockQuoteKind::Warning => (
          "\u{26a0}\u{fe0f}",
          "WARNING",
          "border-l-yellow-500",
          "border-yellow-200 dark:border-yellow-800/60",
          "bg-yellow-50 dark:bg-yellow-950/30",
          "text-yellow-600 dark:text-yellow-400",
        ),
        BlockQuoteKind::Caution => (
          "\u{1f6d1}",
          "CAUTION",
          "border-l-red-500",
          "border-red-200 dark:border-red-800/60",
          "bg-red-50 dark:bg-red-950/30",
          "text-red-600 dark:text-red-400",
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
      tracing::warn!(error = %e, "math: LaTeX render error");
      format!("<code>{}</code>", latex.replace('<', "&lt;").replace('>', "&gt;"))
    }
  }
}

// Phase 2.2 后：`<Yellow .../>` / `<Underline .../>` / `<Strikethrough .../>`
// 等文字样式组件都走 [`crate::components`] 下的 `MdxComponent` 实现，
// `render_text_style_component` 不再需要。

// Phase 2.2 后只在单测中使用；生产路径走 `parse_attrs`。保留以便
// 未来有需要调试全属性提取。
#[allow(dead_code)]
fn extract_attr(html: &str, attr: &str) -> Option<String> {
  let pattern = format!("{}=\"", attr);
  let start = html.find(&pattern)? + pattern.len();
  let end = html[start..].find('"')?;
  Some(html[start..start + end].to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_mdx_without_frontmatter() {
    let (meta, body) = parse_mdx("# Hello\n\nworld");
    assert_eq!(meta.title, "");
    assert!(body.contains("Hello"));
  }

  #[test]
  fn parse_mdx_with_frontmatter() {
    let src = "---\ntitle: Hello\ndescription: A test\n---\n\nbody text";
    let (meta, body) = parse_mdx(src);
    assert_eq!(meta.title, "Hello");
    assert_eq!(meta.description.as_deref(), Some("A test"));
    assert_eq!(body.trim(), "body text");
  }

  #[test]
  fn parse_mdx_partial_frontmatter_returns_default() {
    // 只有一段 `---`，未关闭 → 视为无 frontmatter
    let src = "---\ntitle: bad";
    let (meta, body) = parse_mdx(src);
    assert_eq!(meta, PostMetadata::default());
    assert!(body.contains("title: bad"));
  }

  #[test]
  fn convert_admonitions_note() {
    let input = ":::note\nHello\n:::\n";
    let out = convert_admonitions(input);
    assert!(out.contains("> [!NOTE]"));
    assert!(out.contains("> Hello"));
  }

  #[test]
  fn convert_admonitions_warn_alias() {
    let input = ":::warn\nbe careful\n:::\n";
    let out = convert_admonitions(input);
    assert!(out.contains("> [!WARNING]"));
  }

  #[test]
  fn convert_admonitions_unknown_type_falls_back_to_note() {
    let input = ":::random\ntext\n:::\n";
    let out = convert_admonitions(input);
    assert!(out.contains("> [!NOTE]"));
  }

  #[test]
  fn convert_admonitions_no_block_passthrough() {
    let input = "plain line\nanother\n";
    let out = convert_admonitions(input);
    assert_eq!(out, "plain line\nanother\n");
  }

  #[test]
  fn detect_registered_tag_capital_only() {
    assert_eq!(detect_registered_tag("<PodcastCard id=\"1\" />"), Some("PodcastCard".to_string()));
    assert_eq!(detect_registered_tag("<YouTube id=\"abc\" />"), Some("YouTube".to_string()));
    // 小写 HTML 标签不应被识别为注册组件
    assert_eq!(detect_registered_tag("<span>x</span>"), None);
    assert_eq!(detect_registered_tag("<div"), None);
  }

  #[test]
  fn parse_attrs_basic() {
    let attrs = parse_attrs("<PodcastCard id=\"3\" title=\"Hello\" />");
    assert_eq!(attrs.get("id").map(|s| s.as_str()), Some("3"));
    assert_eq!(attrs.get("title").map(|s| s.as_str()), Some("Hello"));
    assert_eq!(attrs.len(), 2);
  }

  #[test]
  fn parse_attrs_single_quotes() {
    let attrs = parse_attrs("<Tag a='1' b='2' />");
    assert_eq!(attrs.get("a").map(|s| s.as_str()), Some("1"));
    assert_eq!(attrs.get("b").map(|s| s.as_str()), Some("2"));
  }

  #[test]
  fn parse_attrs_no_attrs_is_empty() {
    let attrs = parse_attrs("<NoAttrs />");
    assert!(attrs.is_empty());
  }

  #[test]
  fn extract_attr_basic() {
    assert_eq!(extract_attr(r#"<X id="42" />"#, "id"), Some("42".to_string()));
    assert_eq!(extract_attr(r#"<X id="42" />"#, "missing"), None);
  }

  #[test]
  fn latex_to_mathml_inline_smoke() {
    let mathml = latex_to_mathml_string("a + b", false);
    // 不严格断言完整 XML，只校验非空且包含 math 元素
    assert!(!mathml.is_empty());
    assert!(mathml.contains("math"));
  }

  #[test]
  fn latex_to_mathml_invalid_falls_back_to_code() {
    // 故意用不闭合的 LaTeX，确认走 fallback 不 panic
    let mathml = latex_to_mathml_string("\\frac{", false);
    assert!(mathml.contains("<code>") || mathml.contains("math"));
  }
}
