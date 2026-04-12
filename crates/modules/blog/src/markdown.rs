use dioxus::document::eval;
use dioxus::prelude::*;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(PartialEq, Props, Clone)]
pub struct MarkdownProps {
  content: String,
}

#[derive(Default)]
struct PostMetadata {
  title: String,
  tags: Vec<String>,
  description: String,
  date: String,
}

fn parse_frontmatter(content: &str) -> (PostMetadata, &str) {
  if !content.starts_with("---") {
    return (PostMetadata::default(), content);
  }

  let Some(end_idx) = content[3..].find("---") else {
    return (PostMetadata::default(), content);
  };

  let frontmatter_str = &content[3..end_idx + 3];
  let body = &content[end_idx + 6..];

  let mut meta = PostMetadata::default();

  for line in frontmatter_str.lines() {
    let line = line.trim();
    if let Some((key, value)) = line.split_once(':') {
      let key = key.trim();
      let value = value.trim();

      match key {
        "title" => meta.title = value.trim_matches('"').to_string(),
        "description" => meta.description = value.trim_matches('"').to_string(),
        "date" => meta.date = value.trim_matches('"').to_string(),
        "tags" => {
          // Handle [Tag1, Tag2] format
          let tags_content = value.trim_start_matches('[').trim_end_matches(']');
          meta.tags = tags_content
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        }
        _ => {}
      }
    }
  }

  (meta, body)
}

use crate::components::podcast::{Episode, EPISODES};

