# Docs 模块 SPEC

> 范围：`crates/modules/docs` —— 站点 `/docs` 路由对应的**应用内**文档系统，
> 类 Docusaurus 的目录树 + 侧栏导航 + 排序控制。
>
> ⚠️ 命名提醒：本 SPEC 描述的是「内容平台**作为站点功能的** Docs 模块」，
> **不**是仓库根的 `docs/` 文档目录（项目工程文档，含本 SPEC 自身）。
> 路径区分：
> - `assets/docs/...`  → Docs 模块的内容资产（本 SPEC 描述）
> - `docs/...`         → 项目工程文档（开发者读、git 跟踪）

## 1. 设计选择

| 维度 | 选择 | 原因 |
| --- | --- | --- |
| 信息架构 | 至多**三级目录** | 平衡侧栏可读性 vs 大型文档库（轻量级 Docusaurus 即可覆盖 90% 站点需求） |
| 元数据 | YAML frontmatter + 可选 `_meta.json` | frontmatter 写在文档内便于编辑；`_meta.json` 用于「显式排序 + 仅做侧栏导航的虚拟节点」 |
| 渲染 | 服务端读 raw → 前端 `widgets::Markdown` | 同 [`BLOG_SPEC.md`](BLOG_SPEC.md)，复用 MDX 渲染管道 |
| 排序 | `sidebar_position` 数值升序，缺省按字母 | 数值类 Docusaurus；缺省 fallback 让"快速塞文档不指定 position"也工作 |

## 2. 资产布局

```
assets/docs/
├── axum/
│   ├── index.md              ── /docs/axum
│   ├── _meta.json            ── 可选：显式排序 / 隐藏节点
│   ├── basic/
│   │   └── index.md          ── /docs/axum/basic（二级）
│   └── handlers/
│       └── index.md          ── /docs/axum/handlers
├── seaorm/
│   └── index.md
└── …
```

- 每级目录通过 `index.md` 提供"该节点的内容"。
- 跳过：以 `_` / `.` 开头的目录（用于「私有 / 草稿 / 工具脚本」）。
- 三级深度：`/docs/<l1>/<l2>/<l3>`；超出深度可读取但不在侧栏树展示。

## 3. 数据结构

### DocMeta（frontmatter）

```rust
pub struct DocMeta {
  pub title: String,
  pub description: String,
  pub keywords: Vec<String>,
  pub sidebar_label: Option<String>,      // 侧栏标签（覆盖 title）
  pub sidebar_position: Option<i32>,      // 排序权重（越小越靠前）
  pub image: Option<String>,              // OG 图
  pub sort_children: Option<String>,      // "asc" | "desc"：子项排序方向
}
```

`sort_children` 设计动机：周报 / 日报场景，子项是 `issue-001`、`issue-002` …
按字母升序得旧期在前；在父目录 `index.md` frontmatter 加 `sort_children: desc`
让最新期排到顶。

### DocTreeNode（侧栏树节点）

```rust
pub struct DocTreeNode {
  pub slug: String,
  pub title: String,
  pub path: String,            // 完整路径，如 "axum/handlers"
  pub has_content: bool,       // 该节点是否有 index.md（false → 纯组织节点）
  pub description: String,
  pub children: Vec<DocTreeNode>,
}
```

### DocContentResponse（详情页响应）

```rust
pub struct DocContentResponse {
  pub content: String,         // markdown 原文（不含 frontmatter）
  pub meta: DocMeta,           // 已解析的 frontmatter
}
```

## 4. server fn 契约

```rust
#[server]
pub async fn list_doc_tree() -> Result<Vec<DocTreeNode>, ServerFnError>;

#[server]
pub async fn get_doc_content(path: String) -> Result<DocContentResponse, ServerFnError>;
```

### `list_doc_tree`

- 递归扫描 `assets/docs/` 至多 3 层。
- 标题优先级：`sidebar_label` > frontmatter `title` > 文档首个 `# H1` > 目录名。
- 排序：先按 `sidebar_position` 升序；无 position 的按字母排在后面（稳定排序）。
- 若同级有 `_meta.json`，按其声明顺序排列 + 可创建无文件的虚拟节点。

### `get_doc_content`

- 入参 `path` = 斜杠分隔，如 `"axum/handlers"`。
- 拼成 `assets/docs/<path>/index.md`（不存在 → `ServerFnError`）。
- 拆 frontmatter → 同时返回 `content`（raw md）+ `meta`（结构化）。

## 5. `_meta.json` 格式（可选）

放在某级目录下，覆盖该目录的子项列表 + 顺序：

```json
{
  "items": [
    { "slug": "intro", "title": "入门" },
    { "slug": "advanced", "title": "进阶" },
    { "slug": "faq", "title": "FAQ" }
  ]
}
```

存在时 **优先于** 自动扫描 + frontmatter `sidebar_position`。适合：

- 强制控制顺序（自动排序不够灵活）
- 创建"没有 index.md 的虚拟分组节点"（如纯链接列表）

## 6. UI 组件（在本 crate 内）

`docs.rs` 提供 3 个 `#[component]`：

| 组件 | 用途 |
| --- | --- |
| `Docs` | `/docs` 落地页：渲染整棵 `DocTreeNode` 列表 |
| `DocPage(path: Vec<String>)` | `/docs/:l1/:l2?/:l3?` 详情页：侧栏 + 主体 Markdown |
| `TreeSection(node, active_path, depth)` | 侧栏单个节点的递归渲染（高亮当前路径） |

## 7. ModuleEngine 集成

`site.json::modules.docs.enabled = false`：

- Navbar 不展示 "Docs" 入口。
- `/docs` 路由 404。
- `sitemap.xml` / `feed.xml` 不收录。
- Tantivy `indexer.rs::collect_docs_versioned` 不索引（`kind="doc"` 文档剔除）。

## 8. 在搜索中的位置

- `kind = "doc"`，`ref_id` 是相对路径（如 `axum/handlers`），`url = /docs/<ref_id>`。
- mtime 差分增量索引（Phase 7.3.3）。
- 模块开关映射：搜索引擎用 `is_on("docs")` 判断（doc kind 对应 docs module id）。

## 9. 测试覆盖

```bash
cargo test --features server -p module-docs
```

**15 个单测**，覆盖：

- frontmatter 解析（含 / 不含 `---`、空字段、`sort_children` 字段）
- `sidebar_position` 排序（`a position=3, b position=1, c position=2` → `b,c,a`）
- `sort_children: desc`（周报场景：`issue-001..005` 按位置降序）
- `_meta.json` 优先于 frontmatter
- 标题 fallback 链（sidebar_label → frontmatter title → H1 → 目录名）

## 10. 不在本期范围

- 文档版本化（同一路径多版本 v1 / v2 / next）
- 国际化（多语言文档同 path 共存）
- 全文检索内嵌（用户已通过 Cmd+K 全站搜索覆盖，docs 也在索引内）
- 跨页 TOC / "下一篇 / 上一篇" 自动导航
- 文档级权限（当前所有 docs 公开）
