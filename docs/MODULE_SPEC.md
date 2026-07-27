# Module Spec

> 适用阶段：Phase 1C.3 引擎抽象 + Phase 3.4 配置化开关（v2.1 Todos.md）。
> 模块（blog / podcast / forum / docs / course / cases / ...）是站点的业务子系统；
> 通过 `site.json::modules.<id>.enabled` 单字段可整体开/关。

## 1. 概念

```text
ModuleSpec  ─┐
            │   register
            ▼
        ModuleEngine  ◀── site.json::modules (overrides)
            │
            ├─ enabled_ids()     → search indexer / sitemap / feed
            ├─ navigation()      → Navbar 主导航
            ├─ is_enabled(id)    → ModuleGate 单页门禁
            └─ enabled_modules() → admin 后台展示
```

`ModuleSpec` 是模块**静态声明**（id / label / routes / nav_position）；
`ModuleEngine` 是**注册中心 + 运行时开关**。两者均在 `crates/core/src/engines/module.rs` 实现。

## 2. ModuleSpec

```rust
pub struct ModuleSpec {
    pub id: String,             // 唯一 key（与 site.json 对齐）
    pub label: String,          // 显示名
    pub routes: Vec<String>,    // 模块拥有的顶层路由（用于 sitemap / 文档）
    pub nav_position: Option<i32>, // None = 不出现在主导航
    pub enabled: bool,          // 默认 true，可被 site.json 覆盖
}
```

builder API：

```rust
let spec = ModuleSpec::new("blog", "Blog")
    .with_routes(["/blog", "/blog/:id"])
    .with_nav_position(10);
```

## 3. 内置模块清单

`default_module_specs()`（`engines/module.rs:101`）返回 11 个内置模块（Phase 6
新增 5 个内容板块）：

| ID | 显示名 | 路由 | nav_position |
| --- | --- | --- | --- |
| `blog` | Blog | `/blog`, `/blog/:id` | 10 |
| `podcast` | Podcast | `/podcast` | 20 |
| `cases` | 案例 | `/case`, `/case/:slug` | 30 |
| `forum` | 论坛 | `/topics`, `/topics/new`, `/topics/tag/:tag`, `/topics/:id` | 40 |
| `embedded` | 嵌入式 | `/embedded`, `/embedded/:slug` | 50 |
| `ai` | AI | `/ai`, `/ai/:slug` | 60 |
| `web3` | Web3 | `/web3`, `/web3/:slug` | 70 |
| `wasm` | WASM | `/wasm`, `/wasm/:slug` | 80 |
| `cli` | CLI | `/cli`, `/cli/:slug` | 90 |
| `course` | 课程 | `/course`, `/course/:slug`, `/course/:slug/:chapter/:lesson` | — |
| `docs`  | 文档 | `/docs`, `/docs/*` | — |

内容板块（embedded/ai/web3/wasm/cli）是独立 crate（`crates/modules/<board>`），
各扫描 `assets/topics/<board>/*/index.md`，落地页提供子主题筛选 + 精选 crate
侧栏，详情页复用 `widgets::Markdown`。

> `nav_position = None` 的模块不出现在 Navbar 主导航中，但仍可以独立路由、
> 出现在 sitemap / 搜索源。`docs` 在主导航以「Get Started」CTA 进入。

## 4. site.json 控制开关

```json
{
  "modules": {
    "forum":   { "enabled": false },
    "podcast": { "enabled": true }
  }
}
```

- key 必须与 ModuleSpec.id 一致。
- 缺省项默认 `enabled = true`（`ModuleSettings::default()`）。
- 不存在的 ID 静默忽略（不会报错，便于向后兼容）。

## 5. 服务端读取

`crates/core/src/engines/module.rs`：

```rust
let mut engine = ModuleEngine::with_specs(default_module_specs());
engine.apply_site_config(&site_config);

assert!(engine.is_enabled("blog"));
assert!(!engine.is_enabled("forum"));

let nav: Vec<&ModuleSpec> = engine.navigation();
let search_sources: Vec<String> = engine.enabled_ids();
```

便捷帮手 `default_module_engine()` 已在 server feature 下封装上面 4 行：

