use app_core::i18n::Language;
use dioxus::prelude::*;
use dioxus_shadcn::{Empty, EmptyDescription};

use crate::server::{list_papers, PaperCard};

fn use_language_ctx() -> Language {
  try_use_context::<Signal<Language>>().map(|s| s()).unwrap_or(Language::Zh)
}

/// `/papers`：按 HF 日期分组列出已生成解读的论文。
#[component]
pub fn PapersIndexPage() -> Element {
  let lang = use_language_ctx();
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
  let lang = use_language_ctx();
  let res = use_server_future(|| async move { list_papers().await })?;
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
                      li { key: "{p.arxiv_id}", class: "py-3 flex items-baseline gap-3",
                          a {
                              href: "/papers/{p.arxiv_id}",
                              class: "flex-1 min-w-0 font-medium text-slate-900 dark:text-slate-100 hover:underline",
                              "{p.title}"
                          }
                          span { class: "shrink-0 text-xs text-slate-500", "▲ {p.hf_upvotes}" }
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

#[cfg(test)]
mod tests {
  use super::*;

  fn card(id: &str, date: &str) -> PaperCard {
    PaperCard { arxiv_id: id.into(), title: id.into(), date: date.into(), hf_upvotes: 0 }
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
