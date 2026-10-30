use app_core::i18n::Language;
use dioxus::prelude::*;
use dioxus_shadcn::{Empty, EmptyDescription};

use crate::digest::{mindmap_source, PaperDigest};
use crate::server::{get_paper, list_papers, PaperCard, PaperDetail};

fn use_language_signal() -> Signal<Language> {
  try_use_context::<Signal<Language>>().unwrap_or_else(|| Signal::new(Language::Zh))
}

/// 解读按界面语言取；存的语言代码与这里一致。
fn lang_code(lang: Language) -> &'static str {
  match lang {
    Language::En => "en",
    Language::Zh => "zh",
  }
}

/// `/papers`：按 HF 日期分组列出已生成解读的论文。
#[component]
pub fn PapersIndexPage() -> Element {
  let lang = use_language_signal()();
  let title = lang.pick("论文速递", "Papers");
  let subtitle = lang.pick(
    "每天从 Hugging Face Daily Papers 选题，读 arXiv 全文，整理带出处的要点与思维导图（AI 生成）。",
    "Daily picks from Hugging Face Daily Papers, read in full from arXiv, with cited key points and a mind map (AI-generated).",
  );
  rsx! {
      widgets::PageMeta {
          title: title.to_string(),
          description: subtitle.to_string(),
          path: "/papers".to_string(),
      }
      section { class: "py-12 min-h-screen bg-white dark:bg-slate-950",
          div { class: "mx-auto max-w-4xl px-4 sm:px-6 lg:px-8",
              h1 { class: "text-3xl md:text-4xl font-extrabold text-slate-900 dark:text-white", "{title}" }
              p { class: "mt-3 mb-10 text-slate-600 dark:text-slate-400", "{subtitle}" }
              SuspenseBoundary { fallback: |_| rsx! {}, PaperList {} }
          }
      }
  }
}

#[component]
fn PaperList() -> Element {
  let lang_signal = use_language_signal();
  let lang = lang_signal();
  let res = use_server_future(move || {
    let code = lang_code(lang_signal()).to_string();
    async move { list_papers(code).await }
  })?;
  let papers = match res() {
    Some(Ok(list)) => list,
    Some(Err(_)) => {
      return rsx! { widgets::LoadFailed { message: lang.pick("加载失败", "Failed to load").to_string() } }
    }
    None => vec![],
  };
  if papers.is_empty() {
    return rsx! {
        Empty { class: "rounded-2xl py-20",
            EmptyDescription { {lang.pick("还没有论文", "No papers yet")} }
        }
    };
  }
  rsx! {
      for (date, day) in group_by_date(papers) {
          section { key: "{date}", class: "mb-10",
              h2 { class: "text-sm font-semibold text-slate-500 dark:text-slate-400 mb-3", "{date}" }
              ul { class: "divide-y divide-slate-200 dark:divide-slate-800",
                  for p in day {
                      li { key: "{p.arxiv_id}", class: "py-4",
                          div { class: "flex items-baseline gap-3",
                              a {
                                  href: "/papers/{p.arxiv_id}",
                                  class: "flex-1 min-w-0 font-semibold text-slate-900 dark:text-slate-100 hover:underline break-words",
                                  "{p.title}"
                              }
                              span { class: "shrink-0 text-xs text-slate-500", "▲ {p.hf_upvotes}" }
                          }
                          if !p.tldr.is_empty() {
                              p { class: "mt-1 text-sm text-slate-600 dark:text-slate-400", "{p.tldr}" }
                          }
                      }
                  }
              }
          }
      }
  }
}

/// 按日期分组；输入已按日期倒序，组的顺序随之保留。
fn group_by_date(papers: Vec<PaperCard>) -> Vec<(String, Vec<PaperCard>)> {
  let mut groups: Vec<(String, Vec<PaperCard>)> = Vec::new();
  for p in papers {
    match groups.last_mut() {
      Some((date, day)) if *date == p.date => day.push(p),
      _ => groups.push((p.date.clone(), vec![p])),
    }
  }
  groups
}

/// `/papers/:arxiv_id`：单篇解读。
#[component]
pub fn PaperDetailPage(arxiv_id: String) -> Element {
  rsx! {
      section { class: "py-12 min-h-screen bg-white dark:bg-slate-950",
          div { class: "mx-auto max-w-3xl px-4 sm:px-6 lg:px-8",
              SuspenseBoundary { fallback: |_| rsx! {}, PaperDetailLoaded { arxiv_id } }
          }
      }
  }
}

#[component]
fn PaperDetailLoaded(arxiv_id: String) -> Element {
  let lang_signal = use_language_signal();
  let lang = lang_signal();
  let res = use_server_future(use_reactive!(|arxiv_id| {
    let code = lang_code(lang_signal()).to_string();
    async move { get_paper(arxiv_id, code).await }
  }))?;
  match res() {
    Some(Ok(Some(paper))) => rsx! {
        widgets::PageMeta {
            title: paper.title.clone(),
            description: paper.digest.tldr.clone(),
            path: format!("/papers/{}", paper.arxiv_id),
        }
        PaperBody { paper, wanted: lang_code(lang).to_string() }
    },
    Some(Err(_)) => {
      rsx! { widgets::LoadFailed { message: lang.pick("加载失败", "Failed to load").to_string() } }
    }
    _ => {
      rsx! { widgets::NotFound { message: lang.pick("没有这篇论文", "Paper not found").to_string() } }
    }
  }
}

