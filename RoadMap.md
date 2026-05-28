# Rust in Everything — RoadMap

> 把当前 Dioxus 个人站重构为 **多核心引擎 + 可插拔模块 + 已稳定 MDX 内容内核** 的可扩展平台。
> 本路线图与 `Todos.md` 配套：路线图给「为什么 / 做什么 / 验收什么」，Todos 给「具体要写哪些代码」。

## 1. 愿景与定位

- **个人开发者站点**：作者 = 唯一内容生产者；正文（博客 / 文档 / 课程 / 案例 / 播客）通过 git 推送到 `assets/`，**文件系统是单一事实源**
- **互动而非 UGC**：论坛 / 评论 / 标注是为了**与读者互动**，不是开放创作通道；规模小、需要安全保障多于扩容能力
- **MDX 是内容内核（已稳定）**：`crates/modules/blog/src/markdown.rs` 已交付完整能力 — frontmatter / GFM / 数学 / Mermaid / 代码 + Copy / 标注 block-id / 嵌入组件。后续重点是 **开放组件注册表 + 完整 SEO 注入**，不重写解析器
- **MDX 选型理由**：① 内容里可嵌入交互组件（`PodcastCard` / `YouTube` / `Discussion`）② frontmatter 直接驱动 SEO（meta / og / JSON-LD）
- **站点 = 引擎组合**：主题、布局、栏目、内容、审核、搜索、Auth 都是引擎/插件
- **栏目可开关**：课程、论坛、案例、AI、Web3、嵌入式…由 `site.json` 一键控制
- **审核可插拔**：评论 / 话题 / 上传走统一流水线，支持 LLM/VLM 模型 API 插件 — 个人站开放互动的安全底座
- 第三方贡献者**像写 npm 包一样**新增主题 / 审核 / 通知插件（也方便我自己日后维护）
- **可开源给个人开发者**：项目最终开源，提供给希望搭建个人网站（博客 / 课程 / 文档 / 论坛 …）的 Rust 开发者作为可扩展基础架构。当前 `Rust in Everything` 是作者维护的 Rust 生态内容实例；其他开发者可基于同一基础架构衍生出不同主题站点（如设计周报站、独立游戏开发者站、读书笔记站…）

## 2. 架构总览

### 2.1 三层

```text
┌──────────────────────────────────────────────┐
│  Engines  (crates/core/src/engines/*)        │  8 个核心引擎
├──────────────────────────────────────────────┤
│  Modules  (crates/modules/*)                 │  业务垂直切片 = 可关闭栏目
├──────────────────────────────────────────────┤
│  Plugins  (crates/plugins/*  +  layouts/*)   │  WASM 或 Rust crate
└──────────────────────────────────────────────┘
```

### 2.2 8 个核心引擎

| 引擎 | 职责 | 现状 → 目标 |
|---|---|---|
| ThemeEngine | 主题 CSS 变量栈、Dark/Light、用户切换 | 单 wasm → 多主题 stack |
| LayoutEngine | Navbar/Footer/PageShell/Sidebar 槽位 + LayoutPack trait | 硬编码 → 多 layout 包 |
| ModuleEngine | 栏目注册、启用/禁用、动态生成路由与导航 | 全硬编码 → site.json 驱动 |
| PluginEngine | WASM Module 缓存、能力协商、版本校验、hot reload | 每次重读磁盘 → 缓存 + 协商 |
| ContentEngine | **复用现有 MDX**；开放 ComponentRegistry；SEO frontmatter 全注入 | 已稳定 + 闭合注册 → 开放注册 + 完整 SEO |
| ModerationEngine | 规则阶段 + LLM 阶段 + 队列 + 审计 + 阈值 | 无 → LLM/VLM 流水线 |
| SearchEngine | 模块通过 SearchSource trait 自注册索引源 | 硬编码 4 源 → 可注册 |
| AuthEngine | OAuth + state CSRF + PKCE 持久化 | 已有 → 安全加固 |

> 删除项：~~InteractionEngine~~（评论/话题/标注分散稳定，仅在 ModerationEngine 处统一接 hook 即可）；~~I18nEngine~~（`i18n_fluent_plugin` 已够用，延后）

