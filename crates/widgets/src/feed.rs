//! Sitemap / Atom feed / robots.txt 构建器（Phase 2.4）。
//!
//! 把纯字符串组装放在 widgets crate 里方便单测。HTTP 路由由
//! `crates/app/src/main.rs` 的 Axum 自定义路由绑定到这些函数上。
//!
//! ## 设计要点
//! 1. **URL 转义**：内容页的 slug 可能含 `&`、空格等，按 XML 1.0
//!    最小集合转义（`&`/`<`/`>`/`'`/`"`)。
//! 2. **失败容错**：缺失日期 / 描述 → 跳过对应字段（`<lastmod>` /
//!    `<description>`），不输出空标签。
//! 3. **不依赖 dioxus**：此模块仅用 std 字符串拼接，零运行时开销。
//! 4. **base_url 归一化**：尾随 `/` 自动剔除；与 [`crate::seo::build_canonical`]
//!    保持一致。

use serde::{Deserialize, Serialize};

/// sitemap / atom feed 共用的内容条目。
///
/// 由 server fn 把 `BlogPostSummary` / `DocSummary` / 其它列表数据
/// 映射到本结构后批量入参。`url` 必须是 `path` 形式（`/blog/foo`），
/// 由 builder 拼接 `base_url`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentEntry {
    pub url_path: String,
    pub title: String,
    pub description: String,
    /// ISO 8601 日期（如 `2026-01-15`）；空字符串表示未提供。
    pub date: String,
    pub tags: Vec<String>,
}

/// XML 1.0 最小转义。仅替换 5 个保留字符。
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn join_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{}{}", base, path)
    } else {
        format!("{}/{}", base, path)
    }
}

/// 构造 `sitemap.xml` 字符串。
///
/// - `entries`：所有要收录的内容页（任意模块）。
/// - `base_url`：站点根 URL（含 scheme）。
/// - `static_paths`：站点级固定路径（如 `/`、`/blog`、`/podcast`）。
///   每条以斜杠开头。
pub fn build_sitemap_xml(
    entries: &[ContentEntry],
    static_paths: &[&str],
    base_url: &str,
) -> String {
    let mut out = String::with_capacity(256 + 128 * entries.len());
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    // 站点级固定路径，无 lastmod。
    for p in static_paths {
        out.push_str("  <url>\n");
        out.push_str(&format!("    <loc>{}</loc>\n", xml_escape(&join_url(base_url, p))));
        out.push_str("  </url>\n");
    }

    for e in entries {
        out.push_str("  <url>\n");
        out.push_str(&format!(
            "    <loc>{}</loc>\n",
            xml_escape(&join_url(base_url, &e.url_path))
        ));
        if !e.date.is_empty() {
            out.push_str(&format!(
                "    <lastmod>{}</lastmod>\n",
                xml_escape(&e.date)
            ));
        }
        out.push_str("  </url>\n");
    }

    out.push_str("</urlset>\n");
    out
}

/// 构造 Atom feed XML 字符串。
///
/// - `entries`：博客文章列表（按时间倒序，最近 50 篇由调用方裁剪）。
/// - `site_title` / `site_description`：站点级标题与描述。
/// - `base_url`：站点根 URL。
pub fn build_atom_feed(
    entries: &[ContentEntry],
    site_title: &str,
    site_description: &str,
    base_url: &str,
) -> String {
    let mut out = String::with_capacity(512 + 256 * entries.len());
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");

    out.push_str(&format!("  <title>{}</title>\n", xml_escape(site_title)));
    if !site_description.is_empty() {
        out.push_str(&format!(
            "  <subtitle>{}</subtitle>\n",
            xml_escape(site_description)
        ));
    }
    out.push_str(&format!(
        "  <link href=\"{}\"/>\n",
        xml_escape(base_url.trim_end_matches('/'))
    ));
    out.push_str(&format!(
        "  <link rel=\"self\" href=\"{}\"/>\n",
        xml_escape(&join_url(base_url, "/feed.xml"))
    ));
    // feed 级 id：使用 base_url；若想更稳建议显式传入站点 UUID。
    out.push_str(&format!(
        "  <id>{}</id>\n",
        xml_escape(base_url.trim_end_matches('/'))
    ));

    // 取第一条 entry 的 date 作为 feed 最新更新；若全空回退到 1970-01-01。
    let updated = entries
        .iter()
        .find_map(|e| if e.date.is_empty() { None } else { Some(e.date.clone()) })
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    out.push_str(&format!("  <updated>{}</updated>\n", xml_escape(&updated)));

    for e in entries {
        let url = join_url(base_url, &e.url_path);
        out.push_str("  <entry>\n");
        out.push_str(&format!("    <title>{}</title>\n", xml_escape(&e.title)));
        out.push_str(&format!("    <link href=\"{}\"/>\n", xml_escape(&url)));
        out.push_str(&format!("    <id>{}</id>\n", xml_escape(&url)));
        if !e.date.is_empty() {
            out.push_str(&format!(
                "    <updated>{}</updated>\n",
                xml_escape(&e.date)
            ));
        }
        if !e.description.is_empty() {
            out.push_str(&format!(
                "    <summary>{}</summary>\n",
                xml_escape(&e.description)
            ));
        }
        for tag in &e.tags {
            out.push_str(&format!(
                "    <category term=\"{}\"/>\n",
                xml_escape(tag)
            ));
        }
        out.push_str("  </entry>\n");
    }

    out.push_str("</feed>\n");
    out
}

