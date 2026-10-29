//! 生成结果的校验：出处节号必须存在、导图不超限、数字能在原文找到，翻译不改结构。

use crate::digest::{MindNode, PaperDigest};

/// 导图在根以下最多几层。
pub const MINDMAP_MAX_DEPTH: usize = 3;
/// 导图任一节点最多几个子节点。
pub const MINDMAP_MAX_CHILDREN: usize = 6;

/// 通过校验后的附带信息。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Checked {
  /// 原文里没找到的数字（不删，页面上标「未核实」）
  pub unverified_numbers: Vec<String>,
}

/// 校验英文解读。`sections` 是原文里存在的节号（含无编号节的标题），
/// `source` 是喂给模型的全文。返回 `Err` 时列出全部问题，用来让模型重写一次。
pub fn check_digest(
  d: &PaperDigest,
  sections: &[String],
  source: &str,
) -> Result<Checked, Vec<String>> {
  let mut problems = Vec::new();
  if d.tldr.trim().is_empty() {
    problems.push("tldr is empty".to_string());
  }
  if d.results.is_empty() || d.key_points.is_empty() {
    problems.push("results and key_points must not be empty".to_string());
  }
  let cited = d
    .results
    .iter()
    .map(|r| ("results", &r.section))
    .chain(d.key_points.iter().map(|k| ("key_points", &k.section)));
  for (kind, section) in cited {
    let s = clean_section(section);
    if s != "Abstract" && !sections.iter().any(|x| x == s) {
      problems.push(format!("{kind}: section \"{s}\" does not exist in the paper"));
    }
  }
  if d.mindmap.depth() > MINDMAP_MAX_DEPTH {
    problems.push(format!(
      "mindmap has {} levels below the root, at most {MINDMAP_MAX_DEPTH} allowed",
      d.mindmap.depth()
    ));
  }
  if d.mindmap.widest() > MINDMAP_MAX_CHILDREN {
    problems.push(format!(
      "a mindmap node has {} children, at most {MINDMAP_MAX_CHILDREN} allowed",
      d.mindmap.widest()
    ));
  }
  if !problems.is_empty() {
    return Err(problems);
  }

  let src = normalize_number(source);
  let unverified_numbers = d
    .results
    .iter()
    .flat_map(|r| r.numbers.iter())
    .filter(|n| n.chars().any(|c| c.is_ascii_digit()) && !src.contains(&normalize_number(n)))
    .cloned()
    .collect();
  Ok(Checked { unverified_numbers })
}

/// 翻译只能改文字：条目数、节号、数字、导图形状都要和英文版一样。
pub fn check_translation(en: &PaperDigest, tr: &PaperDigest) -> Result<(), Vec<String>> {
  let mut problems = Vec::new();
  let same_results = en.results.len() == tr.results.len()
    && en.results.iter().zip(&tr.results).all(|(a, b)| {
      clean_section(&a.section) == clean_section(&b.section) && a.numbers == b.numbers
    });
  if !same_results {
    problems.push("results: count, sections or numbers changed".to_string());
  }
  let same_points = en.key_points.len() == tr.key_points.len()
    && en
      .key_points
      .iter()
      .zip(&tr.key_points)
      .all(|(a, b)| clean_section(&a.section) == clean_section(&b.section));
  if !same_points {
    problems.push("key_points: count or sections changed".to_string());
  }
  if en.limitations.len() != tr.limitations.len() {
    problems.push("limitations: count changed".to_string());
  }
  if !same_shape(&en.mindmap, &tr.mindmap) {
    problems.push("mindmap: shape changed".to_string());
  }
  if problems.is_empty() {
    Ok(())
  } else {
    Err(problems)
  }
}

fn same_shape(a: &MindNode, b: &MindNode) -> bool {
  a.children.len() == b.children.len()
    && a.children.iter().zip(&b.children).all(|(x, y)| same_shape(x, y))
}

/// 模型偶尔写成 `§3.1` 或带空格。
pub fn clean_section(s: &str) -> &str {
  s.trim().trim_start_matches('§').trim()
}