### 2.3 7 类插件 — 全部 WASM（wasmi 运行时）

| 类别 | 形式 | 现有 | 计划 |
|---|---|---|---|
| AuthProvider | WASM | github / google / discord / twitter | + feishu / wechat / linuxdo |
| Theme | WASM | ocean | + sunset / forest / monochrome / catppuccin |
| Layout | WASM（HTML 模板 + slot） | — | classic / magazine / docs / minimal |
| **MdxComponent** | WASM / 内置 | 7 内置（已稳定） | 开放注册 + 第三方贡献 |
| ModerationProvider | WASM + 宿主 HTTP | — | **openai / anthropic / llamaguard（全 LLM/VLM，不用规则关键词）** |
| Notification | WASM + 宿主 HTTP | — | webhook / discord-bot / feishu-bot / email |
| SearchSource | 模块内置 hook | 内嵌 | 模块自注册（非外部插件） |

> **运行时统一**：所有插件编译为 WebAssembly，宿主使用 **wasmi**（已在 `crates/core/src/lib.rs::PluginManager` 接入）。
> **推荐开发流程**：用 **Rust** 写 cdylib → `cargo build --release --target wasm32-unknown-unknown` → 产出的 `.wasm` 复制到 `assets/plugins/<name>.wasm` → 在 `site.json::plugins` 注册即生效（admin hot reload 见 Phase 5）。
> 复杂 Dioxus 响应式 Layout / MdxComponent 可保留为 workspace 内 Rust crate（编译期合入）；纯展示型组件优先 WASM 化以方便第三方贡献。
> **ABI 规范**：所有插件遵循统一的 `describe + map` auth-like 接口模式，详见 §2.7。

### 2.4 MDX 内核（已稳定，仅扩展）

**现状**（`crates/modules/blog/src/markdown.rs` 已交付）：
- `parse_mdx()` — frontmatter（YAML：title/description/keywords + 自定义字段）
- `pulldown-cmark` 全 GFM：表格、脚注、删除线、任务列表、数学、admonitions（`:::tip` `:::info` `:::note` `:::warning` `:::important` `:::error` `:::caution`）
- 代码块 + Copy 按钮 + Prism 高亮（rust/bash/toml/json/yaml/python）
- Mermaid 图表（`flowchart` / `timeline` 等）
- 数学 inline `$...$` 与 display `$$...$$` → MathML（pulldown-latex）
- 嵌入组件：`<PodcastCard id />` / `<YouTube id />` / `<Bilibili id />` / `<Yellow|Green|Blue|Pink|Purple text />` / `<Underline|Strikethrough text />`
- 图片相对路径 `cc-agent-skills.webp` 自动解析为 `/posts/<slug>/<file>`
- 顶层块自动注入 `data-block-id="bN"` 供标注定位
- 验证：`assets/posts/welcome/index.mdx` 192 行内容全部正常渲染

**Phase 2 仅做两件事**：
1. **开放 ComponentRegistry**：把 `render_mdx_registry` 的 if-else 改成 trait + 注册 API；模块/插件可注册自家组件
2. **完整 SEO 注入**：description / keywords / og:* / twitter:* / canonical / JSON-LD Article schema

### 2.5 审核流水线

```text
text content  ──► LLMStage(并行多 provider) ──┐
image content ──► VLMStage(并行多 provider) ──┴─► AggregateVerdict
                          ▲
                          │
                  ModerationProvider WASM 插件 (wasmi)
                          │
                          ▼
                  host HTTP client → LLM / VLM API
                          │
                          ▼
                  map_verdict → Verdict{score,label,reason}

verdict.label:
  ├ Allow → 入库
  ├ Flag  → 入库 + admin 队列
  └ Block → 拒绝 + 通知用户 + 记录
```

- LLM / VLM 阶段超时 / 失败 = fail-open（Allow + 标记 needs_review）
- 阈值与策略可在 admin 配置：`block_above`、`flag_above`
- **全部走 LLM/VLM**：评论 / 话题 / 话题回复 → LLM 文本审核；上传图片 → VLM 视觉审核；**不使用传统规则关键词或正则黑名单**

### 2.6 SEO 与发现

