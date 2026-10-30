//! MDX 渲染管道（Phase 2.1 从 `crates/modules/blog/src/markdown.rs` 迁移过来）。
//!
//! 与原版的差异：
//! 1. 不再直接 `use module_podcast::podcast::PodcastCard`，
//!    改为通过 [`crate::registry`] 查表渲染，避免 widgets crate 依赖具体业务模块。
//! 2. 其余渲染逻辑（GFM / 数学 / Mermaid / 代码块 + Copy / 颜色组件 / 标注 block-id）
//!    保持与原版一致，welcome 示例和现有快照仍可像素级回归。

use dioxus::prelude::*;
use pulldown_cmark::{
  Alignment, BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag,
  TagEnd,
};
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
#[serde(default)]
pub struct PostMetadata {
  /// 缺省为空（Z3）：之前缺 `title` 会让整块 frontmatter 解析失败。
  #[serde(deserialize_with = "de_text")]
  pub title: String,
  /// `summary` / `excerpt` 是其他博客系统的同义字段（Z3）。
  #[serde(alias = "summary", alias = "excerpt", deserialize_with = "de_opt_text")]
  pub description: Option<String>,
  /// 字符串或列表；列表以 `, ` 连接（Z3）。
  #[serde(deserialize_with = "de_opt_joined")]
  pub keywords: Option<String>,
  /// 社交分享封面。推荐是绝对 URL；相对路径也允许，调用方负责拼接。
  pub image: Option<String>,
  /// 作者。JSON-LD 中作为 `author.name` 用。
  #[serde(deserialize_with = "de_opt_text")]
  pub author: Option<String>,
  /// 手动覆盖 canonical URL；不提供时 [`crate::seo::inject_seo`]
  /// 从 `base_url + path` 自动推导。
  pub canonical: Option<String>,
  /// 发布日期。[`parse_mdx`] 会规范化（见 [`normalize_date`]）。JSON-LD 中
  /// 填入 `datePublished`。未提供时不输出该字段。
  #[serde(deserialize_with = "de_opt_text")]
  pub date: Option<String>,
  /// 更新日期，规范化同 `date`。
  #[serde(alias = "lastmod", deserialize_with = "de_opt_text")]
  pub updated: Option<String>,
  /// 文章标签，字符串或列表（Z3）。JSON-LD `keywords` + Open Graph `article:tag`。
  #[serde(deserialize_with = "de_list")]
  pub tags: Vec<String>,
  /// 草稿不进列表 / sitemap / feed / 搜索，文章页 404（Z3）。
  pub draft: bool,
}

/// YAML 标量（字符串 / 数字 / 布尔）转成字符串；`title: 2024` 这类也能读。
fn yaml_scalar(v: serde_yaml::Value) -> Option<String> {
  match v {
    serde_yaml::Value::String(s) => Some(s),
    serde_yaml::Value::Number(n) => Some(n.to_string()),
    serde_yaml::Value::Bool(b) => Some(b.to_string()),
    _ => None,
  }
}

fn de_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
  Ok(de_opt_text(d)?.unwrap_or_default())
}

fn de_opt_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
  Ok(yaml_scalar(serde_yaml::Value::deserialize(d)?))
}

fn de_list<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
  Ok(match serde_yaml::Value::deserialize(d)? {
    serde_yaml::Value::Sequence(items) => items.into_iter().filter_map(yaml_scalar).collect(),
    v => yaml_scalar(v).into_iter().filter(|s| !s.is_empty()).collect(),
  })
}

fn de_opt_joined<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
  let list = de_list(d)?;
  Ok(if list.is_empty() { None } else { Some(list.join(", ")) })
}

/// 规范化各博客系统常见的日期写法：`YYYY-M-D`、`YYYY/MM/DD`、`YYYY-MM-DD HH:MM[:SS]`
/// 归一为 `YYYY-MM-DD`（不带时区的时间无法确定时区，只留日期）；RFC 3339 原样保留。
/// 无法识别时返回 `None`，调用方保留原值。
pub fn normalize_date(raw: &str) -> Option<String> {
  let raw = raw.trim();
  if chrono::DateTime::parse_from_rfc3339(raw).is_ok() {
    return Some(raw.to_string());
  }
  for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y/%m/%d %H:%M:%S", "%Y/%m/%d %H:%M"] {
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, fmt) {
      return Some(dt.date().format("%Y-%m-%d").to_string());
    }
  }
  for fmt in ["%Y-%m-%d", "%Y/%m/%d"] {
    if let Ok(d) = chrono::NaiveDate::parse_from_str(raw, fmt) {
      return Some(d.format("%Y-%m-%d").to_string());
    }
  }
  None
}

fn normalize_date_field(field: &mut Option<String>) {
  if let Some(raw) = field.as_deref() {
    match normalize_date(raw) {
      Some(d) => *field = Some(d),
      None if raw.trim().is_empty() => *field = None,
      None => tracing::warn!(date = raw, "frontmatter: unrecognized date, kept as is"),
    }
  }
}

#[derive(Props, Clone, PartialEq)]
pub struct MarkdownProps {
  pub content: String,
  /// 当前文章 / 章节 / 案例 ID，用于解析图片相对路径。
  pub blog_id: String,
  /// Phase 4.2：来自用户的不受信内容（评论 / 话题 / 标注 等）。
  /// 默认 `false`（站点作者内容）。`true` 时原始 HTML 一律按文本输出，链接与图片走白名单，
  /// 不解析 frontmatter，不画 mermaid，标题不出 h1、不能自定 id 和 class。
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
  let mut metadata: PostMetadata = match serde_yaml::from_str(parts[1]) {
    Ok(m) => m,
    Err(e) => {
      tracing::warn!(error = %e, "frontmatter: invalid YAML, metadata ignored");
      PostMetadata::default()
    }
  };
  normalize_date_field(&mut metadata.date);
  normalize_date_field(&mut metadata.updated);
  let body = parts[2].trim().to_string();
  (metadata, body)
}