```rust
let engine = app_core::engines::module::default_module_engine();
let ids = engine.enabled_ids();
```

## 6. 与各子系统的集成

### 6.1 Navigation（导航栏）

`crates/app/src/components/layouts/classic.rs::ClassicShell` 在 mount 时拉
[`enabled_module_ids`](../crates/app/src/server/mod.rs) server fn，按布尔决定渲染：

```rust
nav { class: "...",
    if on_blog    { Link { to: Route::BlogIndex {},   "Blog" } }
    if on_podcast { Link { to: Route::Podcast {},      "Podcast" } }
    if on_cases   { Link { to: Route::Cases {},        "Cases" } }
    if on_forum   { Link { to: Route::TopicsIndex {}, "Forum" } }
}
```

Footer 链接、用户菜单中的「我的话题」、CTA「Get Started → Docs」均按相应模块开关动态显示。

### 6.2 路由门禁 (ModuleGate)

`crates/app/src/components/module_gate.rs` 提供 `ModuleGate { id, children }`：

```rust
#[component]
pub fn BlogIndex() -> Element {
    rsx! { ModuleGate { id: "blog".to_string(), BlogIndexInner {} } }
}
```

- 调 [`is_module_enabled`](../crates/app/src/server/mod.rs) server fn。
- 启用 → 渲染 children。
- 未启用 → 渲染统一占位（302 风格 banner + 「回到首页」链接）。
- server fn 未返回前默认渲染 children，避免首屏闪烁。

### 6.3 搜索索引 (Search Indexer)

`crates/modules/search/src/indexer.rs::collect_documents` 在汇总后按
`default_module_engine().enabled_ids()` 过滤。kind → module id 映射：

| `IndexedDocument.kind` | Module ID |
| --- | --- |
| `blog`  | `blog` |
| `doc`   | `docs` |
| `topic` | `forum` |
| `case`  | `cases` |

未知 kind 默认保留（保守策略，避免误丢数据）。

纯逻辑函数 `filter_documents_by_enabled(docs, &enabled_ids)` 抽离便于单测。

### 6.4 sitemap.xml / feed.xml

`crates/app/src/main.rs` 中两个 Axum 路由：

- **sitemap.xml**：静态路径数组（`/`, `/blog`, `/podcast`, ...）按 `is_on(id)` 拼接；
  blog 关闭时跳过 `list_blog_posts` 调用（节省 IO）。
- **feed.xml**：blog 关闭时 `entries = []`，但仍输出有效 RSS/Atom 骨架（站点元信息保留）。

`/robots.txt` 由 `build_robots_txt(&base_url)` 生成，与模块开关无关。

## 7. 测试覆盖

- `crates/core/src/engines/module.rs`：13 个单测
  - ModuleSpec builder / 注册查询 / 重复 id 拒绝
  - site.json 关闭 / navigation 过滤 / 搜索源过滤
  - `default_module_specs` 含 11 内置 / nav 9 项 / 关闭传导到 nav 与 enabled_ids
- `crates/modules/search/src/indexer.rs`：3 个 filter 测试
  - 仅保留 enabled 模块文档
  - 空 enabled → 全部丢弃
  - 未知 kind 默认保留

`cargo test --features server --workspace` 全绿（319 tests pass; Phase 3.4 提交 8 个新测试）。

## 8. 实际开关示例

### 关闭论坛

```jsonc
{
  "modules": {
    "forum": { "enabled": false }
  }
}
```

预期行为：

- ✅ Navbar 不再渲染「论坛」链接
- ✅ 用户菜单不再渲染「我的话题」
- ✅ 用户访问 `/topics` 系页面看到统一「模块已停用」占位
- ✅ 全站搜索结果不再含 `kind=topic`
- ✅ sitemap.xml 不再列出 `/topics`
- ✅ feed.xml 不受影响（feed 只追踪 blog）

### 关闭博客 + 案例

```jsonc
{
  "modules": {
    "blog":  { "enabled": false },
    "cases": { "enabled": false }
  }
}
```

- ✅ Navbar 仅剩 podcast + forum
- ✅ sitemap 仅含 `/`, `/podcast`, `/topics`
- ✅ feed.xml 输出空 entries 但保留 channel 信息

