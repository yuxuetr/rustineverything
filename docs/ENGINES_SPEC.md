# Engine Layer Spec
> 适用阶段：Phase 1C 完成（v2.1 Todos.md）。
> 8 大核心引擎在 `crates/core/src/engines/` 中实现，向上为 server fn / 业务模块提供统一接入点。
## 1. 总体架构
```text
┌──────────────────────────────────────────────────────────────────┐
│                       crates/app (Dioxus + Axum)                 │
│  server fn / 路由 / RSX                                           │
└──────────────────────────────────────────────────────────────────┘
                                 ▲
                  EngineRegistry │
                                 │
┌──────────────────────────────────────────────────────────────────┐
│                  crates/core/src/engines/                        │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ Plugin   │  │ Module   │  │ Auth     │  │ Theme    │          │
│  │ Engine   │  │ Engine   │  │ Engine   │  │ Engine   │          │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘          │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ Layout   │  │ Content  │  │Moderation│  │ Search   │          │
│  │ Engine   │  │ Engine   │  │ Engine   │  │ Engine   │          │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘          │
│                                                                  │
│       共享 Engine trait / EngineRegistry / EngineContext          │
└──────────────────────────────────────────────────────────────────┘
                                 ▲
                                 │ wasmi 调用
                                 │
┌──────────────────────────────────────────────────────────────────┐
│         crates/plugins/  (WASM cdylibs + assets/plugins/*.wasm)  │
└──────────────────────────────────────────────────────────────────┘
```
## 2. 共享抽象
### 2.1 `Engine` trait
所有引擎的共同骨架，定义在 `crates/core/src/engines/mod.rs:73`。
```rust
pub trait Engine: Send + Sync + Any {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &EngineContext) -> AppResult<()> { Ok(()) }
    fn shutdown(&mut self) -> AppResult<()> { Ok(()) }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```