```text
frontmatter (yaml)
   ├─ title / description / keywords / image / date / tags / author
   │
   ▼
ContentEngine.inject_seo(meta, current_url)
   ├─ <title> + <meta name="description"> + <meta name="keywords">
   ├─ Open Graph: og:title / og:description / og:image / og:url / og:type
   ├─ Twitter Card: twitter:card / twitter:title / twitter:image
   ├─ <link rel="canonical">
   └─ <script type="application/ld+json"> Article schema

站点级
   ├─ /sitemap.xml — ModuleEngine 收集所有内容页
   ├─ /feed.xml    — 博客 Atom feed (最近 50 篇)
   └─ /robots.txt  — sitemap 链接
```

### 2.7 插件 ABI 规范（auth-like 统一标准）

所有第三方 WASM 插件遵循统一的「describe + map」函数模式，骨架与现有 `crates/plugins/github-auth` 一致。SDK（`rustineverything-sdk`）提供所有共享类型与默认 alloc/dealloc 实现。

#### 2.7.1 内存与函数契约

每个插件 cdylib 必须导出 SDK 提供的内存管理函数：

```rust
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8;

#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize);
```

所有业务函数统一签名：

```rust
#[no_mangle]
pub unsafe extern "C" fn <name>(ptr: *mut u8, len: usize) -> u64
```

返回值 64 位打包：**高 32 位 = 输出指针，低 32 位 = 输出长度**。宿主读取后调用 `dealloc(ptr, len)` 释放。

#### 2.7.2 两类业务函数

| 类别 | 用途 | 输入 | 输出 |
|---|---|---|---|
| `describe` 类 | 声明配置 / 端点 / 元信息（一次性获取） | 通常为空字节 | JSON / CSS / HTML 字符串 |
| `map` 类 | 在不同数据格式之间转换（每次调用） | JSON 字符串 | JSON / HTML / 字符串 |

#### 2.7.3 各类插件必备函数清单

| 插件类别 | describe 函数 | map 函数 | 输入→输出 |
|---|---|---|---|
| AuthProvider | `get_provider_config` / `get_display_info` | `map_profile` | OAuth profile JSON → `StandardUser` JSON |
| Theme | `get_theme_css`（直返 CSS） | — | — → CSS 字符串 |
| ModerationProvider | `get_endpoint` | `map_request` / `map_verdict` | content JSON → 请求体；response JSON → `Verdict` JSON |
| Notification | `get_endpoint` | `map_request` | event JSON → webhook body |
| Layout | `get_layout_info` | `render_slot` | `{ slot_name, ctx }` JSON → HTML 字符串 |
| MdxComponent | `get_component_info` | `render` | attrs JSON → HTML 字符串 |
| I18n | — | `translate` | `{ key, lang }` JSON → 翻译字符串 |

#### 2.7.4 通用 manifest（所有插件必备）

```rust
#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64;
```

返回 SDK 中定义的 `PluginManifest`：

```rust
pub struct PluginManifest {
    pub id: String,                // 如 "feishu-auth"
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub abi_version: u32,          // 必须等于 SDK_ABI_VERSION
    pub capabilities: Vec<String>, // ["auth-provider"] / ["theme"] / ["moderation"] / ...
}
```

#### 2.7.5 错误处理

- 业务函数解析输入失败 → 返回 JSON `{ "error": "<message>" }`
- 宿主加载时校验 `abi_version` 不匹配 → 拒绝并提示升级
- 插件 panic → wasmi 捕获 trap，宿主记录日志 + 标记插件不可用 + 发 admin 通知
- **禁止** `unwrap` / `expect`（遵循项目代码规范）

#### 2.7.6 实现骨架

每个导出函数 6 步走（参考 `crates/plugins/github-auth/src/lib.rs`）：

```rust
#[no_mangle]
pub unsafe extern "C" fn map_profile(ptr: *mut u8, len: usize) -> u64 {
    // 1. 读输入
    let input = slice::from_raw_parts(ptr, len);
    // 2. 反序列化
    let raw: Value = serde_json::from_slice(input).unwrap_or_default();
    // 3. 业务逻辑（无 unwrap / expect）
    let result = StandardUser { /* ... */ };
    // 4. 序列化输出
    let bytes = serde_json::to_string(&result).unwrap_or_default().into_bytes();
    // 5. 在插件内分配输出内存
    let out_len = bytes.len();
    let out_ptr = alloc(out_len);
    slice::from_raw_parts_mut(out_ptr, out_len).copy_from_slice(&bytes);
    // 6. 打包返回
    ((out_ptr as u64) << 32) | (out_len as u64)
}
```

