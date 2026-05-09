# MDX 渲染规范（Phase 2 后）

> 本文描述 `crates/widgets` 提供的 MDX 渲染管道：frontmatter 解析、GFM
> 渲染、嵌入组件注册表，以及业务模块如何接入自定义组件。

## 1. 整体架构

```
content (MDX 字符串)
   │
   ▼
parse_mdx(content) → (PostMetadata, body)
   │                              │
   ▼                              ▼
seo::inject_seo(meta, …)      pulldown-cmark Parser
                                   │
                                   ▼
                        render_stream → Vec<Element>
                                   │
                                   ▼
                          rsx! { div.prose { … } }
```

实现位于 `crates/widgets/src/mdx.rs`，由 [`Markdown`] 组件统一暴露。
所有内容页（Blog / DocPage / Lesson / Comments / Cases / Forum）
都通过 `use rustineverything_widgets::Markdown` 引入。

## 2. Frontmatter

使用 `---` 分隔的 YAML 块。`PostMetadata` 现支持以下字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `title` | `String` | 页面标题，`<title>` + JSON-LD `headline` |
| `description` | `Option<String>` | `<meta name=description>` + og/twitter/json-ld |
| `keywords` | `Option<String>` | `<meta name=keywords>`（逗号分隔） |
| `image` | `Option<String>` | og:image / twitter:image / json-ld image |
| `author` | `Option<String>` | json-ld `author.name` |
| `canonical` | `Option<String>` | 显式 canonical URL；不提供时自动派生 |
| `date` | `Option<String>` | ISO 8601 日期；json-ld `datePublished` |
| `tags` | `Vec<String>` | json-ld `keywords`、Atom feed `<category>` |

所有新字段都是 `Option` / `Vec` + `#[serde(default)]`，存量
frontmatter 无需修改即可加载。

示例：

```yaml
---
title: "Hello, Rust"
description: "A welcome post"
image: "/posts/welcome/cover.png"
author: "Hal"
date: "2026-01-15"
tags: [rust, welcome]
---

# Hello

文章正文……
```

## 3. 支持的 Markdown 语法（GFM 全集 + 扩展）

`pulldown-cmark` 启用以下选项（详见 `mdx.rs::Markdown::options`）：

- `ENABLE_TABLES`：标准表格（带 `<thead>`）
- `ENABLE_FOOTNOTES`：脚注 `[^id]`
- `ENABLE_STRIKETHROUGH`：`~~text~~`
- `ENABLE_TASKLISTS`：`- [x] / - [ ]`
- `ENABLE_MATH`：`$inline$` 与 `$$display$$`
- `ENABLE_GFM`：GFM 警告 `> [!NOTE] / [!TIP] / [!IMPORTANT] / [!WARNING] / [!CAUTION]`

额外扩展（在 `convert_admonitions` 中预处理）：

- `:::note / :::tip / :::important / :::warning / :::caution / :::info /
  :::warn / :::danger / :::error / :::success`
  → 转换为对应的 GFM alert 块

代码块支持：

- 语言高亮（PrismJS 在 `assets/js/prism-*.min.js`）
- 代码 Copy 按钮
- ```mermaid 块自动调用 `mermaid.run`

数学：`pulldown-latex` 把 LaTeX 转 MathML，渲染失败回退为
`<code>` 显示原文，绝不 panic。

## 4. 嵌入组件（MDX 标签）

形如 `<Tag attr="value" />` 的标签会在 HTML 流中被识别。识别规则：

- 大写字母开头（避免和原生 HTML 标签冲突）
- 由 `detect_registered_tag` 提取标签名
- 由 `parse_attrs` 解析 `key="value"` / `key='value'` 属性
- 调用 `crate::registry::render(name, attrs)` 查表渲染
- 未注册的标签 → 降级为占位 `span`，保证整篇文章仍可渲染

### 4.1 内置默认组件（widgets 自带 9 个）

由 `register_default_components()` 在 app 启动期注册：

| 标签 | 用途 | 关键属性 |
|---|---|---|
| `<YouTube id="…" />` | 嵌入 YouTube 视频（16:9） | `id` |
| `<Bilibili id="BV…" />` | 嵌入 Bilibili 视频 | `id` |
| `<Yellow text="…" />` | 黄色高亮文字 | `text` |
| `<Green text="…" />` | 绿色高亮文字 | `text` |
| `<Blue text="…" />` | 蓝色高亮文字 | `text` |
| `<Pink text="…" />` | 粉色高亮文字 | `text` |
| `<Purple text="…" />` | 紫色高亮文字 | `text` |
| `<Underline text="…" />` | 下划线文字 | `text` |
| `<Strikethrough text="…" />` | 删除线文字 | `text` |

详细说明见 `docs/components/<Tag>.md`。

### 4.2 业务模块贡献的组件

各模块在自己的 `register_components()` 中注册：

| 标签 | 提供模块 | 用途 |
|---|---|---|
| `<PodcastCard id="…" />` | `crates/modules/podcast` | 嵌入播客卡片 |

模块应在 `crates/app/src/main.rs` 顶部调用其
`register_components()`，与 `register_default_components()` 同期运行。

## 5. 编写新 MDX 组件（≤ 30 行）

以编写一个 `<Tweet id="…" />` 为例：

```rust
use std::collections::HashMap;
use dioxus::prelude::*;
use rustineverything_widgets::{register, MdxComponent};

struct TweetComponent;
impl MdxComponent for TweetComponent {
    fn name(&self) -> &'static str {
        "Tweet"
    }
    fn render(&self, attrs: &HashMap<String, String>) -> Element {
        let id = attrs.get("id").cloned().unwrap_or_default();
        rsx! {
            blockquote { class: "twitter-tweet",
                a { href: "https://twitter.com/i/web/status/{id}" }
            }
        }
    }
}

pub fn register_components() {
    register(Box::new(TweetComponent));
}
```

组件 trait 要求：

- `Send + Sync`（由 trait 上界强制；不存 `Element`，仅按需构造）
- `name()` 返回 `&'static str`，必须与 MDX 标签大小写一致
- `render(attrs)` 在调用线程中构造 Element，不应 panic

完整 trait：

```rust
pub trait MdxComponent: Send + Sync {
    fn name(&self) -> &'static str;
    fn render(&self, attrs: &HashMap<String, String>) -> Element;
}
```

## 6. 顶层块标注 (`data-block-id`)

每个顶层块（`<h1-3>` / `<p>` / `<ul>` / `<ol>` / `<table>` /
`<blockquote>` / 代码块 / mermaid 等）会被注入：

- `id="b<N>"` —— 用于跳转 hash
- `data-block-id="b<N>"` —— 标注系统的稳定锁点

序号从 1 开始连续递增；嵌套子块不分配 block-id（避免锁点抖动）。

## 7. 测试

`cargo test -p rustineverything-widgets` 涵盖：

- `mdx::tests`（13）：parse_mdx / convert_admonitions / detect_registered_tag /
  parse_attrs / extract_attr / latex_to_mathml
- `registry::tests`（6）：register / lookup / list / clear / overwrite
- `components::tests`（3）：default_components_register_all_expected_names /
  idempotent re-register / unknown lookup
- `seo::tests`（11）：build_canonical / build_json_ld / inject_seo
- `feed::tests`（7）：xml_escape / sitemap / atom feed / robots.txt

共 40 个 widgets 单测，全 workspace `cargo test --features server
--workspace` 共 299 passed。
