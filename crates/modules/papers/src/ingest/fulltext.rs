//! arXiv 全文：`arxiv.org/html/<id>`（LaTeXML 生成），抽出正文和节号。
//!
//! 输出给模型的文本每节以 `[§<节号>] <标题>` 开头，模型引用出处时只能用这些节号。
//! 没有编号的节（Limitations 等）用标题当节号。公式换成 `$<LaTeX>$`。

use app_core::error::with_sources;
use scraper::{node::Node, ElementRef, Html, Selector};

/// 这些块不进正文：参考文献、作者、脚注。
const SKIP_CLASSES: [&str; 4] =
  ["ltx_bibliography", "ltx_authors", "ltx_role_footnote", "ltx_note"];

#[derive(Debug, Clone, PartialEq)]
pub struct FullText {
  pub title: String,
  /// 出现过的节号，顺序同原文
  pub sections: Vec<String>,
  /// 喂给模型的全文
  pub text: String,
}

fn selector(s: &str) -> Selector {
  match Selector::parse(s) {
    Ok(sel) => sel,
    Err(e) => unreachable!("固定的选择器字面量 {s} 解析失败：{e}"),
  }
}

fn has_class(el: &ElementRef, class: &str) -> bool {
  el.value().classes().any(|c| c == class)
}

fn skipped(el: &ElementRef) -> bool {
  SKIP_CLASSES.iter().any(|c| has_class(el, c))
}

fn collect_text(el: ElementRef, out: &mut String) {
  for child in el.children() {
    match child.value() {
      Node::Text(t) => out.push_str(t),
      Node::Element(e) => {
        let Some(child_el) = ElementRef::wrap(child) else { continue };
        if e.name() == "math" {
          out.push_str(" $");
          out.push_str(e.attr("alttext").unwrap_or_default());
          out.push_str("$ ");
        } else if !skipped(&child_el) {
          out.push(' ');
          collect_text(child_el, out);
        }
      }
      _ => {}
    }
  }
}

