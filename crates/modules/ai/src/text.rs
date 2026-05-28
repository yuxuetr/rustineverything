//! 板块元数据 + 纯逻辑（无 IO / 无 dioxus），可独立单测。

pub const BOARD_ID: &str = "ai";
pub const BOARD_LABEL: &str = "AI";
pub const BOARD_ROUTE: &str = "/ai";
pub const BOARD_TAGLINE: &str =
  "用 Rust 做张量计算、模型推理与 LLM 应用：candle、burn 与 ONNX 生态。";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subtopic {
  pub slug: &'static str,
  pub label: &'static str,
  pub blurb: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeaturedCrate {
  pub name: &'static str,
  pub blurb: &'static str,
  pub url: &'static str,
}

pub const SUBTOPICS: &[Subtopic] = &[
  Subtopic {
    slug: "tensors",
    label: "张量计算",
    blurb: "在 CPU / CUDA / Metal 上做张量运算与自动微分。",
  },
  Subtopic {
    slug: "inference",
    label: "推理引擎",
    blurb: "加载预训练权重做前向推理，部署到服务端或边缘。",
  },
  Subtopic {
    slug: "llm", label: "大模型", blurb: "本地跑 LLM、量化、KV cache 与流式生成。"
  },
  Subtopic {
    slug: "tokenizers",
    label: "分词",
    blurb: "BPE / WordPiece 分词与 HuggingFace tokenizers。",
  },
  Subtopic {
    slug: "training",
    label: "训练框架",
    blurb: "用纯 Rust 框架定义网络、反向传播与优化器。",
  },
  Subtopic {
    slug: "embeddings",
    label: "向量与检索",
    blurb: "句向量、相似度检索与向量数据库集成。",
  },
];

pub const FEATURED_CRATES: &[FeaturedCrate] = &[
  FeaturedCrate {
    name: "candle",
    blurb: "HuggingFace 极简张量与推理框架",
    url: "https://github.com/huggingface/candle",
  },
  FeaturedCrate {
    name: "burn", blurb: "纯 Rust、多后端深度学习框架", url: "https://burn.dev"
  },
  FeaturedCrate {
    name: "tch",
    blurb: "libtorch（PyTorch C++）绑定",
    url: "https://github.com/LaurentMazare/tch-rs",
  },
  FeaturedCrate {
    name: "tokenizers",
    blurb: "HuggingFace 高性能分词器",
    url: "https://github.com/huggingface/tokenizers",
  },
  FeaturedCrate { name: "ort", blurb: "ONNX Runtime 的 Rust 绑定", url: "https://ort.pyke.io" },
  FeaturedCrate {
    name: "safetensors",
    blurb: "安全、零拷贝的张量序列化格式",
    url: "https://github.com/huggingface/safetensors",
  },
];

pub trait DatedArticle {
  fn date(&self) -> &str;
  fn title(&self) -> &str;
}

pub fn normalize_tag(raw: &str) -> String {
  raw
    .trim()
    .to_ascii_lowercase()
    .chars()
    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
    .collect()
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
  let mut out: Vec<String> =
    tags.iter().map(|t| normalize_tag(t)).filter(|t| !t.is_empty()).collect();
  out.sort();
  out.dedup();
  out
}

pub fn subtopic_label(slug: &str) -> Option<&'static str> {
  SUBTOPICS.iter().find(|s| s.slug == slug).map(|s| s.label)
}

pub fn matches_query(title: &str, description: &str, tags: &[String], query: &str) -> bool {
  let q = query.trim().to_lowercase();
  if q.is_empty() {
    return true;
  }
  title.to_lowercase().contains(&q)
    || description.to_lowercase().contains(&q)
    || tags.iter().any(|t| t.to_lowercase().contains(&q))
}