#### 2.7.7 禁止依赖

- ❌ `dioxus = { features = ["web"] }` / `wasm-bindgen` / `web-sys` / `js-sys`（wasmi 无 JS 桥）
- ❌ 浏览器 API（`window` / `document` / `localStorage`）
- ❌ Tokio / async 多线程运行时（wasmi 单线程）
- ✅ 推荐：`serde` / `serde_json` / `chrono`（只启 serde feature）/ 纯数据处理 crate
- ✅ HTML 输出可选：`format!()` / `maud` / `dioxus-core` + `dioxus-ssr`（仅静态渲染）

## 3. 阶段路线图

### Phase 0 — 已完成基线 ✅
- 7 业务模块、6 WASM 插件、JWT/Session、PG + SeaORM、Tantivy + jieba 搜索、admin 后台、DiscussionPanel
- **MDX 渲染管道已稳定**（`markdown.rs` + welcome 示例覆盖 GFM / 数学 / Mermaid / 7 嵌入组件）

### Phase 1 — 引擎层重构（重要前置）
**目标**：把当前散落在 `core/lib.rs`、`core/auth/mod.rs`、`app/src/server/mod.rs` 的能力抽象为 8 个引擎；解决 DB 连接 / 插件缓存两大性能债

**关键产出**
- `crates/core/src/engines/{theme,layout,module,plugin,content,moderation,search,auth}.rs`（8 个）
- `Engine` trait + `EngineRegistry`
- PluginEngine：`Module` 缓存（mtime 失效）+ ABI 版本协商
- DB 连接池单例（`OnceLock<DatabaseConnection>`）
- `crates/app/src/server/mod.rs` 瘦身至 ≤ 200 行；评论 / 文档 / 上传 server fn 下沉到对应模块
- 文档：`docs/ENGINES_SPEC.md`

**验收**
- `cargo test --features server --workspace` 全绿
- 评论列表 P95 延迟下降 ≥ 50%

### Phase 2 — MDX 开放注册 + SEO 完善
**目标**：保留现有稳定 MDX；让模块/插件可贡献组件；前端 SEO 一次到位

**关键产出**
- 把 `crates/modules/blog/src/markdown.rs` 抽到 `crates/widgets/src/mdx.rs`（MDX 已稳定，仅搬位置 + 解耦 podcast 直接 import）
- `MdxComponent` trait + `ComponentRegistry::register(name, fn)`；现有 7 嵌入组件改为通过注册 API 接入
- 各模块注册自家组件：`Discussion` / `Comment` / `PodcastCard` / `Annotation`
- ContentEngine SEO 注入器：description / keywords / og:* / twitter:* / canonical / JSON-LD
- `/sitemap.xml`（ModuleEngine 收集路由）
- `/feed.xml`（博客 Atom）
- `/robots.txt`
- 文档：`docs/MDX_SPEC.md`、`docs/SEO_SPEC.md`、`docs/components/<Component>.md`

**验收**
- 现有 welcome 示例 + 全部内容渲染像素级一致
- Lighthouse SEO 评分 ≥ 95
- 第三方加新 MDX 组件 ≤ 50 行
- sitemap 通过 google sitemap test；feed 通过 W3C feed validator

### Phase 3 — 主题 / 布局 / 模块开关
**目标**：站点形态完全由 `site.json` 决定

**关键产出**
- ThemeEngine 支持 stack：`themes: ["base", "ocean"]`，后者覆盖前者
- 4 主题 wasm：`theme-sunset`、`theme-forest`、`theme-monochrome`、`theme-catppuccin`
- 4 layout 包：`crates/layouts/{classic,magazine,docs,minimal}`
- ModuleEngine：`site.json::modules.{blog,podcast,course,forum,cases,docs,ai,web3,embedded,wasm,cli}.enabled`
- 用户主题/布局切换 UI（cookie 持久）
- 文档：`docs/THEME_SPEC.md`、`docs/LAYOUT_SPEC.md`、`docs/MODULE_SPEC.md`

