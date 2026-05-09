# SEO 规范（Phase 2.3 / 2.4）

> 本文描述 widgets crate 提供的 SEO 注入与发现机制：`inject_seo` 元
> 标签、canonical URL、JSON-LD 结构化数据，以及 sitemap.xml /
> feed.xml / robots.txt 三件套。

## 1. 流水线

```
MDX frontmatter
   │
   ▼
parse_mdx → PostMetadata
   │
   ▼
inject_seo(meta, path, base_url) → Element
   │
   ▼
<head>
   ├── <title>
   ├── <meta name=description|keywords>
   ├── <meta property=og:*>
   ├── <meta name=twitter:*>
   ├── <link rel=canonical>
   └── <script type=application/ld+json> Article schema
```

实现位于 `crates/widgets/src/seo.rs`。

## 2. inject_seo

```rust
pub fn inject_seo(
    meta: &PostMetadata,
    path: &str,
    base_url: &str,
) -> Element
```

- `meta`：MDX frontmatter 解析结果（含 image / author / canonical / date / tags 5 个新字段）
- `path`：当前页面相对路径（如 `/blog/welcome`）；leading slash 可有可无
- `base_url`：站点根 URL，含 scheme（如 `https://example.com`）；尾随 `/` 会自动去掉

返回值是单个 `Element`（实际是若干 `document::Title / Meta / Link /
Script` 的组合）。Dioxus runtime 会把它们提升到当前文档的 `<head>`。

### 2.1 注入字段一览

| 字段 | 输出位置 | 触发条件 |
|---|---|---|
| `title` | `<title>` | 始终输出 |
| `description` | `<meta name=description>` | `meta.description` 非空 |
| `keywords` | `<meta name=keywords>` | `meta.keywords` 非空 |
| `og:title` | `<meta property=og:title>` | 始终输出 |
| `og:type` | `<meta property=og:type>` = `article` | 始终输出 |
| `og:url` | `<meta property=og:url>` | 始终输出（canonical） |
| `og:description` | `<meta property=og:description>` | `meta.description` 非空 |
| `og:image` | `<meta property=og:image>` | `meta.image` 非空 |
| `twitter:card` | `<meta name=twitter:card>` | 始终输出（image 存在 → `summary_large_image`，否则 `summary`） |
| `twitter:title` | `<meta name=twitter:title>` | 始终输出 |
| `twitter:description` | `<meta name=twitter:description>` | `meta.description` 非空 |
| `twitter:image` | `<meta name=twitter:image>` | `meta.image` 非空 |
| canonical | `<link rel=canonical href=…>` | 始终输出 |
| JSON-LD | `<script type=application/ld+json>` | 始终输出（缺字段自动跳过） |

**关键约束：缺失或空字符串的可选字段不会输出空标签**——避免被
Lighthouse 识别为「empty meta description」。

### 2.2 canonical URL

由 `build_canonical(explicit, path, base_url)` 决定：

1. 如果 `meta.canonical` 显式提供且非空 → 原样返回
2. 否则用 `base_url.trim_end_matches('/') + path`（自动补 leading slash）

```rust
build_canonical(None, "/blog/foo", "https://example.com")
   == "https://example.com/blog/foo"

build_canonical(None, "/blog/foo", "https://example.com/")
   == "https://example.com/blog/foo"  // 尾随 / 自动剔除

build_canonical(Some("https://other.com/x"), "/p", "https://x.com")
   == "https://other.com/x"  // 显式覆盖
```

### 2.3 JSON-LD Article schema

```json
{
  "@context": "https://schema.org",
  "@type": "Article",
  "headline": meta.title,
  "url": canonical,
  "description": meta.description,         // 非空时
  "image": meta.image,                     // 非空时
  "datePublished": meta.date,              // 非空时
  "author": { "@type": "Person", "name": meta.author },  // 非空时
  "keywords": meta.tags.join(", ")         // 非空时
}
```

序列化失败时返回 `None`，调用方不输出 `<script>` 节点（不 panic）。

## 3. 内容页接入

App crate 的内容页通过 `use_resource` 拉 `BASE_URL`，再调
`inject_seo`：

