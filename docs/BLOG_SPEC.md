# Blog 模块 SPEC

> 范围：`crates/modules/blog` —— 文件型 markdown 博客集合，对应站点 `/blog`
> 路由。**没有数据库**，文章源头是 `assets/posts/<slug>/index.{md,mdx}`。

## 1. 设计选择

| 维度 | 选择 | 原因 |
| --- | --- | --- |
| 存储 | 文件系统（仓库内 markdown） | git 即版本控制；可走 PR 工作流编辑；零运行时 DB 依赖 |
| 渲染 | 服务端读 raw → 前端 `widgets::Markdown` 解析 | MDX 组件可在博客中嵌入（Podcast / Mermaid / Math 等，详见 [`MDX_SPEC.md`](MDX_SPEC.md)） |
| 索引 | 按目录扫描，frontmatter 是元数据真相 | 不需要构建步骤，新增文章直接 `mkdir + index.md` 即可 |
| 增量 | Phase 7.3.3 起 Tantivy 索引按 mtime 差分 | 详见 [`SEARCH_SPEC.md`](SEARCH_SPEC.md) §7 |

## 2. 资产布局

```
assets/posts/
├── <slug>/
│   ├── index.md       ── 或 index.mdx（优先 mdx）
│   ├── cover.png      ── 可选封面图
│   └── *.png / *.jpg  ── 文内引用图
└── …
```

`<slug>` 是 URL 路径片段。任何符合 `[A-Za-z0-9-_]+` 的字符均可，但建议小写连字符。

### Frontmatter 字段

`index.md` 顶部 YAML frontmatter：

```yaml
---
title: 文章标题
description: 简短描述，用于卡片 / SEO
date: 2026-05-31
tags: [rust, axum]    # 可选
---
```

未声明 `title` 时退化为：首个 `# H1` → 文件名 stem。

## 3. server fn 契约

```rust
#[server]
pub async fn list_blog_posts() -> Result<Vec<BlogPostSummary>, ServerFnError>;

#[server]
pub async fn get_blog_content(id: String) -> Result<String, ServerFnError>;
```

### `BlogPostSummary`

```rust
pub struct BlogPostSummary {
  pub slug: String,
  pub title: String,
  pub description: String,
  pub date: String,         // YYYY-MM-DD 字符串，便于跨时区一致显示
  pub tags: Vec<String>,
}
```

### `list_blog_posts` 行为

- 扫描 `assets/posts/` 直接子目录。
- 跳过不含 `index.{md,mdx}` 的目录（容忍未完成草稿）。
- 资产目录不存在 → 返回 `Ok(vec![])`，**不** panic。
- 排序：按 frontmatter `date` 字段降序（缺失日期排到最后）。

### `get_blog_content` 行为

- 入参 `id` = slug；拼成 `assets/posts/<slug>/index.{mdx,md}`。
- 返回 raw 文件内容（含 frontmatter）；前端 `widgets::Markdown` 负责拆 frontmatter +
  渲染。
- 文件缺失 → `ServerFnError::new("…")`。

## 4. 路由 / UI

| 路径 | 行为 |
| --- | --- |
| `/blog` | 文章卡片列表（标题 / 描述 / 日期 / 标签 chip） |
| `/blog/:slug` | 单篇详情：`Markdown` 组件渲染，自带 TOC / 代码高亮 / Mermaid |

UI 组件在 `crates/app/src/components/`，本 crate 仅暴露 server fn + 数据结构。

## 5. ModuleEngine 集成

`site.json::modules.blog.enabled = false` 关闭后：

- Navbar 不显示 "Blog" 入口。
- `/blog` 与 `/blog/:slug` 在 ModuleEngine 路由 gate 内返回 404。
- `sitemap.xml` 不收录博客静态路径与文章条目。
- `feed.xml` 不收录文章。
- Tantivy `indexer.rs::collect_blogs_versioned` 不索引（kind="blog" 文档从结果中剔除）。

## 6. 在搜索中的位置

`indexer.rs::collect_blogs_versioned` 读取每篇 `index.md` 的 mtime 作为版本键。
Phase 7.3.3 的 `diff_for_reindex` 仅对 mtime 变更的文章触发 upsert，新增 / 删除文件
自动反映到索引（详见 [`SEARCH_SPEC.md`](SEARCH_SPEC.md) §7 增量索引）。

## 7. SEO

每篇博客的详情页通过 `widgets::seo::inject_seo` 注入：

- `<title>` = frontmatter title
- `<meta name="description">` = frontmatter description
- Open Graph: `og:title` / `og:description` / `og:image`（取 cover.png 若存在）
- `<link rel="canonical">` 来自 `BASE_URL + /blog/<slug>`
- JSON-LD `Article` schema（含 datePublished）

完整字段见 [`SEO_SPEC.md`](SEO_SPEC.md)。

## 8. 测试覆盖

```bash
cargo test --features server -p module-blog
```

当前 `server.rs` **无单测**（文件系统操作；测试通过运行时端到端覆盖：
`cargo test --features server -p module-search` 的 indexer 测试侧面覆盖 `assets/posts`
扫描逻辑；MDX 渲染测试在 `widgets/src/mdx.rs`）。如需 server 单测可参考
`module-search/src/indexer.rs::tests` 的 `tempdir` 写测试 markdown 模式。

## 9. 不在本期范围

- 文章草稿 / 定时发布（frontmatter `draft: true` 已被 `markdown_to_plain` 忽略，
  但发布定时未实现）
- 多语言版本（同一 slug 多个 `index.<lang>.md`）
- RSS 全文 vs 摘要切换（当前 `feed.xml` 输出摘要）
- 评论挂载：博客详情页可挂 `module-comments`，目前由前端组件直接组装而非 blog crate 暴露