**验收**
- 关闭某模块后路由/导航/搜索源/sitemap 全部消失
- 切换 layout 不需重启 dx serve
- 4 主题切换在前端可见 + 持久

### Phase 4 — 互动 + 内容审核（个人站安全底座）
**目标**：评论 / 话题 / 上传走统一审核流水线，支持 LLM/VLM 插件

**为什么对个人站尤其重要**
- 单一管理员，不可能 7×24 人工审核 → LLM 前置异步过滤是必需
- 内容是 SoT 但**用户互动是变量**，需要安全网防垃圾 / 仇恨 / SQL 注入文本
- VLM 用于上传图片审核，避免违规图片污染站点

**关键产出**
- 评论 / 话题创建 / 话题回复 / 标注 / 上传 全部接 ModerationEngine hook
- ModerationEngine：`Vec<Box<dyn ModerationStage>>`，串行 + 早停（任一 Block 即终止）
- ModerationProvider WASM ABI：`get_endpoint() / map_request(content) / map_verdict(response)`，宿主代调 HTTP
- 内置审核插件（**全部 LLM/VLM**，wasmi 运行时，**无规则关键词路径**）：`moderation-openai` / `moderation-anthropic` / `moderation-llamaguard`（本地 ollama，作 fallback）
- VLM：`moderation-openai` / `moderation-anthropic` 扩展支持视觉模型
- 数据库：`moderation_log` / `moderation_decisions` / `moderation_queue`
- Admin 审核界面：队列 / 全链路审计 / 阈值与启用插件配置
- 文档：`docs/MODERATION_SPEC.md`、`docs/INTERACTION_SPEC.md`

**验收**
- 评论审核 P95 ≤ 1.5s（LLM 超时降级 Allow + 标记）
- 模拟违规内容触发 Block / Flag 路径正确
- LLM 失败时用户体验不阻塞
- admin 队列可见 Flag 内容并可复核

### Phase 5 — 插件生态（个人站口径：自用为主 + 朋友贡献）
**关键产出**
- ABI 版本号 `SDK_ABI_VERSION` + capabilities + manifest 校验
- Hot reload：admin 上传 wasm → 校验 → 替换 → 重新初始化
- 脚手架 CLI `dx-plugin new <kind> <name>`（自己用方便）
- 5 个示例插件项目：theme / auth / moderation / mdx-component / notification
- 文档：`docs/PLUGIN_DEV.md`、`docs/PLUGIN_ABI.md`、`docs/PLUGIN_RECIPES.md`
- **插件市场**（项目开源后启用）：
  - `assets/plugins/registry.json` 维护已审核插件清单 + 站点 `/plugins` 浏览页
  - 第三方提交必须附 **源代码（GitHub repo）+ wasm 产物 + manifest**
  - 仓库维护者**严格审核源代码、ABI 兼容性、安全性**，通过后才纳入 registry
  - 未审核插件**不在前端展示**，避免供应链风险
- 全部插件统一 **wasmi** 运行时；文档强调 **Rust → wasm** 推荐路径

**验收**
- 自测 30 分钟内做出新主题
- admin 上传 wasm 不需重启
- 不兼容版本被拒绝并提示升级

### Phase 6 — 内容板块扩展（推送内容驱动）
**目标**：补齐 Rust 全方位生态；每个板块都是「我会持续推送内容」的栏目

**关键产出**
- `crates/modules/embedded` — Rust 嵌入式（no_std、Embassy、RTIC、stm32 / esp32 / rp2040）
- `crates/modules/ai` — Rust AI（candle、burn、llm 推理、tokenizers、ort）
- `crates/modules/web3` — Rust Web3（alloy、solana-sdk、anchor、substrate）
- `crates/modules/wasm` — WASM 专题（wasm-bindgen、wasi、组件模型）
- `crates/modules/cli` — CLI 工具（clap、ratatui、indicatif）
- 每模块遵循 `lib.rs + <name>.rs(UI) + server.rs + text.rs(纯逻辑)` 风格 + 单元测试 ≥ 12
- 与 `Cases` 联动：每模块有"精选案例"标签
- 每模块若干 MDX 种子内容（按节奏推送，不强求 MVP 数量）
- 文档：每模块对应 `docs/<MODULE>_SPEC.md`

