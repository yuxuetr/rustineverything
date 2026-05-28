//! 板块元数据 + 纯逻辑（无 IO / 无 dioxus），可独立单测。

pub const BOARD_ID: &str = "wasm";
pub const BOARD_LABEL: &str = "WASM";
pub const BOARD_ROUTE: &str = "/wasm";
pub const BOARD_TAGLINE: &str = "WebAssembly 全景：浏览器互操作、WASI、组件模型与服务端运行时。";

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
    slug: "bindgen",
    label: "wasm-bindgen",
    blurb: "Rust 与 JS 互调，把 Rust 编进浏览器。",
  },
  Subtopic {
    slug: "wasi",
    label: "WASI",
    blurb: "WebAssembly 系统接口：文件、时钟、网络的可移植 ABI。",
  },
  Subtopic {
    slug: "components",
    label: "组件模型",
    blurb: "WIT / wit-bindgen 与可组合的 wasm 组件。",
  },
  Subtopic {
    slug: "runtimes",
    label: "运行时",
    blurb: "wasmtime / wasmer 在服务端嵌入 wasm 沙箱。",
  },
  Subtopic {
    slug: "frontend", label: "前端框架", blurb: "Leptos / Yew 用 Rust 写响应式前端。"
  },
  Subtopic {
    slug: "plugins", label: "插件系统", blurb: "用 wasm 做安全、可热更新的插件 ABI。"
  },
];

pub const FEATURED_CRATES: &[FeaturedCrate] = &[
  FeaturedCrate {
    name: "wasm-bindgen",
    blurb: "Rust ↔ JS 互操作绑定生成",
    url: "https://github.com/rustwasm/wasm-bindgen",
  },
  FeaturedCrate {
    name: "wasmtime",
    blurb: "Bytecode Alliance 的 wasm 运行时",
    url: "https://wasmtime.dev",
  },
  FeaturedCrate {
    name: "wasmer", blurb: "通用 wasm 运行时与包管理", url: "https://wasmer.io"
  },
  FeaturedCrate {
    name: "wit-bindgen",
    blurb: "组件模型 WIT 绑定生成",
    url: "https://github.com/bytecodealliance/wit-bindgen",
  },
  FeaturedCrate {
    name: "leptos",
    blurb: "细粒度响应式的 Rust 前端框架",
    url: "https://leptos.dev",
  },
  FeaturedCrate {
    name: "trunk", blurb: "Rust+WASM 前端打包工具", url: "https://trunkrs.dev"
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
    assert_eq!(BOARD_ID, "wasm");
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
    assert_eq!(normalize_tag(" Wasmtime!! "), "wasmtime");
    assert_eq!(normalize_tag("wasm-bindgen"), "wasm-bindgen");
  }

  #[test]
  fn normalize_tags_dedups_and_drops_empty() {
    let tags =
      vec!["Leptos".to_string(), "leptos".to_string(), "   ".to_string(), "WASI".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["leptos".to_string(), "wasi".to_string()]);
  }

  #[test]
  fn subtopic_label_known_and_unknown() {
    assert_eq!(subtopic_label("wasi"), Some("WASI"));
    assert_eq!(subtopic_label("does-not-exist"), None);
  }

  #[test]
  fn matches_query_empty_returns_true() {
    assert!(matches_query("t", "d", &[], ""));
    assert!(matches_query("t", "d", &[], "   "));
  }

  #[test]
  fn matches_query_hits_title_description_tags() {
    let tags = vec!["wasmtime".to_string(), "wasi".to_string()];
    assert!(matches_query("Wasmtime 嵌入", "服务端沙箱", &tags, "wasmtime"));
    assert!(matches_query("Wasmtime 嵌入", "服务端沙箱", &tags, "沙箱"));
    assert!(matches_query("Wasmtime 嵌入", "服务端沙箱", &tags, "wasi"));
    assert!(!matches_query("Wasmtime 嵌入", "服务端沙箱", &tags, "solana"));
  }

  #[test]
  fn matches_query_supports_chinese() {
    let tags = vec!["bindgen".to_string()];
    assert!(matches_query("浏览器互操作", "wasm-bindgen", &tags, "浏览器"));
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
