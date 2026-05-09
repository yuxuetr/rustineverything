//! SEO 注入（Phase 2.3）。
//!
//! 给定 [`crate::PostMetadata`] 和当前页面 URL，在 `<head>` 注入：
//! - `<title>`
//! - `<meta name="description">` / `<meta name="keywords">`
//! - Open Graph: `og:title` / `og:description` / `og:image` / `og:url` / `og:type`
//! - Twitter Card: `twitter:card` / `twitter:title` / `twitter:description` / `twitter:image`
//! - `<link rel="canonical">`
//! - `<script type="application/ld+json">` Article schema
//!
//! 设计要点：
//! 1. **不注入空 meta**：`description / keywords / image / author` 缺失时
//!    完全不输出对应 `<meta>`，避免 Lighthouse 检测到空 description。
//! 2. **canonical 自动派生**：未提供时使用 `base_url + path` 拼接；提供时
//!    保留原值（用于跨域同步内容）。
//! 3. **JSON-LD 容错**：序列化失败时退化为不输出 schema 节点，不 panic。
//! 4. **无副作用**：函数返回 `Element`，调用方决定挂在哪一棵 vdom 树上。
//!
//! ## 用法
//! ```ignore
//! use rustineverything_widgets::{parse_mdx, inject_seo};
//! let (meta, _body) = parse_mdx(&content);
//! rsx! {
//!     {inject_seo(&meta, "/blog/welcome", "https://example.com")}
//!     // ... rest of page
//! }
//! ```
//!
//! `base_url` 必填且应为站点公开访问根（含 scheme），通常从 `BASE_URL`
//! 环境变量读出。

use dioxus::prelude::*;

use crate::mdx::PostMetadata;

/// 注入 SEO 元信息到当前页面 `<head>`。
///
/// - `meta`：MDX frontmatter 解析结果。
/// - `path`：当前页面路径（如 `/blog/welcome`）。允许带 leading slash 与否；
///   会自动归一化。
/// - `base_url`：站点根 URL（必须以 `http://` 或 `https://` 开头）。
pub fn inject_seo(meta: &PostMetadata, path: &str, base_url: &str) -> Element {
    let canonical = build_canonical(meta.canonical.as_deref(), path, base_url);
    let json_ld = build_json_ld(meta, &canonical);

    rsx! {
        document::Title { "{meta.title}" }
        if let Some(desc) = meta.description.as_ref() {
            if !desc.is_empty() {
                document::Meta { name: "description", content: "{desc}" }
            }
        }
        if let Some(kw) = meta.keywords.as_ref() {
            if !kw.is_empty() {
                document::Meta { name: "keywords", content: "{kw}" }
            }
        }

        // ── Open Graph ──
        document::Meta { property: "og:title", content: "{meta.title}" }
        document::Meta { property: "og:type", content: "article" }
        document::Meta { property: "og:url", content: "{canonical}" }
        if let Some(desc) = meta.description.as_ref() {
            if !desc.is_empty() {
                document::Meta { property: "og:description", content: "{desc}" }
            }
        }
        if let Some(img) = meta.image.as_ref() {
            if !img.is_empty() {
                document::Meta { property: "og:image", content: "{img}" }
            }
        }

        // ── Twitter Card ──
        document::Meta { name: "twitter:card", content: if meta.image.as_deref().map(|s| !s.is_empty()).unwrap_or(false) { "summary_large_image" } else { "summary" } }
        document::Meta { name: "twitter:title", content: "{meta.title}" }
        if let Some(desc) = meta.description.as_ref() {
            if !desc.is_empty() {
                document::Meta { name: "twitter:description", content: "{desc}" }
            }
        }
        if let Some(img) = meta.image.as_ref() {
            if !img.is_empty() {
                document::Meta { name: "twitter:image", content: "{img}" }
            }
        }

        // ── Canonical link ──
        document::Link { rel: "canonical", href: "{canonical}" }

        // ── JSON-LD article schema ──
        if let Some(ld) = json_ld {
            document::Script { r#type: "application/ld+json", "{ld}" }
        }
    }
}

/// 构造 canonical URL：如果 `meta.canonical` 已显式提供则原样返回；否则
/// 以 `base_url + path` 拼接。`base_url` 末尾的 `/` 会被去掉，`path`
/// 前的 `/` 会被补上，避免 `https://x.com//foo` 双斜杠。
pub fn build_canonical(explicit: Option<&str>, path: &str, base_url: &str) -> String {
    if let Some(c) = explicit {
        if !c.is_empty() {
            return c.to_string();
        }
    }
    let base = base_url.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{}{}", base, path)
    } else {
        format!("{}/{}", base, path)
    }
}

