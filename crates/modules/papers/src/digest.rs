//! 一篇论文的结构化解读（`paper_summaries.digest` 里存的 JSON）与思维导图源码。
//!
//! 模型只输出这份结构；思维导图的 mermaid 源码由 [`mindmap_source`] 生成并转义，
//! 所以语法一定合法，不用在服务端跑 mermaid 校验。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperDigest {
  pub tldr: String,
  pub problem: String,
  pub method: String,
  pub results: Vec<ResultItem>,
  pub limitations: Vec<String>,
  pub key_points: Vec<KeyPoint>,
  pub mindmap: MindNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultItem {
  pub claim: String,
  /// 从原文逐字抄下来的数字，例如 `73.4`、`2.1×`、`15%`
  #[serde(default)]
  pub numbers: Vec<String>,
  /// 出处节号，例如 `4.2`、`A.1`，没有编号的节用标题
  pub section: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyPoint {
  pub text: String,
  pub section: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MindNode {
  pub label: String,
  #[serde(default)]
  pub children: Vec<MindNode>,
}

impl MindNode {
  /// 根以下的层数：只有根是 0。
  pub fn depth(&self) -> usize {
    self.children.iter().map(|c| 1 + c.depth()).max().unwrap_or(0)
  }

  /// 任一节点的最多子节点数。
  pub fn widest(&self) -> usize {
    self.children.iter().map(MindNode::widest).fold(self.children.len(), usize::max)
  }
}

/// 生成 mermaid `mindmap` 源码。标签里会被 mermaid 当成语法的字符换成空格，
/// 每个节点都写成 `["..."]` 形状，避免标签被当成关键字或节点 id。
pub fn mindmap_source(root: &MindNode) -> String {
  fn label(s: &str) -> String {
    let cleaned: String = s
      .chars()
      .map(|c| {
        if matches!(c, '"' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '`') {
          ' '
        } else {
          c
        }
      })
      .collect();
    let squashed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if squashed.is_empty() {
      "-".to_string()
    } else {
      squashed
    }
  }
  fn walk(node: &MindNode, level: usize, out: &mut String) {
    out.push('\n');
    out.push_str(&"  ".repeat(level));
    if level == 1 {
      out.push_str("root");
    }
    out.push_str(&format!("[\"{}\"]", label(&node.label)));
    for child in &node.children {
      walk(child, level + 1, out);
    }
  }
  let mut out = String::from("mindmap");
  walk(root, 1, &mut out);
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn node(label: &str, children: Vec<MindNode>) -> MindNode {
    MindNode { label: label.into(), children }
  }

  #[test]
  fn depth_and_width() {
    let leaf = node("c", vec![]);
    let tree =
      node("r", vec![node("a", vec![leaf.clone(), leaf.clone(), leaf]), node("b", vec![])]);
    assert_eq!(tree.depth(), 2);
    assert_eq!(tree.widest(), 3);
    assert_eq!(node("r", vec![]).depth(), 0);
  }

  #[test]
  fn mindmap_escapes_syntax_characters() {
    let tree = node("Root (v2)", vec![node("say \"hi\" [x] {y} <z> `q`", vec![node("", vec![])])]);
    assert_eq!(
      mindmap_source(&tree),
      "mindmap\n  root[\"Root v2\"]\n    [\"say hi x y z q\"]\n      [\"-\"]"
    );
  }
}