## 9. 与其他引擎的关系

| 引擎 | 关系 |
| --- | --- |
| `PluginEngine` | 不直接关联；插件按 capability 注册到对应引擎 |
| `ThemeEngine` | 正交（[`THEME_SPEC.md`](THEME_SPEC.md)） |
| `LayoutEngine` | 正交（壳决定结构，模块决定内容） |
| `AuthEngine` | 正交（auth 不是「模块」） |
| `SearchEngine` | 消费 `enabled_ids()` 过滤索引源 |
| `ContentEngine` | 正交（MDX 组件注册与模块开关无关） |
| `ModerationEngine` | 后续 Phase 4：审核流水线会按模块开关启用各 hook |

## 10. 后续阶段

- **Phase 6**（已完成）：5 个内容板块（embedded / ai / web3 / wasm / cli）已通过 `default_module_specs` 注册，享受同样的开关能力（nav / 路由 gate / sitemap / feed）。
- **Phase 5**：Admin 页面提供 UI 切换 `modules.<id>.enabled`，写回 site.json（目前只能手改文件 + hot reload「重新载入」生效）。
- **Phase 7**：CI/CD 在不同环境（staging / prod）通过 site.json 控制模块灰度。

## 11. 模块依赖规则（Module Dependency Policy）

> 来源：2026-06-26 架构评估。目标是保持分层单向、避免内容模块间的“横向”编译期耦合。

### 11.1 允许的依赖方向

```text
sdk-macros → sdk → {core, widgets}
core → llm
{core, widgets, sdk} → modules/*        （单向：基础设施 → 业务模块）
modules/* → app                          （单向：业务模块 → 组合根）
```

- **内容模块（`crates/modules/<id>`）只可依赖 `app-core` / `sdk` / `widgets`**，
  不得依赖其他兄弟内容模块。
- **跨模块的 UI 组合发生在组合根 `app`**（它可以依赖一切模块）：需要把
  A 模块的组件放进 B 模块页面时，由 `app` 通过插槽（`Element` prop / children）注入，
  而不是让 B 直接依赖 A。
- **跨模块的数据依赖通过 `app-core` 中立抽象倒置**（IoC 注册表），由 `app`
  在启动期注入具体实现，避免消费方编译期依赖生产方。
- **`module-moderation`** 是可选的审核基础设施（依赖 `core`+`llm`+`sdk`），可被其他
  内容模块以 `optional = true` 引入（如 forum / comments / admin）——这属于“业务模块 →
  审核基设”的单向依赖，不违反本规则。

### 11.2 实现手法与现状

- **UI 组合插槽**：`docs` 不再依赖 `course` / `forum`。`module_docs::docs::DocPage`
  接受 `footer: Element` 插槽，由 `app/src/routes/mod.rs` 的 `DocPage` 包装传入
  `AnnotationLayer`（course）+ `DiscussionPanel`（forum）。
- **数据源 IoC**：`search` 不再依赖 `cases`。`app_core::engines::doc_source` 提供
  `register_doc_source` / `collect_registered_docs`；`app` 在启动期把 `cases` 注册为
  外部索引文档来源（见 `app/src/main.rs`）。
- **死依赖清理**：`forum` / `docs` 原有的 `module-blog` / `module-course` 是未使用的
  死依赖，已移除。

### 11.3 当前合规边与例外

- ✅ 所有内容模块仅依赖 `core` / `sdk` / `widgets`（及可选 `module-moderation`）。
- ✅ `app` 依赖全部模块，是唯一的跨模块组合点。
- ⚠️ 例外：`module-moderation` 被 `forum` / `comments` / `admin` 以 `optional` 引入。
  这是可接受的“业务模块 → 审核基设”单向依赖（moderation 不反向依赖任何业务模块）。

### 11.4 新增跨模块交互时的决策

1. 只是“把 A 的组件放进 B 页面”→ 用 `app` 层插槽注入（同 A2）。
2. “B 需要 A 的数据”→ 在 `core` 定义中立类型 + 注册表，`app` 注入（同 A3）。
3. 真正的公共领域类型（多模块共享）→ 上提到 `core` 或 `sdk`。