/// 构造 `robots.txt` 文本：允许所有爬虫，sitemap 链接到当前 base_url。
pub fn build_robots_txt(base_url: &str) -> String {
    let sitemap = join_url(base_url, "/sitemap.xml");
    format!("User-agent: *\nAllow: /\n\nSitemap: {}\n", sitemap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url_path: &str, title: &str, date: &str) -> ContentEntry {
        ContentEntry {
            url_path: url_path.to_string(),
            title: title.to_string(),
            description: format!("desc of {}", title),
            date: date.to_string(),
            tags: vec!["rust".to_string()],
        }
    }

    #[test]
    fn xml_escape_handles_all_reserved_chars() {
        assert_eq!(
            xml_escape("a & b < c > d \" e ' f"),
            "a &amp; b &lt; c &gt; d &quot; e &apos; f"
        );
    }

    #[test]
    fn join_url_handles_trailing_slash_in_base() {
        assert_eq!(join_url("https://x.com/", "/foo"), "https://x.com/foo");
        assert_eq!(join_url("https://x.com", "foo"), "https://x.com/foo");
    }

    #[test]
    fn sitemap_xml_basic_shape() {
        let entries = vec![
            entry("/blog/welcome", "Welcome", "2026-01-15"),
            entry("/blog/foo", "Foo", ""), // 缺失 date 跳过 lastmod
        ];
        let xml = build_sitemap_xml(&entries, &["/", "/blog"], "https://example.com");
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
        // 静态路径：`/` 拼接后是 `https://example.com/`（保留尾斜杠）
        assert!(xml.contains("<loc>https://example.com/</loc>"));
        assert!(xml.contains("<loc>https://example.com/blog</loc>"));
        // 内容路径
        assert!(xml.contains("<loc>https://example.com/blog/welcome</loc>"));
        assert!(xml.contains("<lastmod>2026-01-15</lastmod>"));
        // 缺失 date 不输出 lastmod
        assert!(xml.contains("<loc>https://example.com/blog/foo</loc>"));
        // 但只能有一处 lastmod（welcome 的）
        assert_eq!(xml.matches("<lastmod>").count(), 1);
        assert!(xml.ends_with("</urlset>\n"));
    }

    #[test]
    fn sitemap_xml_escapes_special_chars_in_url_and_title() {
        let entries = vec![entry("/blog/a&b", "Hello & World", "2026-01-15")];
        let xml = build_sitemap_xml(&entries, &[], "https://example.com");
        assert!(xml.contains("/blog/a&amp;b"));
        // title 不进 sitemap，所以不验证
    }

    #[test]
    fn atom_feed_basic_shape() {
        let entries = vec![
            entry("/blog/a", "Post A", "2026-02-15"),
            entry("/blog/b", "Post B", "2026-01-10"),
        ];
        let xml = build_atom_feed(
            &entries,
            "Rust in Everything",
            "Rust ecosystem",
            "https://example.com/",
        );
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\">"));
        assert!(xml.contains("<title>Rust in Everything</title>"));
        assert!(xml.contains("<subtitle>Rust ecosystem</subtitle>"));
        // self link to feed.xml
        assert!(xml.contains("href=\"https://example.com/feed.xml\""));
        // entries
        assert!(xml.contains("<title>Post A</title>"));
        assert!(xml.contains("<title>Post B</title>"));
        // updated 取最新（首条）
        assert!(xml.contains("<updated>2026-02-15</updated>"));
        // category from tags
        assert!(xml.contains("<category term=\"rust\"/>"));
        assert!(xml.ends_with("</feed>\n"));
    }

    #[test]
    fn atom_feed_skips_empty_optional_fields() {
        let mut e = entry("/blog/x", "X", "");
        e.description = String::new();
        e.tags.clear();
        let xml = build_atom_feed(&[e], "Site", "", "https://example.com");
        // 缺日期 / 描述 / category，全都不输出
        assert!(!xml.contains("<updated></updated>"));
        assert!(!xml.contains("<summary></summary>"));
        assert!(!xml.contains("<category"));
        // 但 entry 仍存在
        assert!(xml.contains("<title>X</title>"));
        // 缺 site_description 不输出 subtitle
        assert!(!xml.contains("<subtitle>"));
        // 全空时 feed 级 updated 走 1970 兜底
        assert!(xml.contains("<updated>1970-01-01T00:00:00Z</updated>"));
    }

    #[test]
    fn robots_txt_includes_sitemap_url() {
        assert_eq!(
            build_robots_txt("https://example.com"),
            "User-agent: *\nAllow: /\n\nSitemap: https://example.com/sitemap.xml\n"
        );
        // 末尾斜杠也能正确处理
        assert_eq!(
            build_robots_txt("https://example.com/"),
            "User-agent: *\nAllow: /\n\nSitemap: https://example.com/sitemap.xml\n"
        );
    }
}