fn text_of(el: ElementRef) -> String {
  let mut raw = String::new();
  collect_text(el, &mut raw);
  raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_heading(el: &ElementRef) -> bool {
  matches!(el.value().name(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") || has_class(el, "ltx_title")
}

/// 节号：标题里 `.ltx_tag` 的文字去掉 `Appendix` / `Section` 前缀和末尾的点；
/// 没有编号就用标题本身。
fn section_label(heading: ElementRef) -> String {
  let tag_sel = selector(".ltx_tag");
  if let Some(tag) = heading.select(&tag_sel).next() {
    let t = text_of(tag);
    let t = t.trim_start_matches("Appendix ").trim_start_matches("Section ");
    let t = t.trim_end_matches(['.', ' ']);
    if !t.is_empty() {
      return t.to_string();
    }
  }
  text_of(heading)
}

/// 解析 LaTeXML HTML。既没有摘要也没有任何节时返回 `None`（不是论文正文页）。
pub fn parse(html: &str) -> Option<FullText> {
  let doc = Html::parse_document(html);
  let root = doc.select(&selector("article")).next().unwrap_or_else(|| doc.root_element());
  let title = root.select(&selector(".ltx_title_document")).next().map(text_of).unwrap_or_default();

  let mut parts = Vec::new();
  let mut sections = Vec::new();
  if let Some(abs) = root.select(&selector(".ltx_abstract")).next() {
    parts.push(format!("[§Abstract]\n{}", text_of(abs)));
  }
  let section_sel = selector(
    "section.ltx_section, section.ltx_appendix, section.ltx_subsection, section.ltx_subsubsection",
  );
  for sec in root.select(&section_sel) {
    let children: Vec<ElementRef> = sec.children().filter_map(ElementRef::wrap).collect();
    let heading = children.iter().copied().find(is_heading);
    let label = heading.map(section_label).unwrap_or_default();
    let head = heading.map(text_of).unwrap_or_default();
    let body: Vec<String> = children
      .iter()
      .filter(|c| c.value().name() != "section" && !is_heading(c) && !skipped(c))
      .map(|c| text_of(*c))
      .filter(|t| !t.is_empty())
      .collect();
    if label.is_empty() && body.is_empty() {
      continue;
    }
    if !label.is_empty() && !sections.contains(&label) {
      sections.push(label.clone());
    }
    parts.push(format!("[§{label}] {head}\n{}", body.join("\n")));
  }
  if parts.is_empty() {
    return None;
  }
  Some(FullText { title, sections, text: parts.join("\n\n") })
}

/// 版本号：`arxiv.org/html/<id>` 不跳转，版本只出现在页面里（`<id>v2` 这样的链接与文字），
/// 取其中最大的。
pub fn version_in_html(html: &str, arxiv_id: &str) -> Option<i32> {
  let needle = format!("{arxiv_id}v");
  html
    .match_indices(&needle)
    .filter_map(|(i, _)| {
      let digits: String =
        html[i + needle.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
      digits.parse().ok()
    })
    .max()
}

/// 下载全文。没有 HTML 版（404）时返回 `Ok(None)`。
pub async fn fetch(
  client: &reqwest::Client,
  arxiv_id: &str,
) -> Result<Option<(FullText, Option<i32>)>, String> {
  let url = format!("https://arxiv.org/html/{arxiv_id}");
  let resp = client.get(&url).send().await.map_err(|e| format!("arXiv 请求失败：{}", with_sources(&e)))?;
  let status = resp.status();
  if status == reqwest::StatusCode::NOT_FOUND {
    return Ok(None);
  }
  if !status.is_success() {
    return Err(format!("arXiv 返回 {status}"));
  }
  let html = resp.text().await.map_err(|e| format!("arXiv 响应读取失败：{}", with_sources(&e)))?;
  let version = version_in_html(&html, arxiv_id);
  Ok(parse(&html).map(|ft| (ft, version)))
}

#[cfg(test)]
mod tests {
  use super::*;

  const HTML: &str = r#"<html><body><article>
    <h1 class="ltx_title ltx_title_document">A <em>Fast</em> Model</h1>
    <div class="ltx_authors">Someone</div>
    <div class="ltx_abstract"><h6 class="ltx_title">Abstract</h6><p>We go fast.</p></div>
    <section class="ltx_section"><h2 class="ltx_title"><span class="ltx_tag">3 </span>Method</h2>
      <p>Speed is <math alttext="49\times"><mi>x</mi></math> higher.<span class="ltx_note">fn</span></p>
      <section class="ltx_subsection"><h3 class="ltx_title"><span class="ltx_tag">3.1 </span>Setup</h3>
        <p>Details.</p></section>
    </section>
    <section class="ltx_section"><h2 class="ltx_title">Limitations</h2><p>Small data.</p></section>
    <section class="ltx_appendix"><h2 class="ltx_title"><span class="ltx_tag">Appendix A </span>Extra</h2>
      <p>More.</p></section>
    <section class="ltx_bibliography"><h2>References</h2><p>[1] x</p></section>
  </article></body></html>"#;

  #[test]
  fn extracts_sections_math_and_skips_notes() {
    let ft = parse(HTML).unwrap();
    assert_eq!(ft.title, "A Fast Model");
    assert_eq!(ft.sections, ["3", "3.1", "Limitations", "A"]);
    assert!(ft.text.starts_with("[§Abstract]\nAbstract We go fast."), "{}", ft.text);
    assert!(ft.text.contains("[§3] 3 Method\nSpeed is $49\\times$ higher."), "{}", ft.text);
    // 子节的正文只出现在子节里
    assert_eq!(ft.text.matches("Details.").count(), 1);
    assert!(ft.text.contains("[§Limitations] Limitations\nSmall data."));
    assert!(!ft.text.contains("fn") && !ft.text.contains("Someone") && !ft.text.contains("[1]"));
  }

  #[test]
  fn page_without_paper_body_is_none() {
    assert_eq!(parse("<html><body><p>No HTML for this paper</p></body></html>"), None);
  }

  #[test]
  fn version_is_the_largest_mentioned() {
    let html = r#"<a href="/abs/2510.12345v1">v1</a> 2510.12345v3 cites 2510.99999v9"#;
    assert_eq!(version_in_html(html, "2510.12345"), Some(3));
    assert_eq!(version_in_html("2510.12345 only", "2510.12345"), None);
  }
}
