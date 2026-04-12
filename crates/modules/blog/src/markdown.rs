use dioxus::prelude::*;
use pulldown_cmark::{Options, Parser, Tag, TagEnd};
use rustineverything_module_podcast::podcast::{Episode, EPISODES};

#[derive(Props, Clone, PartialEq)]
pub struct MarkdownProps {
  content: String,
}

pub fn parse_markdown_metadata(content: &str) -> (std::collections::HashMap<String, String>, String) {
  let mut metadata = std::collections::HashMap::new();
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

  let parser = Parser::new_ext(&body, options);

  let mut current_block = Vec::new();
  let mut elements = Vec::new();

  for event in parser {
    match event {
      pulldown_cmark::Event::Start(tag) => {
        current_block.push(tag);
      }
      pulldown_cmark::Event::End(tag_end) => {
        let tag = current_block.pop().unwrap();
        let element = render_tag(tag, tag_end, &mut elements);
        if current_block.is_empty() {
          elements.push(element);
        }
      }
      pulldown_cmark::Event::Text(text) => {
        if current_block.is_empty() {
          elements.push(rsx! { "{text}" });
        }
      }
      pulldown_cmark::Event::Code(code) => {
        elements.push(rsx! {
            code { class: "bg-slate-100 dark:bg-slate-800 px-1 rounded", "{code}" }
        });
      }
      _ => {}
    }
  }

  rsx! {
      div { class: "prose prose-slate dark:prose-invert max-w-none",
          {elements.into_iter()}
      }
  }
}

fn render_tag(tag: Tag, _tag_end: TagEnd, _children: &mut Vec<Element>) -> Element {
  match tag {
    Tag::Heading { level, .. } => {
      let level_num = level as u32;
      match level_num {
        1 => rsx! { h1 { "Heading 1" } },
        2 => rsx! { h2 { "Heading 2" } },
        _ => rsx! { h3 { "Heading" } },
      }
    }
    Tag::Paragraph => rsx! { p { "Paragraph" } },
    Tag::Link { dest_url, .. } => rsx! { a { href: "{dest_url}", "Link" } },
    Tag::Image { dest_url, .. } => rsx! { img { src: "{dest_url}" } },
    _ => rsx! { div { "Other" } },
  }
}