#[component]
pub fn Markdown(props: MarkdownProps) -> Element {
  let (metadata, body) = split_source(&props.content, props.untrusted);

  // 预处理：将 :::type 语法转换为 GFM alert 语法
  let body = convert_admonitions(&body);
  let mut events = parse_events(&body);
  if !props.untrusted {
    assign_heading_ids(&mut events);
  }
  let mut it = number_footnotes(events).into_iter();

  // 渲染流，传入 blog_id；同时按顶层块编号注入 data-block-id 供标注定位
  let mut block_idx: usize = 1;
  let elements = render_stream(&mut it, &props.blog_id, props.untrusted, &mut block_idx, true);

  rsx! {
      // 没有 frontmatter 标题时不设置，免得把页面自己的标题（如文档页）覆盖成空串
      if !metadata.title.is_empty() {
          crate::browser::PageTitle { title: crate::seo::page_title(&metadata.title) }
      }

      div { class: "prose prose-slate dark:prose-invert max-w-none",
          {elements.into_iter()}
      }
      // 注：math / .prose code 等全局静态样式以及 Prism / Mermaid 重高亮触发器
      // 不在此组件内挂载——它们由 App 根 (`crates/app/src/main.rs`) 一次性
      // 安装。在 Markdown 组件级别用 document::Style/Script 会触发
      // dioxus-document "Changing the props … is not supported" 警告（每次
      // 路由切换重渲都会"换"一遍），而且替换后的 script 不会被再次执行 →
      // SPA 导航后 Prism/Mermaid 失效。改由 main.rs 里的 MutationObserver
      // 自动捕获新插入的 code/mermaid 块，覆盖首屏 + SPA 导航两个场景。
  }
}

/// 拆出 frontmatter 和正文。用户内容没有 frontmatter：开头的 `---` 只是分隔线，
/// 不能让评论 / 帖子设置页面标题，也不能把两条 `---` 之间的文字吞掉。
fn split_source(content: &str, untrusted: bool) -> (PostMetadata, String) {
  if untrusted {
    // 原始 HTML 在渲染时按文本输出（render_stream 的 Html 分支），解析前不改源文：
    // 字符串级的删除会毁掉用户贴的代码（`let one=1;` 被当成 `on*=` 属性）。
    (PostMetadata::default(), content.to_string())
  } else {
    parse_mdx(content)
  }
}

fn parse_events(body: &str) -> Vec<Event<'_>> {
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_FOOTNOTES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);
  options.insert(Options::ENABLE_MATH);
  options.insert(Options::ENABLE_GFM);
  options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
  Parser::new_ext(body, options).collect()
}

/// 页内目录的一项：`level` 是 2 或 3，`id` 与 [`Markdown`] 渲染出的标题 id 相同。
#[derive(Clone, Debug, PartialEq)]
pub struct TocEntry {
  pub level: u8,
  pub id: String,
  pub text: String,
}

/// 站点作者内容的 h2 / h3 标题（T2）。走与 [`Markdown`] 相同的解析和 id 分配，
/// 服务端与客户端由同一份正文算出同样的结果。
pub fn heading_outline(content: &str) -> Vec<TocEntry> {
  let (_, body) = parse_mdx(content);
  let body = convert_admonitions(&body);
  let mut events = parse_events(&body);
  assign_heading_ids(&mut events);
  let mut entries = Vec::new();
  let mut current: Option<TocEntry> = None;
  for event in events {
    match event {
      Event::Start(Tag::Heading { level, id: Some(id), .. })
        if matches!(level, HeadingLevel::H2 | HeadingLevel::H3) =>
      {
        let level = if level == HeadingLevel::H2 { 2 } else { 3 };
        current = Some(TocEntry { level, id: id.to_string(), text: String::new() });
      }
      Event::Text(t) | Event::Code(t) => {
        if let Some(entry) = current.as_mut() {
          entry.text.push_str(&t);
        }
      }
      Event::End(TagEnd::Heading(_)) => entries.extend(current.take()),
      _ => {}
    }
  }
  entries
}