- `name()` 唯一名（注册表 key）。
- `init()` 注册后 `EngineRegistry::init_all` 触发，可读 `ctx.site_config`。
- `shutdown()` `EngineRegistry::shutdown_all` 反序触发，释放资源。
- `as_any` / `as_any_mut` 供按类型 downcast。
### 2.2 `EngineRegistry`
按注册顺序保存 `Box<dyn Engine>`：
- `register::<E>(engine)` — 重复名返回 `AppError::Other`
- `init_all(&ctx)` / `shutdown_all()` — 顺序 / 逆序触发钩子
- `get::<E>(name)` / `get_mut::<E>(name)` — 类型不匹配返回 `None`
- `names() -> Vec<&'static str>` — 已注册引擎名清单
### 2.3 `EngineContext`
`init` 阶段注入的共享上下文：
```rust
pub struct EngineContext {
    pub site_config: Arc<SiteConfig>,
    pub asset_root: PathBuf,
}
```
不直接放 `PluginManager` / DB 句柄，避免和 `EngineRegistry` 互相借用：插件管理器交由 `PluginEngine` 持有，DB 句柄走 `crate::db::get_or_init_pool()` 全局取出。
## 3. 8 个引擎详述
| 引擎 | name | 主文件 | 职责 |
|---|---|---|---|
| PluginEngine | `plugin` | `engines/plugin.rs` | wasmi Module 缓存 + ABI 校验 + 输出大小限制 + 能力分发 |
| ModuleEngine | `module` | `engines/module.rs` | 业务模块注册中心 + site.json 开关 + 导航/搜索源过滤 |
| AuthEngine | `auth` | `engines/auth.rs` | 包装 AuthService（OAuth/PKCE/state） |
| ThemeEngine | `theme` | `engines/theme.rs` | 主题插件注册 + CSS 聚合 |
| LayoutEngine | `layout` | `engines/layout.rs` | LayoutPack 注册（Phase 3.3 完整实现） |
| ContentEngine | `content` | `engines/content.rs` | MDX ComponentRegistry（Phase 2 完整实现） |
| ModerationEngine | `moderation` | `engines/moderation.rs` | 串行审核流水线 + Verdict（Phase 4 完整实现） |
| SearchEngine | `search` | `engines/search.rs` | SearchSource 注册中心（Phase 3.4 完整迁移） |
### 3.1 PluginEngine
**完整状态**（Phase 1C.2 ✅）。包装 `Arc<PluginManager>`：
- `call(path, fn, input)` — 自动读 manifest，ABI 不兼容拒绝；输出超过 `output_limit`（默认 8MB）报错
- `strict_call(path, fn, input)` — 必须有 manifest，否则视为不兼容
- `try_get_manifest(path)` / `get_manifest(path)` — 读插件 `get_manifest` 导出
- `capabilities_of(path)` / `filter_by_capability(paths, cap)` — 能力分发
- `with_output_limit(n)` — 链式覆盖大小限制
- `shutdown()` 调用 `manager.invalidate_all()` 释放 wasmi cache
ABI 协议：见 `RoadMap.md §2.7` + `crates/sdk/src/lib.rs`。SDK 提供：
- `pack_output(Vec<u8>) -> u64`
- `pack_json(&T) -> u64`
- `read_input(ptr, len) -> &[u8]`
- `PluginManifest` (含 `abi_version` + `capabilities`) + builder API
- `capabilities` 常量模块
### 3.2 ModuleEngine
**完整状态**（Phase 1C.3 ✅）。
```rust
pub struct ModuleSpec {
    pub id: String,
    pub label: String,
    pub routes: Vec<String>,
    pub nav_position: Option<i32>,
    pub enabled: bool,
}
```
- `register(spec)` — 重复 id 拒绝
- `apply_site_config(&site)` — 读 `SiteConfig.modules: HashMap<String, ModuleSettings>` 覆盖 `enabled`
- `is_enabled(id)` / `enabled_modules()` / `enabled_ids()` / `navigation()`（按 `nav_position` 升序，稳定排序）
`SiteConfig::modules` 可用形式：
```json
{
  "modules": {
    "forum": { "enabled": false },
    "podcast": { "enabled": true }
  }
}
```
### 3.3 AuthEngine
**完整状态**（Phase 1C.4 ✅，server-only）。包装现有 `crate::auth::AuthService`：
- `service()` — 暴露 `&AuthService`，server fn 直接调用现有方法（list_available_providers / get_auth_url / handle_callback）
- `replace_service(service)` — Hot reload 时替换内部 service
- `init` 检查 `site_config.auth.enabled`，关闭则只发日志（AuthService 内部自然返回空 provider 列表）
### 3.4 ThemeEngine
**骨架状态**（Phase 1C.4 ✅，Phase 3.1 完整实现）。
- 通过 `Arc<PluginEngine>` 调用每个主题插件的 `get_theme_css` 函数
- `register_theme(path)` / `set_themes(paths)` — 主题栈
- `aggregate_css()` — 按声明顺序拼接（后者覆盖前者）。失败的插件被跳过，不阻断
- `init` 阶段从 `SiteConfig.active_theme`（已有字段）读出默认主题路径
- 失败的插件被跳过，发 `eprintln` 日志
**Phase 3.1 计划补充**：主题栈 `themes: ["base", "ocean"]` 多层覆盖 + 用户 navbar 切换 + cookie 持久。
### 3.5 LayoutEngine
**骨架状态**（Phase 1C.4 ✅，Phase 3.3 完整实现）。
```rust
pub trait LayoutPack: Send + Sync {
    fn name(&self) -> &'static str;
    fn label(&self) -> &'static str;
}
```
- `register::<L>(pack)` — 重复名拒绝
- `set_active(name)` / `active() -> Option<&str>`
- `names() -> Vec<&'static str>` 列出所有已注册
**Phase 3.3 计划补充**：
- `LayoutPack` trait 增加 `render_navbar(ctx) -> Element` / `render_footer` / `render_sidebar` 等 slot 方法（依赖 widgets crate 落地）
- `crates/layouts/{classic,minimal}/` 实现两个 layout 包
- `SiteConfig.active_layout` 字段 + admin 设置页切换
### 3.6 ContentEngine
**骨架状态**（Phase 1C.4 ✅，Phase 2 完整实现）。
```rust
pub trait MdxComponent: Send + Sync {
    fn name(&self) -> &'static str;
    fn render(&self, attrs: &serde_json::Value) -> String;
}
```
- `ComponentRegistry::register / lookup / list / render`
- 未知组件降级为 HTML 注释占位（`<!-- unknown MDX component: NAME -->`）避免渲染失败
**Phase 2 计划补充**：
- `render(attrs)` 改为返回 Dioxus `Element` 替代 HTML 字符串（依赖 widgets crate）
- 现有 7 嵌入组件（YouTube/Bilibili/PodcastCard/Annotation/...）作为默认注册项迁移过来
- `render_mdx_registry()` if-else 链改为 registry 查询
### 3.7 ModerationEngine
**骨架状态**（Phase 1C.4 ✅，Phase 4 完整实现）。
```rust
pub enum ModerationLabel { Allow, Flag, Block }
pub struct Verdict { pub score: f32, pub label: ModerationLabel, pub reason: String }
pub trait ModerationStage: Send + Sync {
    fn name(&self) -> &'static str;
    fn evaluate(&self, content: &str) -> Verdict;
}
```
管道行为：
- 串行调用 stages
- 任一 `Block` 立即返回（早停）
- 所有 `Flag` 中取最高 score 的作为最终结果
- 全部 `Allow` 默认通过
- `Verdict::block(score, ...)` / `Verdict::flag(score, ...)` 自动 clamp 到 `[0, 1]`
**Phase 4 计划补充**：
- `evaluate` 改为 `async`，输入改为 `Submission { text, image_url }`（含视觉审核）
- 内置 stages: `LLMStage` / `VLMStage`（OpenAI / Anthropic / LlamaGuard）
- 阈值配置 `block_above` / `flag_above`
- 5s 超时 + fail-open（Allow + 标记需复核）
- 数据库表 `moderation_log` / `moderation_decisions` / `moderation_queue`
### 3.8 SearchEngine
**骨架状态**（Phase 1C.4 ✅，Phase 3.4 完整迁移）。
```rust
pub struct SearchDocument {
    pub kind: String,
    pub ref_id: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub created_at: String,
}
pub trait SearchSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn collect(&self) -> Vec<SearchDocument>;
}
```
- `register(source)` — 模块自注册数据源
- `collect_all()` — 全部源 documents
- `collect_filtered(&enabled)` — 按 `ModuleEngine::enabled_ids()` 过滤
**Phase 3.4 计划补充**：
- 将 `modules/search/src/indexer.rs` 中硬编码的 4 个源（blog / docs / topics / cases）迁移到 `SearchSource` 实现
- 索引时调 `engine.collect_filtered(&module_engine.enabled_ids())`
- `MmapDirectory` 替代 `RAMDirectory`，增量索引（Phase 7.3）
## 4. 生命周期
```text
┌────────────────┐
│ App boot       │
│ main.rs        │
└────────┬───────┘
         ▼
┌────────────────┐
│ build registry │
│ + register all │
│ engines        │
└────────┬───────┘
         ▼
┌────────────────┐
│ init_all(&ctx) │      <- 按注册顺序，被依赖者先注册
└────────┬───────┘         （PluginEngine → ThemeEngine → ...）
         ▼
┌────────────────┐
│ serve traffic  │      <- server fn 通过 registry.get::<E>(name) 取出
└────────┬───────┘         具体引擎使用
         ▼
┌────────────────┐
│ shutdown_all() │      <- 按注册逆序
└────────────────┘         （ThemeEngine 先于 PluginEngine 关闭）
```
## 5. 依赖关系
```text
PluginEngine ──┐
               ├─► ThemeEngine（调 wasm 拿 CSS）
               └─► AuthEngine（调 auth 插件）
ModuleEngine ──┐
               ├─► SearchEngine（按 enabled_ids 过滤源）
               └─► （未来）LayoutEngine 决定哪些 nav 项显示
所有引擎 ─► EngineContext.site_config（只读）
```
## 6. 测试覆盖
Phase 1C 完成时：
- 工作区 `cargo test --features server --workspace` 全绿
- `engines::*` 单测合计 57 个
  - mod.rs（注册/init/shutdown）：8
  - plugin.rs：12（含 3 个真实 wasm 集成测试）
  - module.rs：10
  - theme.rs：4
  - layout.rs：4
  - content.rs：5
  - moderation.rs：6
  - auth.rs：4
  - search.rs：4