/// A component that renders Markdown content and supports embedding components (MDX-style).
///
/// It uses `pulldown-cmark` to parse the markdown.
/// It detects specific HTML patterns to inject Dioxus components.
#[component]
pub fn Markdown(props: MarkdownProps) -> Element {
  // We collect elements to render here
  let mut rendered_elements = Vec::new();

  let _ = eval(
    r#"
        const init = () => {
            // Highlight code
            if (window.Prism) { 
                window.Prism.highlightAll();
                addCopyButtons();
            }
            
            // Render Math (Manual rendering for better control)
            if (window.katex) {
                document.querySelectorAll('.katex-math').forEach(el => {
                    if (el.getAttribute('data-processed')) return;
                    const tex = el.textContent;
                    const displayMode = el.classList.contains('display-math');
                    try {
                        window.katex.render(tex, el, {
                            displayMode: displayMode,
                            throwOnError: false
                        });
                        el.setAttribute('data-processed', 'true');
                    } catch (e) {
                        console.error('KaTeX error:', e);
                    }
                });
            }
        };

        const addCopyButtons = () => {
            document.querySelectorAll('pre').forEach(pre => {
                // Check if button already exists
                if (pre.querySelector('.copy-btn')) return;
                
                // Create wrapper if not exists
                let wrapper = pre.parentElement;
                if (!wrapper.classList.contains('code-wrapper')) {
                    wrapper = document.createElement('div');
                    wrapper.className = 'code-wrapper relative group';
                    pre.parentNode.insertBefore(wrapper, pre);
                    wrapper.appendChild(pre);
                }
                
                const button = document.createElement('button');
                button.className = 'copy-btn';
                button.innerHTML = `
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                    </svg>
                `;
                
                button.addEventListener('click', async () => {
                    const code = pre.querySelector('code')?.innerText;
                    if (code) {
                        try {
                            await navigator.clipboard.writeText(code);
                            const originalIcon = button.innerHTML;
                            button.innerHTML = `
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-green-400">
                                    <polyline points="20 6 9 17 4 12"></polyline>
                                </svg>
                            `;
                            setTimeout(() => {
                                button.innerHTML = originalIcon;
                            }, 2000);
                        } catch (err) {
                            console.error('Failed to copy:', err);
                        }
                    }
                });

                wrapper.appendChild(button);
            });
        };

        // Run immediately and wait for libs to load
        init();
        // Retry a few times in case scripts load async
        setTimeout(init, 100);
        setTimeout(init, 500);
        setTimeout(init, 1000);
        setTimeout(init, 2000);
        setTimeout(init, 4000);
        
        // Add minimal CSS for code blocks if not present
        if (!document.getElementById('code-block-styles')) {
            const style = document.createElement('style');
            style.id = 'code-block-styles';
            style.textContent = `
                /* Copy button styles */
                .copy-btn {
                    position: absolute;
                    top: 0.5rem;
                    right: 0.5rem;
                    padding: 0.375rem; /* p-1.5 */
                    border-radius: 0.375rem; /* rounded-md */
                    color: #94a3b8; /* text-slate-400 */
                    background-color: transparent;
                    transition: all 150ms ease-in-out;
                    opacity: 0;
                    cursor: pointer;
                    z-index: 10;
                }
                .copy-btn:hover {
                    color: #ffffff;
                    background-color: rgba(51, 65, 85, 0.5); /* bg-slate-700/50 */
                }
                .code-wrapper:hover .copy-btn {
                    opacity: 1;
                }
                pre {
                    margin: 0 !important; /* Reset margin since wrapper handles it */
                }

                /* Tags matching screenshot style */
                .tag-badge {
                    background-color: #fff;
                    border: 1px solid #e2e8f0;
                    border-radius: 4px;
                    padding: 2px 8px;
                    font-size: 12px;
                    color: #475569;
                    display: inline-flex;
                    align-items: center;
                    margin-right: 8px;
                    font-weight: 500;
                }
                .dark .tag-badge {
                    background-color: #1e293b;
                    border-color: #334155;
                    color: #cbd5e1;
                }

                /* Inline code matching screenshot style */
                :not(pre) > code {
                    background-color: #f1f5f9; /* slate-100 */
                    padding: 2px 6px;
                    border-radius: 4px;
                    font-size: 0.9em;
                    color: #0f172a;
                    border: 1px solid #e2e8f0;
                    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', 'Consolas', 'source-code-pro', monospace;
                }
                :not(pre) > code::before,
                :not(pre) > code::after {
                    content: none !important;
                }
                .dark :not(pre) > code {
                    background-color: #1e293b;
                    border-color: #334155;
                    color: #e2e8f0;
                }

                /* Force table styling for prose content */
                .prose table {
                    width: 100%;
                    border-collapse: collapse;
                    margin: 2em 0;
                    font-size: 0.9em;
                }
                .prose thead {
                    border-bottom: 2px solid #e2e8f0;
                    background-color: #f8fafc;
                }
                .dark .prose thead {
                    border-bottom-color: #334155;
                    background-color: #1e293b;
                }
                .prose th, .prose td {
                    padding: 0.75em 1em;
                    border: 1px solid #e2e8f0;
                    text-align: left;
                }
                .dark .prose th, .dark .prose td {
                    border-color: #334155;
                }
                .prose th {
                    font-weight: 600;
                    color: #1e293b;
                }
                .dark .prose th {
                    color: #e2e8f0;
                }
                .prose tr:nth-child(even) {
                    background-color: #f8fafc;
                }
                .dark .prose tr:nth-child(even) {
                    background-color: #1e293b;
                }

                /* Mac-style code block header */
                pre[class*="language-"] {
                    position: relative;
                    padding-top: 40px !important;
                    background-color: #f8fafc !important; /* slate-50 */
                    border: 1px solid #e2e8f0;
                    border-radius: 8px;
                }
                .dark pre[class*="language-"] {
                    background-color: #0f172a !important;
                    border-color: #334155;
                }
                
                /* The dots */
                pre[class*="language-"]::before {
                    content: "";
                    position: absolute;
                    top: 14px;
                    left: 14px;
                    width: 10px;
                    height: 10px;
                    border-radius: 50%;
                    background: #ff5f56;
                    box-shadow: 18px 0 0 #ffbd2e, 36px 0 0 #27c93f;
                }
            `;
            document.head.appendChild(style);
        }
    "#,
  );

  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_FOOTNOTES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);
  options.insert(Options::ENABLE_MATH);
  let (metadata, content_to_parse) = parse_frontmatter(&props.content);

  // Render Header if metadata exists
  if !metadata.title.is_empty() {
    rendered_elements.push(rsx! {
            // Add Katex CSS and JS
            document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.css" }
            document::Script { src: "https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.js" }

            div { class: "mb-10 pb-8 border-b border-slate-200 dark:border-slate-800",
                h1 { class: "text-3xl md:text-4xl font-extrabold tracking-tight text-slate-900 dark:text-white mb-6 leading-tight",
                    "{metadata.title}"
                }
                if !metadata.description.is_empty() {
                    div { class: "text-lg text-slate-600 dark:text-slate-300 mb-6 leading-relaxed",
                        "{metadata.description}"
                    }
                }
                div { class: "flex flex-wrap gap-2 items-center",
                    if !metadata.date.is_empty() {
                        div { class: "text-sm font-medium text-slate-500 mr-4 flex items-center gap-1", 
                            span { class: "opacity-75", "📅" }
                            "{metadata.date}" 
                        }
                    }
                    for tag in metadata.tags {
                        span { class: "tag-badge",
                            "{tag}"
                        }
                    }
                }
            }
        });
  }

  // Buffer for standard markdown events that will be rendered as HTML
  let mut markdown_buffer = Vec::new();
  let parser = Parser::new_ext(content_to_parse, options);

  // Track if we need to skip the current heading (used to hide the first H1 if it duplicates metadata)
  let mut skip_current_heading = false;
  let mut has_skipped_h1 = false;

  for event in parser {
    match event {
      // Skip H1 if we have a title in metadata
      Event::Start(Tag::Heading {
        level: HeadingLevel::H1,
        ..
      }) if !metadata.title.is_empty() && !has_skipped_h1 => {
        skip_current_heading = true;
        has_skipped_h1 = true;
        continue;
      }
      Event::End(TagEnd::Heading(HeadingLevel::H1)) if skip_current_heading => {
        skip_current_heading = false;
        continue;
      }
      _ if skip_current_heading => {
        continue;
      }

      // Detect our custom component syntax
      // Example: <PodcastCard id="1" />
      // Example: <YouTube id="video_id" />
      // Example: <Bilibili id="bvid" />
      Event::Html(ref html)
        if html.trim().starts_with("<PodcastCard")
          || html.trim().starts_with("<YouTube")
          || html.trim().starts_with("<Bilibili") =>
      {
        let html_trim = html.trim();

        // 1. Flush pending markdown
        if !markdown_buffer.is_empty() {
          let mut html_output = String::new();
          pulldown_cmark::html::push_html(&mut html_output, markdown_buffer.drain(..));
          rendered_elements.push(rsx! {
              div { dangerous_inner_html: "{html_output}" }
          });
        }

        // Handle YouTube Component
        if html_trim.starts_with("<YouTube") {
          let id = html
            .split("id=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("");

          if !id.is_empty() {
            rendered_elements.push(rsx! {
                            div { class: "my-8 not-prose",
                                div { class: "aspect-video rounded-xl overflow-hidden bg-slate-900 shadow-lg",
                                    iframe {
                                        src: "https://www.youtube.com/embed/{id}",
                                        title: "YouTube video player",
                                        // frameborder: "0", // Deprecated in HTML5, use CSS border: none instead (which Tailwind reset handles)
                                        allow: "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share",
                                        allowfullscreen: true,
                                        class: "w-full h-full min-h-[400px] border-none"
                                    }
                                }
                            }
                        });
          }
          continue;
        }

        // Handle Bilibili Component
        if html_trim.starts_with("<Bilibili") {
          let id = html
            .split("id=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("");

          if !id.is_empty() {
            rendered_elements.push(rsx! {
                            div { class: "my-8 not-prose",
                                div { class: "aspect-video rounded-xl overflow-hidden bg-slate-900 shadow-lg",
                                    iframe {
                                        // Bilibili iframe requires specific parameters for clean embedding
                                        src: "https://player.bilibili.com/player.html?bvid={id}&high_quality=1&danmaku=0",
                                        title: "Bilibili video player",
                                        // frameborder: "0",
                                        allowfullscreen: true,
                                        class: "w-full h-full min-h-[400px] border-none"
                                    }
                                }
                            }
                        });
          }
          continue;
        }

        // Handle Podcast Component
        // 2. Extract ID (Simple parsing for demo)
        // In a real app, use a regex or proper attribute parser
        let id = html
          .split("id=\"")
          .nth(1)
          .and_then(|s| s.split('"').next())
          .unwrap_or("0");

        // 3. Render the specific Dioxus component
        // Find episode by ID
        let episode_id = id.parse::<i32>().unwrap_or(0);
        let episode = EPISODES.iter().find(|e| e.id == episode_id);

        if let Some(ep) = episode {
          rendered_elements.push(rsx! {
                        div { class: "my-8 not-prose",
                            div { class: "rounded-xl border border-blue-200 bg-blue-50 p-4 dark:bg-blue-900/20 dark:border-blue-800 flex items-start gap-4",
                                // Play Icon
                                div { class: "flex-shrink-0 w-12 h-12 bg-blue-600 rounded-full flex items-center justify-center text-white",
                                    svg { xmlns: "http://www.w3.org/2000/svg", class: "w-6 h-6 ml-1", fill: "currentColor", view_box: "0 0 24 24",
                                        path { d: "M8 5v14l11-7z" }
                                    }
                                }
                                div { class: "flex-1",
                                    div { class: "text-xs font-bold text-blue-600 dark:text-blue-400 uppercase tracking-wide mb-1", "Podcast Episode #{ep.id}" }
                                    div { class: "font-bold text-lg text-slate-900 dark:text-white mb-1", "{ep.title}" }
                                    div { class: "text-sm text-slate-600 dark:text-slate-300 mb-3", "{ep.desc}" }
                                    div { class: "flex items-center gap-4 text-xs font-medium text-slate-500",
                                        span { class: "flex items-center gap-1",
                                            svg { xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2",
                                                circle { cx: "12", cy: "12", r: "10" }
                                                polyline { points: "12 6 12 12 16 14" }
                                            }
                                            "{ep.duration}"
                                        }
                                        span { class: "flex items-center gap-1",
                                            svg { xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2",
                                                rect { x: "3", y: "4", width: "18", height: "18", rx: "2", ry: "2" }
                                                line { x1: "16", y1: "2", x2: "16", y2: "6" }
                                                line { x1: "8", y1: "2", x2: "8", y2: "6" }
                                                line { x1: "3", y1: "10", x2: "21", y2: "10" }
                                            }
                                            "{ep.date}"
                                        }
                                    }
                                    audio { class: "w-full mt-3 h-8", controls: true, src: "{ep.url}" }
                                }
                            }
                        }
                    });
        } else {
          rendered_elements.push(rsx! {
                        div { class: "my-8 not-prose p-4 border border-red-200 bg-red-50 text-red-600 rounded-lg",
                             "Podcast episode #{id} not found."
                        }
                    });
        }
      }
      // Handle Inline Math
      Event::InlineMath(cow) => {
        let math_content = cow.to_string();
        // Flush pending markdown
        if !markdown_buffer.is_empty() {
          let mut html_output = String::new();
          pulldown_cmark::html::push_html(&mut html_output, markdown_buffer.drain(..));
          rendered_elements.push(rsx! {
              span { dangerous_inner_html: "{html_output}" }
          });
        }

        // Push raw LaTeX content wrapped in a span for KaTeX to find
        rendered_elements.push(rsx! {
            span { class: "katex-math inline-math", "{math_content}" }
        });
      }
      // Handle Display Math
      Event::DisplayMath(cow) => {
        let math_content = cow.to_string();
        // Flush pending markdown
        if !markdown_buffer.is_empty() {
          let mut html_output = String::new();
          pulldown_cmark::html::push_html(&mut html_output, markdown_buffer.drain(..));
          rendered_elements.push(rsx! {
              div { dangerous_inner_html: "{html_output}" }
          });
        }

        // Push raw LaTeX content wrapped in a div for KaTeX to find
        rendered_elements.push(rsx! {
            div { class: "katex-math display-math", "{math_content}" }
        });
      }
      // Standard markdown event
      _ => {
        markdown_buffer.push(event);
      }
    }
  }

  // Flush any remaining markdown
  if !markdown_buffer.is_empty() {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, markdown_buffer.into_iter());
    rendered_elements.push(rsx! {
        div { dangerous_inner_html: "{html_output}" }
    });
  }

  rsx! {
      // We use the 'prose' class from @tailwindcss/typography for nice defaults
      // Customized headings and spacing for better readability
      article { class: "prose prose-slate prose-lg dark:prose-invert max-w-none leading-relaxed
            prose-headings:font-extrabold 
            prose-h1:text-4xl prose-h1:mb-8 
            prose-h2:text-3xl prose-h2:mt-10 prose-h2:mb-5 prose-h2:border-b prose-h2:pb-2 prose-h2:border-slate-200 dark:prose-h2:border-slate-800
            prose-h3:text-2xl prose-h3:mt-8 prose-h3:mb-3
            prose-p:my-5 prose-li:my-1.5",
          for element in rendered_elements {
              {element}
          }
      }
  }
}