#[component]
fn PaperBody(paper: PaperDetail, wanted: String) -> Element {
  let lang = use_language_signal()();
  let d: &PaperDigest = &paper.digest;
  let id = &paper.arxiv_id;
  let version = paper.arxiv_version.map(|v| format!("v{v}")).unwrap_or_default();
  let mindmap = format!("```mermaid\n{}\n```\n", mindmap_source(&d.mindmap));
  let authors = match paper.authors.len() {
    0 => String::new(),
    n if n > 6 => format!("{} et al.", paper.authors[..6].join(", ")),
    _ => paper.authors.join(", "),
  };
  let section_label = |s: &str| format!("§{}", crate::digest::clean_section(s));
  let link = "underline decoration-slate-300 hover:decoration-slate-500";
  rsx! {
      a { href: "/papers", class: "text-sm text-slate-500 hover:underline", {lang.pick("← 论文速递", "← Papers")} }
      h1 { class: "mt-4 text-2xl md:text-3xl font-extrabold text-slate-900 dark:text-white break-words", "{paper.title}" }
      if !authors.is_empty() {
          p { class: "mt-2 text-sm text-slate-500 break-words", "{authors}" }
      }
      p { class: "mt-3 flex flex-wrap gap-x-4 gap-y-1 text-sm text-slate-600 dark:text-slate-400",
          span { "{paper.date}" }
          span { "▲ {paper.hf_upvotes}" }
          a { class: link, href: "https://arxiv.org/abs/{id}{version}", target: "_blank", rel: "noopener noreferrer", "arXiv {id}{version}" }
          a { class: link, href: "https://huggingface.co/papers/{id}", target: "_blank", rel: "noopener noreferrer", "Hugging Face" }
          if let Some(repo) = paper.github_repo.as_ref().filter(|r| r.starts_with("https://")) {
              a { class: link, href: "{repo}", target: "_blank", rel: "noopener noreferrer", {lang.pick("代码", "Code")} }
          }
      }
      div { class: "mt-6 rounded-lg border border-amber-300/60 bg-amber-50 dark:bg-amber-950/30 dark:border-amber-800/60 px-4 py-3 text-sm text-amber-900 dark:text-amber-200",
          p {
              {lang.pick("本页由 AI 根据论文全文生成（", "AI-generated from the full paper (")}
              "{paper.model}"
              {lang.pick("），可能有误，请以原文为准。", "). It may contain mistakes; check the original.")}
          }
          if !paper.has_html {
              p { class: "mt-1", {lang.pick("这篇论文没有 HTML 全文，解读只依据摘要。", "No HTML full text was available; this digest is based on the abstract only.")} }
          }
          if paper.lang != wanted {
              p { class: "mt-1", {lang.pick("暂无中文版，下面显示英文。", "No version in this language yet; showing English.")} }
          }
      }

      DigestSection { title: lang.pick("一句话", "TL;DR").to_string(),
          p { class: "text-lg text-slate-800 dark:text-slate-200", "{d.tldr}" }
      }
      DigestSection { title: lang.pick("要解决的问题", "Problem").to_string(),
          p { "{d.problem}" }
      }
      DigestSection { title: lang.pick("方法", "Method").to_string(),
          p { "{d.method}" }
      }
      DigestSection { title: lang.pick("实验结果", "Results").to_string(),
          ul { class: "list-disc pl-5 space-y-2",
              for (i, r) in d.results.iter().enumerate() {
                  li { key: "{i}",
                      "{r.claim} "
                      span { class: "text-xs text-slate-500", "{section_label(&r.section)}" }
                      for n in r.numbers.iter().filter(|n| paper.unverified_numbers.contains(n)) {
                          span { class: "ml-2 text-xs text-amber-700 dark:text-amber-400",
                              "{n} "
                              {lang.pick("（原文中未核实）", "(not found in the paper)")}
                          }
                      }
                  }
              }
          }
      }
      DigestSection { title: lang.pick("局限", "Limitations").to_string(),
          ul { class: "list-disc pl-5 space-y-1",
              for (i, l) in d.limitations.iter().enumerate() {
                  li { key: "{i}", "{l}" }
              }
          }
      }
      DigestSection { title: lang.pick("要点", "Key points").to_string(),
          ul { class: "list-disc pl-5 space-y-2",
              for (i, k) in d.key_points.iter().enumerate() {
                  li { key: "{i}",
                      "{k.text} "
                      span { class: "text-xs text-slate-500", "{section_label(&k.section)}" }
                  }
              }
          }
      }
      DigestSection { title: lang.pick("思维导图", "Mind map").to_string(),
          div { class: "overflow-x-auto", widgets::Markdown { content: mindmap, blog_id: format!("papers/{id}") } }
      }
  }
}

#[component]
fn DigestSection(title: String, children: Element) -> Element {
  rsx! {
      section { class: "mt-10",
          h2 { class: "text-lg font-bold text-slate-900 dark:text-white mb-3", "{title}" }
          div { class: "text-slate-700 dark:text-slate-300 leading-relaxed break-words", {children} }
      }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn card(id: &str, date: &str) -> PaperCard {
    PaperCard {
      arxiv_id: id.into(),
      title: id.into(),
      date: date.into(),
      hf_upvotes: 0,
      tldr: String::new(),
    }
  }

  #[test]
  fn groups_consecutive_dates_in_order() {
    let groups = group_by_date(vec![
      card("a", "2026-10-28"),
      card("b", "2026-10-28"),
      card("c", "2026-10-27"),
    ]);
    let shape: Vec<(String, usize)> = groups.iter().map(|(d, v)| (d.clone(), v.len())).collect();
    assert_eq!(shape, vec![("2026-10-28".into(), 2), ("2026-10-27".into(), 1)]);
  }
}