**验收**
- 5 个新模块可独立 `cargo test -p` 通过
- ModuleEngine 一键开关
- 案例库自动按模块归类

### Phase 7 — 个人站可托管（精简版）
**目标**：从「dev only」过渡到「能 24×7 在线」，不追求企业级

**保留**
- `sea-orm-migration` 替代 `init.sql`
- Auth 加固：state CSRF（短 TTL）、PKCE 加密 cookie 持久化、JWT 密钥强制 env
- 搜索持久化：`MmapDirectory` + 增量索引
- 简单审计日志：admin 写操作 / 审核决策落表
- `Dockerfile` + `docker-compose.yml`（app + postgres；审核走托管 LLM API）
- GitHub Actions CI：fmt + clippy + test + build wasm + build app
- 基础结构化日志：`tracing` + `tracing-subscriber`

**删除（个人站过度工程）**
- ~~`tower-governor` 限流~~（流量低，反向代理层处理足够）
- ~~OpenTelemetry / Grafana~~（基础 tracing 文件日志够用）
- ~~distroless~~（普通 alpine 即可）
- ~~OWASP Top10 强制清单~~（基础安全审视即可）

**验收**
- CI 全绿
- `docker compose up` 一键启动 + 自动迁移
- JWT / state / PKCE 三项加固完成

## 4. 里程碑

| 里程碑 | 完成 Phase | 关键能力解锁 |
|---|---|---|
| M1 | 1 | 引擎注册 + 性能基线 |
| M2 | 1 + 2 | MDX 开放注册 + SEO 一次到位 |
| M3 | + 3 | 站点形态完全配置化 |
| M4 | + 4 | 评论/话题安全可信（开放互动安全底座） |
| M5 | + 5 | 插件生态（自用与朋友贡献） |
| M6 | + 6 | 全栈 Rust 内容覆盖 |
| M7 | + 7 | 个人站可托管 24×7 |

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| MDX 注册重构引入回归 | 现有 7 组件作为默认注册项；welcome 示例像素级回归测试 |
| WASM 不能直接联网 | 审核 / 通知插件采用「插件描述端点 + 宿主代调」模式 |
| 多主题叠加 CSS 体积膨胀 | LayoutEngine 编译期裁剪 + Tailwind purge |
| 审核 LLM 延迟 | fail-open + 异步队列 + 分级阈值 |
| 模块开关引入条件编译噪声 | 用动态注册而非 cfg |
| 内容板块扩展耗时 | 每模块迭代式上线；不强求 MVP 数量 |
| SEO 指标不达预期 | Lighthouse + sitemap validator + feed validator 三道门禁 |

## 6. 参考目录约定

```text
crates/
├── core/
│   └── src/engines/     # 8 引擎
├── widgets/             # MDX (从 blog/markdown.rs 搬来) + ComponentRegistry + SEO 注入
├── layouts/             # Rust crate 形式 layout 包
│   ├── classic/  magazine/  docs/  minimal/
├── modules/
│   ├── blog/  podcast/  course/  forum/  cases/  docs/  search/  admin/
│   └── ai/  web3/  embedded/  wasm/  cli/   # Phase 6
├── plugins/
│   ├── auth-{github,google,discord,twitter,feishu,wechat}/
│   ├── theme-{ocean,sunset,forest,monochrome,catppuccin}/
│   ├── moderation-{openai,anthropic,llamaguard}/
│   ├── notification-{webhook,discord-bot,feishu-bot,email}/
│   └── i18n-fluent/
└── app/                 # Dioxus 入口（薄层）

docs/
├── ENGINES_SPEC.md
├── MDX_SPEC.md  SEO_SPEC.md
├── THEME_SPEC.md  LAYOUT_SPEC.md  MODULE_SPEC.md
├── INTERACTION_SPEC.md  MODERATION_SPEC.md
├── PLUGIN_DEV.md  PLUGIN_ABI.md  PLUGIN_RECIPES.md
├── DEPLOY_GUIDE.md  OPERATIONS.md
└── components/<Component>.md   # 每个 MDX 组件
```