/// 构造 JSON-LD Article schema。`title` 必填；其余字段缺失时跳过。
/// 序列化失败返回 `None`，调用方负责降级（不输出 `<script>` 节点）。
fn build_json_ld(meta: &PostMetadata, canonical: &str) -> Option<String> {
    let mut value = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": meta.title,
        "url": canonical,
    });

    let obj = value.as_object_mut()?;
    if let Some(desc) = meta.description.as_ref() {
        if !desc.is_empty() {
            obj.insert("description".to_string(), serde_json::Value::String(desc.clone()));
        }
    }
    if let Some(img) = meta.image.as_ref() {
        if !img.is_empty() {
            obj.insert("image".to_string(), serde_json::Value::String(img.clone()));
        }
    }
    if let Some(date) = meta.date.as_ref() {
        if !date.is_empty() {
            obj.insert("datePublished".to_string(), serde_json::Value::String(date.clone()));
        }
    }
    if let Some(author) = meta.author.as_ref() {
        if !author.is_empty() {
            obj.insert(
                "author".to_string(),
                serde_json::json!({
                    "@type": "Person",
                    "name": author,
                }),
            );
        }
    }
    if !meta.tags.is_empty() {
        obj.insert(
            "keywords".to_string(),
            serde_json::Value::String(meta.tags.join(", ")),
        );
    }

    serde_json::to_string(&value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_minimal(title: &str) -> PostMetadata {
        PostMetadata {
            title: title.to_string(),
            ..Default::default()
        }
    }

    fn meta_full() -> PostMetadata {
        PostMetadata {
            title: "Hello, Rust".into(),
            description: Some("A welcome post".into()),
            keywords: Some("rust,welcome".into()),
            image: Some("https://cdn.example.com/cover.png".into()),
            author: Some("Hal".into()),
            canonical: None,
            date: Some("2026-01-15".into()),
            tags: vec!["rust".into(), "welcome".into()],
        }
    }

    #[test]
    fn build_canonical_appends_path() {
        let url = build_canonical(None, "/blog/foo", "https://example.com");
        assert_eq!(url, "https://example.com/blog/foo");
    }

    #[test]
    fn build_canonical_strips_trailing_slash_in_base() {
        let url = build_canonical(None, "/blog/foo", "https://example.com/");
        assert_eq!(url, "https://example.com/blog/foo");
    }

    #[test]
    fn build_canonical_inserts_leading_slash_when_path_missing_one() {
        let url = build_canonical(None, "blog/foo", "https://example.com");
        assert_eq!(url, "https://example.com/blog/foo");
    }

    #[test]
    fn build_canonical_uses_explicit_override_when_present() {
        let url = build_canonical(
            Some("https://other.com/canonical"),
            "/blog/foo",
            "https://example.com",
        );
        assert_eq!(url, "https://other.com/canonical");
    }

    #[test]
    fn build_canonical_ignores_empty_explicit_string() {
        // 空字符串视为未提供
        let url = build_canonical(Some(""), "/blog/foo", "https://example.com");
        assert_eq!(url, "https://example.com/blog/foo");
    }

    #[test]
    fn json_ld_minimal_contains_required_fields() {
        let meta = meta_minimal("Hi");
        let ld = build_json_ld(&meta, "https://example.com/p").expect("json-ld");
        assert!(ld.contains("\"@type\":\"Article\""));
        assert!(ld.contains("\"headline\":\"Hi\""));
        assert!(ld.contains("\"url\":\"https://example.com/p\""));
    }

    #[test]
    fn json_ld_full_includes_all_optional_fields() {
        let meta = meta_full();
        let ld = build_json_ld(&meta, "https://example.com/blog/welcome").expect("json-ld");
        assert!(ld.contains("\"description\":\"A welcome post\""));
        assert!(ld.contains("\"image\":\"https://cdn.example.com/cover.png\""));
        assert!(ld.contains("\"datePublished\":\"2026-01-15\""));
        assert!(ld.contains("\"name\":\"Hal\""));
        assert!(ld.contains("\"keywords\":\"rust, welcome\""));
    }

    #[test]
    fn json_ld_skips_missing_optional_fields() {
        let mut meta = meta_full();
        meta.description = None;
        meta.image = None;
        meta.date = None;
        meta.author = None;
        meta.tags.clear();
        let ld = build_json_ld(&meta, "https://example.com/p").expect("json-ld");
        // 只剩 @context / @type / headline / url
        assert!(!ld.contains("\"description\""));
        assert!(!ld.contains("\"image\""));
        assert!(!ld.contains("\"datePublished\""));
        assert!(!ld.contains("\"author\""));
        assert!(!ld.contains("\"keywords\""));
    }

    #[test]
    fn json_ld_skips_empty_string_optional_fields() {
        // 空字符串不应导致输出空键
        let mut meta = meta_full();
        meta.description = Some(String::new());
        meta.image = Some(String::new());
        meta.author = Some(String::new());
        meta.date = Some(String::new());
        let ld = build_json_ld(&meta, "https://example.com/p").expect("json-ld");
        assert!(!ld.contains("\"description\":\"\""));
        assert!(!ld.contains("\"image\":\"\""));
        assert!(!ld.contains("\"author\""));
        assert!(!ld.contains("\"datePublished\":\"\""));
    }

    #[test]
    fn inject_seo_returns_element_for_minimal_meta() {
        // 仅校验函数能返回 Ok(Element)，不试图 SSR 渲染
        let meta = meta_minimal("Hello");
        let el = inject_seo(&meta, "/blog/welcome", "https://example.com");
        assert!(el.is_ok(), "minimal inject_seo should return Ok element");
    }

    #[test]
    fn inject_seo_returns_element_for_full_meta() {
        let meta = meta_full();
        let el = inject_seo(&meta, "/blog/welcome", "https://example.com");
        assert!(el.is_ok());
    }
}