/// 数字比对前的规范化：原文里的数字常在 LaTeX 公式里（`$49\times$`、`$84.6\%$`），
/// 模型抄出来是 `49×`、`84.6%`；区间的连字符也有好几种写法。
pub fn normalize_number(s: &str) -> String {
  let mut t = s.to_string();
  for (from, to) in [
    ("\\times", "×"),
    ("\\%", "%"),
    ("\\pm", "±"),
    ("\\sim", "~"),
    ("\\approx", "≈"),
    ("\\,", ""),
    ("\\!", ""),
    ("−", "-"),
    ("–", "-"),
    ("—", "-"),
  ] {
    t = t.replace(from, to);
  }
  t.chars().filter(|c| !c.is_whitespace() && !matches!(c, '$' | ',' | '{' | '}')).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::digest::{KeyPoint, ResultItem};

  fn leaf(l: &str) -> MindNode {
    MindNode { label: l.into(), children: vec![] }
  }

  fn digest() -> PaperDigest {
    PaperDigest {
      tldr: "t".into(),
      problem: "p".into(),
      method: "m".into(),
      results: vec![ResultItem {
        claim: "c".into(),
        numbers: vec!["49×".into(), "82.8–84.6%".into(), "0.6439".into()],
        section: "§4.2".into(),
      }],
      limitations: vec!["l".into()],
      key_points: vec![
        KeyPoint { text: "k".into(), section: "3".into() },
        KeyPoint { text: "k".into(), section: "Limitations".into() },
        KeyPoint { text: "k".into(), section: "Abstract".into() },
      ],
      mindmap: MindNode { label: "r".into(), children: vec![leaf("a"), leaf("b")] },
    }
  }

  fn sections() -> Vec<String> {
    vec!["3".into(), "4.2".into(), "Limitations".into()]
  }

  const SOURCE: &str =
    "[§4.2] Results\nIt is $49\\times$ faster, scoring $82.8$ - $84.6\\%$, from 0.6439 to 0.6354.";

  #[test]
  fn good_digest_passes_and_latex_numbers_verify() {
    assert_eq!(check_digest(&digest(), &sections(), SOURCE), Ok(Checked::default()));
  }

  #[test]
  fn invented_number_is_marked_unverified_not_rejected() {
    let mut d = digest();
    d.results[0].numbers.push("97.7".into());
    let checked = check_digest(&d, &sections(), SOURCE).unwrap();
    assert_eq!(checked.unverified_numbers, ["97.7"]);
  }

  #[test]
  fn missing_section_is_rejected() {
    let mut d = digest();
    d.key_points[0].section = "9.9".into();
    let err = check_digest(&d, &sections(), SOURCE).unwrap_err();
    assert!(err[0].contains("\"9.9\""), "{err:?}");
  }

  #[test]
  fn deep_or_wide_mindmap_is_rejected() {
    let mut d = digest();
    let mut deep = leaf("5");
    for l in ["4", "3", "2", "1"] {
      deep = MindNode { label: l.into(), children: vec![deep] };
    }
    d.mindmap.children.push(deep);
    assert!(check_digest(&d, &sections(), SOURCE).unwrap_err()[0].contains("levels"));

    let mut d = digest();
    d.mindmap.children = (0..7).map(|i| leaf(&i.to_string())).collect();
    assert!(check_digest(&d, &sections(), SOURCE).unwrap_err()[0].contains("children"));
  }

  #[test]
  fn translation_must_keep_structure() {
    let en = digest();
    let mut zh = en.clone();
    zh.tldr = "一句话".into();
    zh.results[0].claim = "结论".into();
    assert_eq!(check_translation(&en, &zh), Ok(()));

    let mut bad = zh.clone();
    bad.results[0].numbers[0] = "49 倍".into();
    bad.mindmap.children.pop();
    assert_eq!(check_translation(&en, &bad).unwrap_err().len(), 2);
  }
}