- SDK manifest / pack helpers 单测：10
## 7. 后续阶段
| Phase | 主要落地 |
|---|---|
| 2 | ContentEngine 接入 widgets crate；7 嵌入组件迁移到 ComponentRegistry |
| 3 | ThemeEngine 多层主题栈；LayoutEngine 接入 classic / minimal 包；ModuleEngine 切换 admin UI |
| 3.4 | SearchEngine：迁移 indexer.rs 硬编码 4 源到 SearchSource |
| 4 | ModerationEngine：LLM/VLM stages 落地；ModerationProvider WASM ABI |
| 5 | PluginEngine：Hot reload + 内存回收验证；Extism / wit-bindgen ABI v2 切换（可选） |
## 附：关键文件路径
- `crates/core/src/engines/mod.rs:73` — `Engine` trait
- `crates/core/src/engines/mod.rs:100` — `EngineRegistry`
- `crates/core/src/engines/plugin.rs:38` — `PluginEngine`
- `crates/core/src/engines/module.rs:87` — `ModuleEngine`
- `crates/core/src/engines/auth.rs:24` — `AuthEngine`（仅 server）
- `crates/core/src/engines/theme.rs:18` — `ThemeEngine`
- `crates/core/src/engines/layout.rs:25` — `LayoutEngine`
- `crates/core/src/engines/content.rs:80` — `ContentEngine`
- `crates/core/src/engines/moderation.rs:78` — `ModerationEngine`
- `crates/core/src/engines/search.rs:42` — `SearchEngine`
- `crates/sdk/src/lib.rs:9` — `SDK_ABI_VERSION`
- `crates/sdk/src/lib.rs:23` — `PluginManifest`