```rust
let base_url_res = use_resource(|| async move {
    crate::server::get_seo_base_url().await.unwrap_or_default()
});
let base_url = base_url_res.read().as_ref().cloned().unwrap_or_default();

rsx! {
    {
        let (meta, _body) = parse_mdx(&content);
        rsx! { {inject_seo(&meta, "/blog/welcome", &base_url)} }
    }
    Markdown { content, blog_id: "blog:welcome".to_string() }
}
```

`get_seo_base_url` 是个 server fn（在 `crates/app/src/server/mod.rs`），
读 `BASE_URL` 环境变量；未设置时返回空串，`inject_seo` 退化为相对
路径（仍能保证 `<title>` / `<meta>` 正常）。

当前接入页面：

- ✅ `Blog`（`/blog/:id`）— Phase 2.3 落地
- ⏳ `DocPage` / `Lesson` / `CaseDetail` / `TopicDetail` / `PodcastPage`
  — 同模式可直接套用，详见 [`Blog` 实现](../crates/app/src/routes/mod.rs)

## 4. Sitemap.xml

由 `build_sitemap_xml(entries, static_paths, base_url)` 生成
（`crates/widgets/src/feed.rs`）。

```
GET /sitemap.xml
  ↓
list_blog_posts() → BlogPostSummary[]
  ↓ map 为 ContentEntry
build_sitemap_xml(
  entries,
  static_paths = ["/", "/blog", "/podcast", "/course",
                  "/case", "/docs", "/topics"],
  base_url,
)
```

输出符合 [Sitemap Protocol 0.9](https://www.sitemaps.org/protocol.html)：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
  </url>
  <url>
    <loc>https://example.com/blog/welcome</loc>
    <lastmod>2026-01-15</lastmod>
  </url>
  …
</urlset>
```

XML 1.0 转义内置（`xml_escape` 处理 `&`、`<`、`>`、`"`、`'`）。

> 注：当前仅收录博客内容页 + 7 个静态路径。其它模块（doc / lesson /
> case / topic）将在 **Phase 3.4** ModuleEngine 接入路由层后批量补全。

## 5. Atom feed.xml

由 `build_atom_feed(entries, site_title, site_description, base_url)`
生成。读 `site.json` 拿站点元信息，对 `list_blog_posts()` 结果取最近
50 篇（`truncate(50)`）。

输出符合 [Atom 1.0](https://www.rfc-editor.org/rfc/rfc4287)：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Rust in Everything</title>
  <subtitle>Rust ecosystem</subtitle>
  <link href="https://example.com"/>
  <link rel="self" href="https://example.com/feed.xml"/>
  <id>https://example.com</id>
  <updated>2026-02-15</updated>
  <entry>
    <title>Post A</title>
    <link href="https://example.com/blog/post-a"/>
    <id>https://example.com/blog/post-a</id>
    <updated>2026-02-15</updated>
    <summary>desc of Post A</summary>
    <category term="rust"/>
  </entry>
  …
</feed>
```

空字段（description / tags / 全空 entries 时的 feed-level updated）
都按 1970-01-01 兜底，避免输出非法 XML。

## 6. robots.txt

```
User-agent: *
Allow: /

Sitemap: https://example.com/sitemap.xml
```

由 `build_robots_txt(base_url)` 生成。base_url 末尾的 `/` 自动归一。

## 7. 验收门禁

- ✅ `cargo test -p rustineverything-widgets` —— seo + feed 共 18 单测全绿
- ✅ 缺失字段不注入空 meta（`json_ld_skips_missing_optional_fields`）
- ✅ 空字符串字段不注入（`json_ld_skips_empty_string_optional_fields`）
- ✅ canonical 自动派生（`build_canonical_appends_path` 等 5 测）
- ✅ Atom feed 处理空字段（`atom_feed_skips_empty_optional_fields`）
- ⏳ Lighthouse SEO ≥ 95 —— 需要在线部署后实测
- ⏳ google sitemap test / W3C feed validator —— 需要部署后跑

## 8. 测试样例

`cargo test -p rustineverything-widgets seo`（11 测）+
`cargo test -p rustineverything-widgets feed`（7 测）覆盖了所有边角
情况。可以直接以这些测试作为 SEO 行为变化时的回归门禁。