/// M1：站点内容的标题 id 与 Docusaurus 一致：显式 `{#id}` 优先，否则按 github-slugger
/// 由标题文字生成（重名依次加 `-1`、`-2`），旧文章里的 `](#引言)` 才能跳转。
/// 标注按 `data-block-id` 定位，不受影响。
fn assign_heading_ids(events: &mut [Event<'_>]) {
  let mut used = std::collections::HashSet::new();
  for i in 0..events.len() {
    let Event::Start(Tag::Heading { id, .. }) = &events[i] else {
      continue;
    };
    let anchor = match id {
      Some(explicit) => explicit.to_string(),
      None => {
        let mut text = String::new();
        for event in &events[i + 1..] {
          match event {
            Event::End(TagEnd::Heading(_)) => break,
            Event::Text(t) | Event::Code(t) => text.push_str(t),
            _ => {}
          }
        }
        let base = slugify(&text);
        if base.is_empty() {
          continue;
        }
        let mut candidate = base.clone();
        let mut n = 0;
        while used.contains(&candidate) {
          n += 1;
          candidate = format!("{base}-{n}");
        }
        candidate
      }
    };
    used.insert(anchor.clone());
    if let Event::Start(Tag::Heading { id, .. }) = &mut events[i] {
      *id = Some(anchor.into());
    }
  }
}

/// github-slugger：小写，去掉标点与符号（保留字母、数字、`-`、`_`），空格换成 `-`。
fn slugify(text: &str) -> String {
  text
    .to_lowercase()
    .chars()
    .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
    .map(|c| if c == ' ' { '-' } else { c })
    .collect()
}

/// 脚注按第一次引用的顺序编号，标签换成编号（渲染出的 id 只含数字，用户写的标签
/// 不进 id）；定义从原位置取出，按编号排到正文之后，前面加一条分隔线。
/// 没被引用的定义排在最后，同一标签的重复定义只留第一条。
fn number_footnotes(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
  let mut numbers: HashMap<String, usize> = HashMap::new();
  for event in &events {
    if let Event::FootnoteReference(label) = event {
      let next = numbers.len() + 1;
      numbers.entry(label.to_string()).or_insert(next);
    }
  }
  let mut body = Vec::with_capacity(events.len());
  let mut notes: Vec<(usize, Vec<Event<'_>>)> = Vec::new();
  let mut it = events.into_iter();
  while let Some(event) = it.next() {
    match event {
      Event::FootnoteReference(label) => {
        let n = numbers.get(label.as_ref()).copied().unwrap_or_default();
        body.push(Event::FootnoteReference(CowStr::from(n.to_string())));
      }
      Event::Start(Tag::FootnoteDefinition(label)) => {
        let next = numbers.len() + 1;
        let n = *numbers.entry(label.to_string()).or_insert(next);
        let mut note = vec![Event::Start(Tag::FootnoteDefinition(CowStr::from(n.to_string())))];
        for inner in it.by_ref() {
          let end = matches!(inner, Event::End(TagEnd::FootnoteDefinition));
          note.push(inner);
          if end {
            break;
          }
        }
        if notes.iter().all(|(m, _)| *m != n) {
          notes.push((n, note));
        }
      }
      other => body.push(other),
    }
  }
  if !notes.is_empty() {
    notes.sort_by_key(|(n, _)| *n);
    body.push(Event::Rule);
    body.extend(notes.into_iter().flat_map(|(_, note)| note));
  }
  body
}

/// 表格：列对齐写进每个单元格；表头单元格是 `th`，表体显式包 `tbody`
/// （浏览器会自己补 `tbody`，服务端不写就和 hydration 对不上）。
fn render_table(
  it: &mut std::vec::IntoIter<Event<'_>>,
  aligns: &[Alignment],
  blog_id: &str,
  untrusted: bool,
  block_idx: &mut usize,
  block_id: Option<String>,
) -> Element {
  let bid = block_id.unwrap_or_default();
  let has_bid = !bid.is_empty();
  let mut head = Vec::new();
  let mut rows = Vec::new();
  let mut cells = Vec::new();
  let mut in_head = false;
  while let Some(event) = it.next() {
    match event {
      Event::Start(Tag::TableHead) => in_head = true,
      Event::Start(Tag::TableCell) => {
        let align = match aligns.get(cells.len()) {
          Some(Alignment::Left) => "text-align:left",
          Some(Alignment::Center) => "text-align:center",
          Some(Alignment::Right) => "text-align:right",
          _ => "",
        };
        let children = render_stream(it, blog_id, untrusted, block_idx, false);
        cells.push(if in_head {
          rsx! { th { class: "px-4 py-3 text-left text-sm font-semibold text-slate-700 dark:text-slate-300",
              style: if !align.is_empty() { "{align}" },
              {children.into_iter()}
          } }
        } else {
          rsx! { td { class: "px-4 py-3 text-sm",
              style: if !align.is_empty() { "{align}" },
              {children.into_iter()}
          } }
        });
      }
      Event::End(TagEnd::TableHead) => {
        in_head = false;
        head = std::mem::take(&mut cells);
      }
      Event::End(TagEnd::TableRow) => {
        let row = std::mem::take(&mut cells);
        rows.push(rsx! { tr { class: "border-b border-slate-200 dark:border-slate-800", {row.into_iter()} } });
      }
      Event::End(TagEnd::Table) => break,
      _ => {}
    }
  }
  rsx! {
      div { class: "overflow-x-auto my-6",
          table {
              id: if has_bid { "{bid}" },
              "data-block-id": if has_bid { "{bid}" },
              class: "min-w-full border-collapse",
              thead { class: "bg-slate-50 dark:bg-slate-800/50", tr { {head.into_iter()} } }
              tbody { {rows.into_iter()} }
          }
      }
  }
}

fn render_stream<'a>(
  it: &mut std::vec::IntoIter<Event<'a>>,
  blog_id: &str,
  untrusted: bool,
  block_idx: &mut usize,
  top: bool,
) -> Vec<Element> {
  let mut nodes = Vec::new();

  while let Some(event) = it.next() {
    match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                // 特殊处理代码块：收集原始文本用于 Copy 按钮
                // info 串只取语言名：`rust,ignore`、`py title="x.py"` 后面的部分不是 class
                let lang = match &kind {
                    CodeBlockKind::Fenced(info) => info
                        .split(|c: char| c == ',' || c == '{' || c.is_whitespace())
                        .next()
                        .filter(|lang| !lang.is_empty())
                        .unwrap_or("text")
                        .to_string(),
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
                if renders_as_diagram(&lang, untrusted) {
                    nodes.push(render_mermaid_block(code_text, id));
                } else {
                    nodes.push(render_code_block(lang, code_text, id));
                }
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                // X1：图片的描述是它的子事件，取纯文本作 alt（不当正文渲染）。
                let mut alt = String::new();
                let mut depth = 0usize;
                for event in it.by_ref() {
                    match event {
                        Event::Start(_) => depth += 1,
                        Event::End(_) if depth == 0 => break,
                        Event::End(_) => depth -= 1,
                        Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                        Event::SoftBreak | Event::HardBreak => alt.push(' '),
                        _ => {}
                    }
                }
                nodes.push(render_image(&dest_url, &title, &alt, blog_id, untrusted));
            }
            Event::Start(Tag::Table(aligns)) => {
                let id = mint_block_id(top, block_idx);
                nodes.push(render_table(it, &aligns, blog_id, untrusted, block_idx, id));
            }
            Event::Start(tag) => {
                let id = mint_block_id(top, block_idx);
                let children = render_stream(it, blog_id, untrusted, block_idx, false);
                nodes.push(render_tag(tag, children, untrusted, id));
            }
            Event::End(_) => break,
            Event::Text(text) => nodes.push(rsx! { "{text}" }),
            Event::Code(code) => nodes.push(rsx! {
                code { class: "bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded text-sm font-mono break-words text-pink-600 dark:text-pink-400", "{code}" }
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
            Event::TaskListMarker(checked) => nodes.push(rsx! {
                input { r#type: "checkbox", disabled: true, checked, class: "mr-2 align-middle" }
            }),
            // 标签已由 number_footnotes 换成编号
            Event::FootnoteReference(n) => nodes.push(rsx! {
                sup { id: "fnref-{n}", a { href: "#fn-{n}", class: "text-primary no-underline", "[{n}]" } }
            }),
            Event::SoftBreak => nodes.push(rsx! { " " }),
            Event::HardBreak => nodes.push(rsx! { br {} }),
            Event::Rule => nodes.push(rsx! { hr { class: "my-8 border-slate-200 dark:border-slate-800" } }),
            Event::Html(html) | Event::InlineHtml(html) => {
                let h = html.trim();
                // M2：表格单元格里只能靠 `<br/>` 换行（旧博客常见）
                if matches!(h.to_ascii_lowercase().as_str(), "<br>" | "<br/>" | "<br />") {
                    nodes.push(rsx! { br {} });
                    continue;
                }
                if h.starts_with('<') {
                    if let Some(component) = render_mdx_registry(h) {
                        nodes.push(component);
                        continue;
                    }
                }
                nodes.push(rsx! { span { "{h}" } });
            }
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

/// 图片：相对路径按 blog_id 解析（`image_src`），描述作 alt，title 作图注。
fn render_image(dest_url: &str, title: &str, alt: &str, blog_id: &str, untrusted: bool) -> Element {
  let Some(src) = image_src(dest_url, blog_id, untrusted) else {
    return rsx! { span { class: "text-red-500", "[image rejected: unsafe URL]" } };
  };
  rsx! {
      figure { class: "my-8",
          // 长文图多（迁移后最多一篇 17 张），首屏外的不先下载
          img { src: "{src}", alt: "{alt}", loading: "lazy", decoding: "async", class: "rounded-2xl shadow-xl mx-auto border border-slate-200 dark:border-slate-800" }
          if !title.is_empty() {
              figcaption { class: "text-center text-sm text-slate-500 mt-3 italic", "{title}" }
          }
      }
  }
}

fn render_tag(
  tag: Tag,
  children: Vec<Element>,
  untrusted: bool,
  block_id: Option<String>,
) -> Element {
  let bid = block_id.unwrap_or_default();
  let has_bid = !bid.is_empty();
  match tag {
    Tag::Heading { level, id, classes, .. } => {
      // 用户内容最高到 h2：h1 留给页面标题（G7）；也不能自定 id（M1）和 class
      // （站点的样式类能把评论叠到页面上）。
      let l = if untrusted { (level as u32).max(2) } else { level as u32 };
      let anchor = id.filter(|_| !untrusted).map(|s| s.to_string()).unwrap_or_else(|| bid.clone());
      let has_anchor = !anchor.is_empty();
      let class = if untrusted { String::new() } else { classes.join(" ") };
      let has_class = !class.is_empty();
      match l {
        1 => rsx! { h1 {
            id: if has_anchor { "{anchor}" },
            class: if has_class { "{class}" },
            "data-block-id": if has_bid { "{bid}" },
            {children.into_iter()}
        } },
        2 => rsx! { h2 {
            id: if has_anchor { "{anchor}" },
            class: if has_class { "{class}" },
            "data-block-id": if has_bid { "{bid}" },
            {children.into_iter()}
        } },
        3 => rsx! { h3 {
            id: if has_anchor { "{anchor}" },
            class: if has_class { "{class}" },
            "data-block-id": if has_bid { "{bid}" },
            {children.into_iter()}
        } },
        4 => rsx! { h4 {
            id: if has_anchor { "{anchor}" },
            class: if has_class { "{class}" },
            "data-block-id": if has_bid { "{bid}" },
            {children.into_iter()}
        } },
        5 => rsx! { h5 {
            id: if has_anchor { "{anchor}" },
            class: if has_class { "{class}" },
            "data-block-id": if has_bid { "{bid}" },
            {children.into_iter()}
        } },
        _ => rsx! { h6 {
            id: if has_anchor { "{anchor}" },
            class: if has_class { "{class}" },
            "data-block-id": if has_bid { "{bid}" },
            {children.into_iter()}
        } },
      }
    }
    // 标签已由 number_footnotes 换成编号
    Tag::FootnoteDefinition(n) => rsx! { div {
        id: "fn-{n}",
        "data-block-id": if has_bid { "{bid}" },
        class: "flex gap-2 text-sm text-slate-600 dark:text-slate-400",
        span { class: "shrink-0", "{n}." }
        div { {children.into_iter()} }
        a { href: "#fnref-{n}", class: "shrink-0 no-underline", "↩" }
    } },
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
      // Phase 8.6：scheme allowlist。任何 HTML-entity / 大小写 / 制表符变体的
      // `javascript:` / `data:` 编码 URL 都会被 reject，渲染为纯文本（保留 children）。
      // 实体反编码 + 协议检查在 [`crate::sanitize::is_safe_link_url`]。
      if crate::sanitize::is_safe_link_url(dest_url.as_ref()) {
        rsx! { a { href: "{dest_url}", class: "text-primary underline decoration-primary/30 hover:decoration-primary transition-all", {children.into_iter()} } }
      } else {
        // 丢弃链接、保留可见文本，避免内容因 URL 被拒而消失
        rsx! { span { {children.into_iter()} } }
      }
    }

    Tag::BlockQuote(kind) => {
      render_blockquote(kind, children, if has_bid { Some(bid.clone()) } else { None })
    }
    Tag::Strong => rsx! { strong { {children.into_iter()} } },
    Tag::Emphasis => rsx! { em { {children.into_iter()} } },
    Tag::Strikethrough => rsx! { del { {children.into_iter()} } },
    _ => rsx! { span { {children.into_iter()} } },
  }
}

/// Phase 2.2: 纯注册表查询。内置 9 个组件（YouTube / Bilibili / 五色
/// 高亮 / Underline / Strikethrough）由 [`crate::components::register_default_components`]
/// 在启动期预注册；podcast 等业务模块走自己的 `register_components`。
/// 未注册的标签返回 None，调用方（`render_stream`）会降级为
/// 占位 span，保证整篇文章仍可渲染。
/// 图片最终的 `src`；`None` 表示拒绝渲染。
///
/// 站点作者内容：scheme 白名单（[`crate::sanitize::is_safe_image_url`]），
/// 相对路径解析到文章资产目录。不可信内容只允许本站上传的图片
/// （[`crate::sanitize::is_upload_image_url`]，SEC-09）：其余站内路径会被浏览者
/// 的浏览器带 cookie 请求（如 `/api/auth/logout`），外部图片会泄露浏览者 IP。
fn image_src(url: &str, blog_id: &str, untrusted: bool) -> Option<String> {
  if untrusted {
    return crate::sanitize::is_upload_image_url(url).then(|| url.to_string());
  }
  if !crate::sanitize::is_safe_image_url(url) {
    return None;
  }
  if url.starts_with("http") || url.starts_with('/') {
    return Some(url.to_string());
  }
  // 处理 ID 为 "1" 的特殊情况，映射到 welcome 目录
  let folder = if blog_id == "1" { "welcome" } else { blog_id };
  Some(format!("/posts/{}/{}", folder, url))
}

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

/// 代码围栏的开头：返回围栏字符和长度（至少 3 个 ` 或 ~）。
fn code_fence(trimmed: &str) -> Option<(char, usize)> {
  let ch = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
  let len = trimmed.chars().take_while(|c| *c == ch).count();
  (len >= 3).then_some((ch, len))
}

/// 将 :::type 语法转换为 GFM alert 语法
fn convert_admonitions(body: &str) -> String {
  let mut result = String::with_capacity(body.len());
  let mut in_block = false;
  // 围栏代码块里的 `:::`（比如 rustc 报错里的 `  ::: src/x.rs:1:1`）原样保留
  let mut fence: Option<(char, usize)> = None;

  for line in body.lines() {
    let trimmed = line.trim();
    if let Some((ch, len)) = fence {
      let closes = code_fence(trimmed)
        .is_some_and(|(c, n)| c == ch && n >= len && trimmed.chars().all(|x| x == ch));
      if closes {
        fence = None;
      }
    } else if let Some(open) = code_fence(trimmed) {
      fence = Some(open);
    } else if !in_block {
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

/// ```` ```mermaid ```` 是否渲染成图（SEC-17）。用户内容只按普通代码块显示：
/// mermaid 解析器与 SVG 生成是一大块额外攻击面，评论 / 论坛不需要画图。
fn renders_as_diagram(lang: &str, untrusted: bool) -> bool {
  lang == "mermaid" && !untrusted
}

fn render_mermaid_block(code_text: String, block_id: Option<String>) -> Element {
  let bid = block_id.unwrap_or_default();
  let has_bid = !bid.is_empty();
  rsx! {
      div {
          id: if has_bid { "{bid}" },
          "data-block-id": if has_bid { "{bid}" },
          class: "not-prose my-6 flex justify-center overflow-x-auto rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6",
          // 内容走 dangerous_inner_html：mermaid / Prism 会改写块内节点，Dioxus 不能跟踪
          // 这些子节点，否则 hydration 找不到自己的标记而崩掉（之后整页的事件都失效）。
          // mermaid 给 svg 的是 width="100%" 加 max-width：pre 要占满整行，svg 才按自身宽度显示，
          // 否则 flex 里的 pre 收缩到内容宽度，与 svg 的 100% 互相依赖，图被压得很小。
          pre { class: "mermaid w-full [&>svg]:mx-auto", dangerous_inner_html: escape_markup(&code_text) }
      }
  }
}

fn render_code_block(lang: String, code_text: String, block_id: Option<String>) -> Element {
  rsx! { CodeBlock { lang, code_text, block_id } }
}

#[component]
fn CodeBlock(lang: String, code_text: String, block_id: Option<String>) -> Element {
  let mut copied = use_signal(|| false);
  let bid = block_id.unwrap_or_default();
  let has_bid = !bid.is_empty();
  let code_for_copy = code_text.clone();

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
                  let text = code_for_copy.clone();
                  spawn(async move {
                      if crate::browser::copy_text(&text).await {
                          copied.set(true);
                          crate::browser::sleep_ms(1500).await;
                          copied.set(false);
                      }
                  });
              },
              if copied() { "Copied!" } else { "Copy" }
          }
          pre { class: "rounded-xl p-4 bg-slate-900 overflow-x-auto shadow-inner",
              // 同 render_mermaid_block：Prism 会改写 code 的子节点。
              code { class: "language-{lang} text-sm text-slate-200", dangerous_inner_html: escape_markup(&code_text) }
          }
      }
  }
}

/// 使用 pulldown-latex 将 LaTeX 转换为 MathML
///
/// pulldown-latex 把文本类事件（`\text{}`、`\operatorname{}`、数字）和解析
/// 错误信息原样写进 MathML，而结果会进 `dangerous_inner_html`：`$\text{<img
/// onerror=…>}$` 即存储型 XSS（SEC-01）。这里先把这些字符串转义再交给它；
/// 其余事件只携带单个字符或数值，单独的 `<` 不会被浏览器当作标签。
fn latex_to_mathml_string(latex: &str, display: bool) -> String {
  use pulldown_latex::event::{Content, Event as LatexEvent};

  let storage = Storage::new();
  let events: Vec<_> = LatexParser::new(latex, &storage).collect();
  let escaped: Vec<String> = events
    .iter()
    .map(|event| match event {
      Ok(LatexEvent::Content(Content::Text(s) | Content::Number(s) | Content::Function(s))) => {
        escape_markup(s)
      }
      Ok(_) => String::new(),
      Err(e) => escape_markup(&e.to_string()),
    })
    .collect();
  let parser = events.into_iter().zip(&escaped).map(|(event, esc)| match event {
    Ok(LatexEvent::Content(Content::Text(_))) => Ok(LatexEvent::Content(Content::Text(esc))),
    Ok(LatexEvent::Content(Content::Number(_))) => Ok(LatexEvent::Content(Content::Number(esc))),
    Ok(LatexEvent::Content(Content::Function(_))) => {
      Ok(LatexEvent::Content(Content::Function(esc)))
    }
    Ok(other) => Ok(other),
    Err(_) => Err(EscapedLatexError(esc)),
  });
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
      format!("<code>{}</code>", escape_markup(latex))
    }
  }
}

fn escape_markup(s: &str) -> String {
  s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// 已转义的解析错误信息；pulldown-latex 用它的 `Display` 写进 `<merror>`。
#[derive(Debug)]
struct EscapedLatexError<'a>(&'a str);

impl std::fmt::Display for EscapedLatexError<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.0)
  }
}

