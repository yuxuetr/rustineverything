# Phase 6 内容板块（Boards）SPEC

> 范围：`crates/modules/{ai, cli, embedded, wasm, web3}` —— 5 个独立 crate，
> 同构架构。本文档**合并描述**它们的共享设计，差异点单独列在 §6。
> 引擎层 / 模块开关由 [`ENGINES_SPEC.md`](ENGINES_SPEC.md) 的 ModuleEngine 统一调度。

## 1. 设计动机

Phase 6 把 Rust 应用领域拆成 5 个垂直板块（嵌入式 / AI / Web3 / WASM / CLI），
每个板块作为独立 crate 而非共享 generic 模块。Trade-off：

| 维度 | 选择 | 原因 |
| --- | --- | --- |
| 复用度 | **独立 crate，结构重复** | 板块演进节奏不同（嵌入式可能加 RTIC 子主题、AI 可能加 LLM 子主题），共享 generic 反而难分歧；接受 ~15 行的复制 |
| 编译开销 | 5 × ~250 行 ≈ 可控 | 文件少，深度浅，cargo 增量编译命中率高 |
| 测试隔离 | 各 crate 自带 15 单测 | 任何一个板块改动不污染其他板块测试套 |

## 2. 共享文件结构

每个板块 crate 包含 4 个文件：

```
crates/modules/<board>/src/
├── lib.rs        ── 模块声明 + impl AppModule
├── <board>.rs    ── Dioxus UI 组件（落地页 + 详情页占位）
├── server.rs     ── 2 个 server fn + ArticleSummary 结构
└── text.rs       ── 5 个纯函数 helpers + Subtopic / FeaturedCrate 数据
```

其中 `<board>` 即板块名（如 `ai` / `embedded`）。

## 3. server fn 契约

每个板块导出 2 个 server fn（kind / 文章列表 + 单篇内容）：

```rust
#[server]
pub async fn list_<board>_articles() -> Result<Vec<ArticleSummary>, ServerFnError>;

#[server]
pub async fn get_<board>_article(slug: String) -> Result<String, ServerFnError>;
```

| 字段 / 行为 | 说明 |
| --- | --- |
| `ArticleSummary` | `{ slug, title, description, date, tags: Vec<String> }`，前后端共享 |
| 数据源 | `assets/topics/<board>/<slug>/index.md`（frontmatter + markdown body） |
| 返回顺序 | `text::sort_by_date_desc` 按 frontmatter 的 `date` 字段降序 |
| 缺失资产 | 目录不存在 → 返回 `Ok(vec![])`，**不** panic（容忍开发环境无内容） |
| 单篇内容 | `get_*_article(slug)` 返回 raw markdown 字符串，前端用 `widgets::Markdown` 渲染 |

## 4. text.rs：5 个共享 helper + 2 个数据结构

`text.rs` 在每个板块**字节级相同**（独立 crate 选择带来的可控重复）：

```rust
pub struct Subtopic       { pub slug: &'static str, pub label: &'static str }
pub struct FeaturedCrate  { pub name: &'static str, pub url: &'static str, pub description: &'static str }

pub fn normalize_tag(raw: &str) -> String;
pub fn normalize_tags(tags: &[String]) -> Vec<String>;
pub fn subtopic_label(slug: &str) -> Option<&'static str>;
pub fn matches_query(title: &str, description: &str, tags: &[String], query: &str) -> bool;
pub fn sort_by_date_desc<T: DatedArticle>(items: &mut [T]);
```

各板块在 lib 层独立提供 `const SUBTOPICS: &[Subtopic]` + `const FEATURED_CRATES: &[FeaturedCrate]`，
内容不同但接口一致。

## 5. 路由 + ModuleEngine 集成

| 路径 | 行为 |
| --- | --- |
| `/<board>` | 落地页：subtopic chip 筛选 + 关键词搜索 + 文章卡片 + 精选 crate 侧栏 |
| `/<board>/:slug` | 详情页：`widgets::Markdown` 渲染 `get_*_article` 返回的 md |

落地页和详情页都接 ModuleEngine 的开关：`site.json::modules.<board>.enabled = false`
→ Navbar 不展示入口、`sitemap.xml` 不收录、`feed.xml` 不收录、Tantivy `indexer.rs`
不索引该板块（详见 [`SEARCH_SPEC.md`](SEARCH_SPEC.md) §10 板块门禁单测）。

## 6. 板块差异点

板块除内容主题（subtopic / featured crate / 长文 frontmatter）外，结构完全一致。
当前内容覆盖：

| 板块 | 主要子主题（举例） | 真实长文范例 |
| --- | --- | --- |
| `embedded` | no_std / Embassy / RTIC / HAL / defmt / 平台 | no_std 入门 + Embassy 异步固件 |
| `ai`       | 张量 / 推理 / LLM / tokenizers / 训练 / 向量 | candle 本地 LLM |
| `web3`     | EVM / Solana / Substrate / 合约 / 钱包 / 索引 | alloy 读链上状态 |
| `wasm`     | wasm-bindgen / WASI / 组件模型 / 运行时 / 前端 / 插件 | wasmtime 插件沙箱 |
| `cli`      | 参数 / TUI / 输出 / 配置 / 测试 / 分发 | clap derive 子命令 |

## 7. Cases 联动（6.6）

`crates/modules/cases` 通过 `category` + `tag` 把案例自动归类到板块：

- `embedded` / `ai` / `web3` / `cli` 通过 `case.category == <board>` 命中
- `wasm` 通过 `case.tags.contains("wasm")` 命中（兼容案例 schema 早期约定）

板块落地页可侧栏调 `module_cases::server::list_cases(tags=…, category=…)` 拉对应案例。
2026-05-29 起每板块至少 3 个真实 Rust 项目案例（见 Todos.md §6.6）。

## 8. 测试覆盖

```bash
cargo test --features server -p module-<board>   # 替换 <board>
```

每板块 **15 个单测**（13 text + 2 server，共 75 个跨 5 板块）。覆盖：

- `normalize_tag`：大小写 / 空白 / 特殊字符规整
- `matches_query`：title / description / tag 命中 + 大小写不敏感
- `sort_by_date_desc`：缺日期 / ISO 格式 / 倒序
- `subtopic_label`：已知 slug → 中文 label，未知 → None
- server fn：目录不存在 → 空列表（容忍）；存在 → 解析 frontmatter

## 9. 在搜索中的位置

`indexer.rs::collect_boards()` 扫描 5 个板块的 `assets/topics/<board>/*/index.md`，
每篇文章作为一个 `IndexedDocument`，`kind == 板块 id`，`url = /<board>/<slug>`。
增量索引（Phase 7.3.3）按文件 mtime 差分。搜索结果带靛蓝板块徽章，关闭某板块即从结果剔除。

## 10. 不在本期范围

- 板块**之间**的关联推荐（"看了 AI 文章的人也看 WASM"）
- 子主题分类页面（当前是 chip 筛选，未来可改为 `/<board>/topic/<subtopic>`）
- 板块作者多人协作 / 投稿流（当前只读 markdown 资产）
