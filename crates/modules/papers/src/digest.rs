//! 一篇论文的结构化解读（`paper_summaries.digest` 里存的 JSON）。
//!
//! 思维导图只存 [`MindNode`] 树，页面直接渲染成 HTML（见 `pages.rs` 的 `mind_node`）。

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

/// 出处节号：模型偶尔写成 `§3.1` 或带空格。
pub fn clean_section(s: &str) -> &str {
  s.trim().trim_start_matches('§').trim()
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
}