impl std::error::Error for EscapedLatexError<'_> {}

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

  fn render_markdown(content: &str, untrusted: bool) -> String {
    let content = content.to_string();
    dioxus_ssr::render_element(rsx! { Markdown { content, blog_id: "t", untrusted } })
  }

  /// G7：用户内容（评论 / 话题）不产生 h1，页面只有自己的一个 h1。
  #[test]
  fn untrusted_h1_renders_as_h2() {
    assert!(render_markdown("# 标题", false).contains("<h1"));
    let html = render_markdown("# 标题", true);
    assert!(!html.contains("<h1") && html.contains("<h2"), "{html}");
  }

  /// M1：站点内容的标题 id 与 Docusaurus 一致（旧文章的 `](#引言)` 才能跳转），
  /// `data-block-id` 仍是 `bN`，标注不受影响。
  #[test]
  fn heading_ids_follow_docusaurus() {
    let md = "## 引言\n\n## Hello, World!\n\n## 引言\n\n## 自定义 {#custom-id}\n\n## `Vec<T>` 用法";
    let html = render_markdown(md, false);
    for id in ["引言", "hello-world", "引言-1", "custom-id", "vect-用法"] {
      assert!(html.contains(&format!("id=\"{id}\"")), "缺少 id={id}：{html}");
    }
    assert!(
      html.contains("data-block-id=\"b1\"") && html.contains("data-block-id=\"b5\""),
      "{html}"
    );
    assert!(!html.contains("{#"), "{html}");
  }

  /// T2：页内目录只取 h2 / h3，文字含行内代码，每个 id 都能在渲染结果里找到。
  #[test]
  fn heading_outline_matches_rendered_ids() {
    let md = "---\ntitle: T\n---\n# 标题\n\n## 引言\n\n:::tip\n提示\n:::\n\n### `Vec<T>` 用法\n\n#### 太深\n\n## 引言\n\n## 自定义 {#custom-id}";
    let outline = heading_outline(md);
    let got: Vec<(u8, &str, &str)> =
      outline.iter().map(|e| (e.level, e.id.as_str(), e.text.as_str())).collect();
    assert_eq!(
      got,
      vec![
        (2, "引言", "引言"),
        (3, "vect-用法", "Vec<T> 用法"),
        (2, "引言-1", "引言"),
        (2, "custom-id", "自定义"),
      ]
    );
    let html = render_markdown(md, false);
    for entry in &outline {
      assert!(html.contains(&format!("id=\"{}\"", entry.id)), "缺少 id={}：{html}", entry.id);
    }
  }

  /// 任务列表的勾选框不能丢：`[x]` 是勾上的只读框，`[ ]` 是空框。
  #[test]
  fn task_list_renders_checkboxes() {
    let html = render_markdown("- [x] done\n- [ ] todo", true);
    assert_eq!(html.matches("type=\"checkbox\"").count(), 2, "{html}");
    assert_eq!(html.matches("checked").count(), 1, "{html}");
    assert!(html.contains("disabled"), "{html}");
  }

  /// 脚注按第一次引用的顺序编号（不按标签），引用链到文末的定义，定义链回引用；
  /// id 只用编号，用户写的标签不进 id。
  #[test]
  fn footnotes_number_by_first_reference() {
    let md = "a[^z] b[^a]\n\n[^a]: second\n\n[^z]: first\n\nafter";
    let html = render_markdown(md, true);
    assert!(html.contains("href=\"#fn-1\"") && html.contains("href=\"#fn-2\""), "{html}");
    assert!(html.contains("id=\"fn-1\"") && html.contains("id=\"fnref-1\""), "{html}");
    assert!(!html.contains("fn-z") && !html.contains("fn-a\""), "{html}");
    let first = html.find("first").unwrap_or(0);
    let second = html.find("second").unwrap_or(0);
    let after = html.find("after").unwrap_or(usize::MAX);
    assert!(after < first && first < second, "定义要按编号排在正文之后：{html}");
  }

  /// 表格的列对齐写进单元格，表头和表体都要。
  #[test]
  fn table_alignment_applies_to_cells() {
    let html = render_markdown("| l | c | r |\n| :-- | :-: | --: |\n| 1 | 2 | 3 |", false);
    assert_eq!(html.matches("text-align:center").count(), 2, "{html}");
    assert_eq!(html.matches("text-align:right").count(), 2, "{html}");
    assert_eq!(html.matches("<th ").count(), 3, "{html}");
    assert!(html.contains("<tbody>"), "{html}");
    assert_eq!(html.matches("<td").count(), 3, "{html}");
  }

  /// H4–H6 保持自己的级别；站点内容的 `{.class}` 生效，用户内容忽略。
  #[test]
  fn deep_headings_and_classes() {
    let html = render_markdown("#### four {.note}\n\n##### five\n\n###### six", false);
    assert!(html.contains("<h4") && html.contains("<h5") && html.contains("<h6"), "{html}");
    assert!(html.contains("class=\"note\""), "{html}");
    let html = render_markdown("#### four {.fixed}", true);
    assert!(html.contains("<h4") && !html.contains("fixed\""), "{html}");
  }

  /// 代码块 info 串只取语言名：`rust,ignore` 与 `rust title="x"` 都是 rust。
  #[test]
  fn code_info_keeps_only_language() {
    let html =
      render_markdown("```rust,ignore\nfn a() {}\n```\n\n```py title=\"x.py\"\npass\n```", false);
    assert!(html.contains("language-rust ") && html.contains("language-py "), "{html}");
    assert!(!html.contains("ignore") && !html.contains("title="), "{html}");
  }

  /// M2：表格单元格里的 `<br/>` 是换行，不是文字。
  #[test]
  fn inline_br_is_line_break() {
    let html = render_markdown("| a |\n| --- |\n| x<br/>y<BR>z<br />w |", false);
    assert_eq!(html.matches("<br/>").count() + html.matches("<br>").count(), 3, "{html}");
    assert!(!html.contains("&#60;br"), "{html}");
  }

  /// M3：旧博客的 `<AudioPlayer>`（单行或跨行写法）渲染为播放器；非 https 地址不渲染。
  #[test]
  fn audio_player_renders_audio_element() {
    crate::components::register_default_components();
    let one_line = r#"<AudioPlayer src="https://a.example/x.m4a" title="标题" artist="作者" />"#;
    let multi_line =
      "<AudioPlayer\n  src=\"https://a.example/x.m4a\"\n  title=\"标题\"\n  artist=\"作者\"\n/>";
    for md in [one_line, multi_line] {
      let html = render_markdown(md, false);
      assert!(
        html.contains("<audio") && html.contains("src=\"https://a.example/x.m4a\""),
        "{html}"
      );
      assert!(html.contains("标题") && !html.contains("&#60;AudioPlayer"), "{html}");
    }
    let html = render_markdown(r#"<AudioPlayer src="javascript:alert(1)" />"#, false);
    assert!(!html.contains("<audio") && !html.contains("javascript"), "{html}");
  }

  /// 用户内容的标题不能自定 id（可能与页面元素重名），仍用 `bN`。
  #[test]
  fn untrusted_heading_ids_stay_block_ids() {
    let html = render_markdown("## 引言 {#main}", true);
    assert!(
      html.contains("id=\"b1\"") && !html.contains("main") && !html.contains("引言\""),
      "{html}"
    );
  }

  /// X1：`![描述](x.png)` 的描述作为 alt，内联格式只取文本；没写描述时 alt 为空。
  #[test]
  fn image_description_becomes_alt() {
    for untrusted in [false, true] {
      let html = render_markdown("![架构 *总览* 图](/uploads/a.png)", untrusted);
      assert!(html.contains("alt=\"架构 总览 图\""), "{html}");
      assert!(!html.contains("<em>"), "描述不应作为正文渲染：{html}");
      let html = render_markdown("![](/uploads/a.png)", untrusted);
      assert!(html.contains("alt=\"\""), "{html}");
      assert!(html.contains("loading=\"lazy\""), "{html}");
    }
  }

  #[test]
  fn inline_emphasis_renders_semantic_tags() {
    for untrusted in [false, true] {
      let html = render_markdown("**粗体** *斜体* ~~删除~~", untrusted);
      assert!(html.contains("<strong>粗体</strong>"), "{html}");
      assert!(html.contains("<em>斜体</em>"), "{html}");
      assert!(html.contains("<del>删除</del>"), "{html}");
    }
  }

  #[test]
  fn trusted_images_keep_relative_and_external_sources() {
    assert_eq!(image_src("hero.jpg", "1", false).as_deref(), Some("/posts/welcome/hero.jpg"));
    assert_eq!(image_src("a.png", "rust-intro", false).as_deref(), Some("/posts/rust-intro/a.png"));
    assert_eq!(
      image_src("https://cdn.example.com/x.png", "1", false).as_deref(),
      Some("https://cdn.example.com/x.png")
    );
    assert_eq!(image_src("javascript:alert(1)", "1", false), None);
  }

  #[test]
  fn mermaid_renders_as_diagram_only_in_trusted_content() {
    assert!(renders_as_diagram("mermaid", false));
    assert!(!renders_as_diagram("mermaid", true));
    assert!(!renders_as_diagram("rust", false));
  }

  #[test]
  fn untrusted_images_only_load_uploads() {
    assert_eq!(
      image_src("/uploads/1700_cat_Ab12Cd.png", "comment", true).as_deref(),
      Some("/uploads/1700_cat_Ab12Cd.png")
    );
    // 站内接口、外部图片（泄露 IP）、相对路径（会落到 /posts/... 下）都拒绝
    for bad in ["/api/auth/logout", "https://evil.example/x.png", "x.png", "/uploads/../api/x.png"]
    {
      assert_eq!(image_src(bad, "comment", true), None, "{bad}");
    }
  }

  /// 渲染结果里真正的标签（`<` 后跟字母）的名字和 `on*` 属性，
  /// 转义成文本的 `&lt;script` 不算。
  fn live_tags(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (i, _) in html.match_indices('<') {
      let rest = &html[i + 1..];
      if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        continue;
      }
      let tag = &rest[..rest.find('>').unwrap_or(rest.len())];
      let name: String = tag.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
      found.push(name.to_ascii_lowercase());
      for attr in tag.split_whitespace().skip(1) {
        let attr = attr.to_ascii_lowercase();
        if attr.starts_with("on") && attr.contains('=') {
          found.push(attr);
        }
      }
    }
    found
  }

  /// 用户内容里的原始 HTML 一律当文本输出：渲染出的真实标签里没有 script / iframe /
  /// object / embed / style / svg，也没有任何 `on*=` 属性。只靠事件层，不靠解析前的字符串处理。
  #[test]
  fn untrusted_raw_html_never_becomes_markup() {
    let payloads = [
      "before <script>alert(1)</script> after",
      "<script>\nalert(1)\n</script>",
      "<SCRIPT>x</SCRIPT> <sCrIpT src=\"https://evil.com/x.js\"></sCrIpT>",
      r#"<iframe src="https://evil.com"></iframe> ok"#,
      "<object data='x.swf'></object><embed src='y' /> z",
      "<style>body{display:none}</style> body",
      r#"<a href="/x" onclick="alert(1)">link</a>"#,
      r#"<img src="x" onerror='alert(1)'>"#,
      "<img src=x onerror=alert(1)>",
      "<svg onload=alert(1)><script>alert(2)</script>",
      "<div>\n<img src=x onerror=alert(1)>\n</div>",
      "text <span onmouseover=alert(1)>hover</span>",
      "<Unregistered onload=\"alert(1)\" />",
    ];
    for payload in payloads {
      let html = render_markdown(payload, true);
      for tag in live_tags(&html) {
        assert!(
          !matches!(tag.as_str(), "script" | "iframe" | "object" | "embed" | "style" | "svg")
            && !tag.starts_with("on"),
          "{payload:?} 渲染出了 {tag:?}：{html}"
        );
      }
    }
  }

  /// 注册组件在用户内容里照常渲染（YouTube 的 iframe 是组件自己的），用户写的
  /// `on*` 属性不会带进去：组件只读取自己认识的属性。
  #[test]
  fn untrusted_component_drops_event_attributes() {
    crate::components::register_default_components();
    let html = render_markdown("<YouTube id=\"x\" onload=\"alert(1)\" />", true);
    assert!(html.contains("src=\"https://www.youtube.com/embed/x\""), "{html}");
    assert!(live_tags(&html).iter().all(|tag| !tag.starts_with("on")), "{html}");
  }

  /// 用户贴的代码原样显示：`let one=1;` 不会被当成事件属性删掉，代码里的 `<script>` 也在。
  #[test]
  fn untrusted_code_is_kept_verbatim() {
    let html = render_markdown("`let one=1;`\n\n```html\n<script>alert(1)</script>\n```", true);
    assert!(html.contains("let one=1;"), "{html}");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "{html}");
  }

  #[test]
  fn untrusted_content_has_no_frontmatter() {
    let (meta, body) = split_source("---\ntitle: 站点公告\n---\n普通评论", true);
    assert!(meta.title.is_empty());
    assert!(body.contains("普通评论") && body.contains("title: 站点公告"));
    let (_, body) = split_source("---\n第一段\n\n---\n第二段", true);
    assert!(body.contains("第一段") && body.contains("第二段"));
    // 站点作者的内容照常解析 frontmatter
    let (meta, _) = split_source("---\ntitle: 文章\n---\n正文", false);
    assert_eq!(meta.title, "文章");
  }

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

  /// Z3：其他博客系统常见的 frontmatter 写法都能读出来，缺 title 不连累其他字段。
  #[test]
  fn parse_mdx_tolerates_other_blog_frontmatter() {
    let src = "---\ndate: 2019-3-5 10:20:00\ntags: rust\nkeywords: [a, b]\nsummary: 摘要\nupdated: 2020/01/02\ndraft: true\nauthor: 某人\n---\nbody";
    let (meta, _) = parse_mdx(src);
    assert_eq!(meta.title, "");
    assert_eq!(meta.date.as_deref(), Some("2019-03-05"));
    assert_eq!(meta.updated.as_deref(), Some("2020-01-02"));
    assert_eq!(meta.tags, vec!["rust"]);
    assert_eq!(meta.keywords.as_deref(), Some("a, b"));
    assert_eq!(meta.description.as_deref(), Some("摘要"));
    assert!(meta.draft);
    let (meta, _) =
      parse_mdx("---\ntitle: 2024\ntags: [x, 1]\ndate: 2024-01-02T03:04:05+08:00\n---\n");
    assert_eq!(meta.title, "2024");
    assert_eq!(meta.tags, vec!["x", "1"]);
    assert_eq!(meta.date.as_deref(), Some("2024-01-02T03:04:05+08:00"));
    assert!(!meta.draft);
  }

  #[test]
  fn normalize_date_keeps_unknown_as_none() {
    assert_eq!(normalize_date("2024-1-2").as_deref(), Some("2024-01-02"));
    assert_eq!(normalize_date("2024/01/02").as_deref(), Some("2024-01-02"));
    assert_eq!(normalize_date("yesterday"), None);
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
  fn convert_admonitions_ignores_markers_inside_code_fences() {
    // rustc 的报错里有 `  ::: /path/to/file.rs:68:1` 这样的行
    let input = "```text\nerror[E0599]\n  ::: src/routing/mod.rs:68:1\n   |\n```\nafter\n";
    let out = convert_admonitions(input);
    assert_eq!(out, input);
  }

  #[test]
  fn convert_admonitions_fence_inside_block_keeps_prefix() {
    let input = ":::note\n~~~text\n:::\n~~~\nend\n:::\nafter\n";
    let out = convert_admonitions(input);
    assert_eq!(out, "> [!NOTE]\n> ~~~text\n> :::\n> ~~~\n> end\n\nafter\n");
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

  /// Every `<` that a browser would read as a tag opener must open a MathML
  /// element; anything else in the output is an injected tag.
  fn assert_only_mathml_tags(out: &str) {
    const MATHML: &[&str] = &[
      "math",
      "semantics",
      "annotation",
      "mrow",
      "mi",
      "mn",
      "mo",
      "mtext",
      "mspace",
      "msub",
      "msup",
      "msubsup",
      "munder",
      "mover",
      "munderover",
      "mfrac",
      "msqrt",
      "mroot",
      "mtable",
      "mtr",
      "mtd",
      "mpadded",
      "mphantom",
      "menclose",
      "merror",
      "mstyle",
    ];
    for (i, _) in out.match_indices('<') {
      let rest = out[i + 1..].trim_start_matches('/');
      let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
      if name.is_empty() {
        continue; // `<` followed by a non-letter is text to an HTML parser
      }
      assert!(MATHML.contains(&name.as_str()), "non-MathML tag <{name}> in {out}");
    }
  }

  #[test]
  fn latex_text_payloads_are_escaped() {
    // SEC-01：这些都来自用户可写的评论 / 帖子
    let payloads = [
      r"\text{<img src=x onerror=alert(1)>}",
      r"\text{<img src=x/onerror=alert(1)>}",
      r#"\text{<a href="javascript:alert(1)">x</a>}"#,
      r"\operatorname{<svg/onload=alert(1)>}",
      r"\mathrm{\text{</math><img src=x onerror=alert(1)>}}",
      r"\text{<script>alert(1)</script>}",
    ];
    for latex in payloads {
      for display in [false, true] {
        let out = latex_to_mathml_string(latex, display);
        assert_only_mathml_tags(&out);
        assert!(!out.contains("<img") && !out.contains("<a ") && !out.contains("<svg"), "{out}");
      }
    }
  }

  #[test]
  fn latex_error_messages_are_escaped() {
    // 解析错误会把出错的输入片段写进 <merror>
    for latex in [r"\begin{<img src=x onerror=alert(1)>}", r"\unknown<img src=x onerror=alert(1)>"]
    {
      assert_only_mathml_tags(&latex_to_mathml_string(latex, false));
    }
  }

  #[test]
  fn latex_text_keeps_visible_characters() {
    let out = latex_to_mathml_string(r"\text{a < b & c}", false);
    assert!(out.contains("a &lt; b &amp; c"), "{out}");
  }

  #[test]
  fn latex_common_formulas_render_unchanged() {
    // 不含特殊字符的公式，输出与 pulldown-latex 直出一致
    for latex in [
      r"\frac{a}{b} + x_i^2",
      r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}",
      r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
      r"a < b \le c",
      r"\sin x + \operatorname{arccot} y + \text{if } 3.14",
    ] {
      let storage = Storage::new();
      let mut expected = String::new();
      let config = pulldown_latex::RenderConfig {
        display_mode: pulldown_latex::config::DisplayMode::Inline,
        ..Default::default()
      };
      push_mathml(&mut expected, LatexParser::new(latex, &storage), config).unwrap();
      assert_eq!(latex_to_mathml_string(latex, false), expected, "{latex}");
    }
  }
}