pub fn sort_by_date_desc<T: DatedArticle>(items: &mut [T]) {
  items.sort_by(|a, b| {
    b.date().cmp(a.date()).then_with(|| a.title().to_lowercase().cmp(&b.title().to_lowercase()))
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  struct A {
    date: String,
    title: String,
  }
  impl DatedArticle for A {
    fn date(&self) -> &str {
      &self.date
    }
    fn title(&self) -> &str {
      &self.title
    }
  }

  #[test]
  fn board_constants_well_formed() {
    assert_eq!(BOARD_ID, "ai");
    assert!(BOARD_ROUTE.starts_with('/'));
    assert!(!BOARD_LABEL.is_empty());
    assert!(!BOARD_TAGLINE.is_empty());
  }

  #[test]
  fn subtopics_have_unique_slugs() {
    let mut slugs: Vec<&str> = SUBTOPICS.iter().map(|s| s.slug).collect();
    let n = slugs.len();
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), n, "子主题 slug 应唯一");
  }

  #[test]
  fn subtopics_are_non_empty() {
    assert!(SUBTOPICS.len() >= 4);
    for s in SUBTOPICS {
      assert!(!s.slug.is_empty());
      assert!(!s.label.is_empty());
      assert!(!s.blurb.is_empty());
    }
  }

  #[test]
  fn subtopic_slugs_are_url_safe() {
    for s in SUBTOPICS {
      assert_eq!(normalize_tag(s.slug), s.slug, "slug 应已是归一化形态");
    }
  }

  #[test]
  fn featured_crates_use_https() {
    assert!(FEATURED_CRATES.len() >= 4);
    for c in FEATURED_CRATES {
      assert!(c.url.starts_with("https://"), "{} 应为 https URL", c.name);
      assert!(!c.name.is_empty());
      assert!(!c.blurb.is_empty());
    }
  }

  #[test]
  fn featured_crates_unique_names() {
    let mut names: Vec<&str> = FEATURED_CRATES.iter().map(|c| c.name).collect();
    let n = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), n);
  }

  #[test]
  fn normalize_tag_lowercases_and_strips() {
    assert_eq!(normalize_tag(" Candle!! "), "candle");
    assert_eq!(normalize_tag("safe-tensors"), "safe-tensors");
  }

  #[test]
  fn normalize_tags_dedups_and_drops_empty() {
    let tags =
      vec!["Candle".to_string(), "candle".to_string(), "   ".to_string(), "Burn".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["burn".to_string(), "candle".to_string()]);
  }

  #[test]
  fn subtopic_label_known_and_unknown() {
    assert_eq!(subtopic_label("llm"), Some("大模型"));
    assert_eq!(subtopic_label("does-not-exist"), None);
  }

  #[test]
  fn matches_query_empty_returns_true() {
    assert!(matches_query("t", "d", &[], ""));
    assert!(matches_query("t", "d", &[], "   "));
  }

  #[test]
  fn matches_query_hits_title_description_tags() {
    let tags = vec!["candle".to_string(), "llm".to_string()];
    assert!(matches_query("Candle 推理", "本地大模型", &tags, "candle"));
    assert!(matches_query("Candle 推理", "本地大模型", &tags, "大模型"));
    assert!(matches_query("Candle 推理", "本地大模型", &tags, "llm"));
    assert!(!matches_query("Candle 推理", "本地大模型", &tags, "solana"));
  }

  #[test]
  fn matches_query_supports_chinese() {
    let tags = vec!["inference".to_string()];
    assert!(matches_query("张量入门", "candle 张量", &tags, "张量"));
  }

  #[test]
  fn sort_by_date_desc_orders_newest_first_then_title() {
    let mut items = vec![
      A { date: "2026-01-01".into(), title: "beta".into() },
      A { date: "2026-03-01".into(), title: "zeta".into() },
      A { date: "2026-01-01".into(), title: "alpha".into() },
    ];
    sort_by_date_desc(&mut items);
    let order: Vec<&str> = items.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(order, vec!["zeta", "alpha", "beta"]);
  }
}
