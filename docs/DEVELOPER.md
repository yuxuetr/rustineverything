# Rust in Everything 开发者文档

欢迎来到 **Rust in Everything** 项目！本系统是一个基于 Dioxus 0.7 的全栈 Web 应用。本文档旨在帮助开发者理解项目架构、现有功能，以及如何添加主题、登录方式与审核规则等能力。

---

## 1. 项目核心功能 (Current Features)

*   **全栈内容管理**：
    *   **Blog 模块**：支持 MDX 语法，具备 Frontmatter 解析、LaTeX 数学公式、代码高亮（Prism.js）、GFM Alert (Admonitions) 支持。
    *   **Podcast 模块**：内置音频播放系统，支持列表切换与元数据管理。
    *   **Doc 模块**：以目录为单位的三级文档树，支持 frontmatter SEO、`sidebar_position` / `sort_children` 控制侧栏。
    *   **Course 模块**： `Course → Chapter → Lesson` 三级模型，适配 Doc / Video / Audio / Code 四种课节布局，含进度与标注系统。
    *   **Forum 模块**：话题 / 回复 / Tag；支持从博客、文档、课节页面直接发起讨论并建立资源引用关联（详见 `docs/FORUM_SPEC.md`）。
*   **内置扩展点**（均编译进宿主，无运行时插件）：
    *   **主题**：内置 CSS 变量主题，访客可切换（`docs/THEME_SPEC.md`）。
    *   **登录方式**：GitHub / Google / Discord / Twitter，按凭据与 `site.json` 启用（`docs/AUTH_GUIDE.md`）。
    *   **内容审核**：链接黑名单 + 可选 LLM 审核（`docs/MODERATION_SPEC.md`）。
*   **权限与安全**：
    *   **OAuth2 集成**：支持 GitHub 登录，集成 PostgreSQL (Sea-ORM) 进行用户同步。
*   **现代前端交互**：
    *   基于 Dioxus 0.7 的信号量 (Signals) 状态管理。
    *   响应式布局与动态资源同步逻辑 (`build.rs`)。

---

## 2. 系统架构 (Architecture)

项目采用典型的 Rust 工作区 (Workspace) 结构，遵循“核心隔离、接口驱动、模块解耦”的原则。

### 2.1 目录结构

```text
├── assets/                 # 静态资源 (posts, docs, courses, podcasts, images)
├── crates/
│   ├── app/                # 前端入口 (Dioxus App + Axum Server 路由)
│   ├── core/               # 核心逻辑 (SeaORM Entities、认证、会话、主题、配置)
│   ├── sdk/                # 业务模块共享类型 (AppModule、审核提交 ModerationSubmission)
│   └── modules/            # 业务领域模块 (blog, podcast, course, forum, moderation ...)
└── docs/                   # 开发者文档
```

### 2.2 数据库层与连接池

后端用 **SeaORM + PostgreSQL**。实体定义在 `crates/core/src/entities/`，schema 由
`crates/migration`（sea-orm-migration）管理，应用启动时自动 `Migrator::up`（失败仅
日志、不退出，便于 schema 已存在的场景）。

**连接池单例**（`crates/core/src/db/pool.rs`）——`DatabaseConnection` 内部已是连接池，
clone 只复制一个 `Arc`，因此全局用一个 `OnceCell` 复用，避免每次请求重建连接 + TLS
握手：

```rust
// 启动时调用一次（main.rs，读 DATABASE_URL）
app_core::db::init_pool(&db_url).await?;

// 任意 server fn 里获取共享连接（已初始化则直接返回 clone）
let db = app_core::db::get_or_init_pool().await?;
```

API：
*   `init_pool(url) -> Result<(), DbErr>`：启动期初始化全局池。
*   `get_or_init_pool() -> Result<DatabaseConnection, DbErr>`：获取共享连接（server fn 首选）。
*   `pool() -> Option<DatabaseConnection>`：非阻塞读取，未初始化返回 `None`。

> 旧的 `db::init_db(url)`（每调用新建连接）仅保留兼容；新代码一律用 `get_or_init_pool()`。
> 本地用 `.env` 提供 `DATABASE_URL`；DB 不可用时仅 DB 相关 server fn 报错，静态/markdown
> 页面（blog、内容板块等）仍可访问。

### 2.3 本地构建产物路径（共享 target-dir）

仓库根 `.cargo/config.toml` 配置了项目全局的 cargo target-dir：

```toml
[build]
target-dir = "/Users/hal/.target"
```

**为什么**：本 workspace 已有 30+ crate × 几百个上游依赖，单独的 `target/` 在
开发态可膨胀到 10–30 GB。把多个 Rust 项目共用一个 target 目录可以：

- 跨项目共享 deps 编译产物（同版本的 `tokio` / `serde` 等只编一次）。
- 把所有项目的临时产物集中在一个盘，方便清理（`du -sh ~/.target/*` → 定向 `cargo clean -p <crate> --target-dir ~/.target`）。
- 让仓库本身保持轻量，避免编辑器索引 / `rg` 误扫进 target。

**fork / 新机器 setup**：自己改 `~/.cargo/config.toml`（用户级别）落地相同配置，
路径换成你本地的（macOS / Linux 任意目录均可，盘要够，目录可后续随时清空重建）：

```toml
[build]
target-dir = "/Users/<your-username>/.target"   # 或 /home/<user>/.target
```

> 也可以**不**做这项设置，cargo 会回落到默认的 `<repo>/target/`，仅占用更多
> 仓库目录空间，不影响功能。

**CI / Docker 部署不能用本地路径**——它们覆盖该配置：

- `.github/workflows/ci.yml`：`env: CARGO_TARGET_DIR: target`（写到 runner 工作目录，便于 cache action 收集）。
- `Dockerfile`：`ENV CARGO_TARGET_DIR=/tmp/target`（写到容器 tmp，构建结束随多阶段镜像丢弃）。

只要 `CARGO_TARGET_DIR` 环境变量被设置，就优先于 `.cargo/config.toml::build.target-dir`，
所以 CI / Docker 不需要改本仓库的 config 文件。

**`dx serve` 下 `/wasm` 整页请求是 404（只在 debug）**：debug 构建把 wasm 产物目录 `public/wasm/`
整个挂在 `/wasm`（为热补丁动态读目录），盖住了 WASM 板块的 `/wasm` 与 `/wasm/:slug`。站内链接走客户端路由，
不受影响；直接打开或刷新才会 404。release 构建把 wasm 产物放进带哈希的 `public/assets/`，没有这个目录，
`/wasm` 正常服务端渲染（2026-10-08 用 `dx build --release` 按 Docker 目录布局验证）。

---

## 3. 扩展站点能力

站点不加载运行时插件：主题、登录方式、审核都是宿主里的普通 Rust 代码，改动随
`cargo test` / clippy 一起受检。2026-10 之前这些能力由 wasmi 加载的 WASM 插件提供，
因全部插件都在本仓库构建、且运行时加载带来的攻击面（见
`docs/SECURITY_REMEDIATION.md`）没有对应收益而移除。

### 3.1 新增主题

在 `crates/core/src/engines/themes/` 放一份 CSS 变量文件，并在
`crates/core/src/engines/theme.rs` 的 `THEMES` 表登记 id / 名称。cookie 与
`site.json::theme` 只接受该表中的 id。详见 `docs/THEME_SPEC.md`。

### 3.2 新增登录方式

在 `crates/core/src/auth/provider.rs` 的 `Provider` 枚举加一个变体，给出
`ProviderSpec`（固定的 https 授权 / token / 用户信息端点、scope、展示信息）和
`map_profile` 字段映射（取不到稳定的用户 id 时返回 `None`，登录失败而不是落到默认 id）。
凭据只走环境变量。详见 `docs/AUTH_GUIDE.md`。

### 3.3 调整内容审核

审核流水线在 `crates/modules/moderation`：`UrlBlocklistStage`（链接黑名单）与
`LlmModerationStage`（提示词 + 结论解析，经 `crates/llm` 调用模型）。新增规则时实现
`AsyncModerationStage` 并在 `ModerationPipeline::from_site_config` 注册。stage 不返回
错误，自己决定失败时的判定；LLM 失败默认送人工复核（`on_llm_failure`）。详见
`docs/MODERATION_SPEC.md`。

### 3.4 server fn 联调日志规范（debug 习惯）

新增 server fn 时**默认**用 `#[tracing::instrument]` 注入入口 / 退出 / 错误日志，
避免每个 fn 手写 `tracing::info!("foo started"); ... tracing::info!("foo done")` 的
重复 + 不一致问题。`tracing-subscriber` 已在 main.rs 启动期初始化（详见
Phase 7.5 / `docs/OPERATIONS.md`）。

#### 模板

```rust
#[cfg_attr(
  feature = "server",
  tracing::instrument(
    name = "server::<fn-name>",          // 显式命名，避免被 SSR 框架展开后的难读 span
    level = "info",                       // 默认 info；细节 fn 用 debug
    skip(secret_arg, large_payload),      // 敏感 / 体积大的参数走 skip
    fields(
      ref_id = %some_id,                  // %  用 Display
      len = some_string.len(),            // 标量直接进 fields
      hits = tracing::field::Empty,       // 占位，后续在函数体内 record
    ),
    err                                   // 自动 Err(...) 时记一条 error
  )
)]
#[post("/api/...")]
pub async fn my_endpoint(
  secret_arg: String,
  ref_id: String,
  some_string: String,
) -> Result<MyResponse, ServerFnError> { /* ... */ }
```

函数体内可以补字段：

```rust
tracing::Span::current().record("hits", hits.len());
```

#### 命名 / 字段约定

- `name = "server::<fn-name>"`：与文件树平行命名，避免被 dioxus_fullstack 的内部
  span 包覆遮掩。
- `level = "info"` 是默认；调试 fn / 高频热路径用 `"debug"`，避免 prod 默认 RUST_LOG=info
  下日志爆炸。
- **PII / 机密一律 `skip`**：OAuth code、JWT、明文密码、邮件正文、评论原文、查询
  `q` 等都走 skip + 单独留 `xxx_len` 字段。
- 资源 id / kind / 长度 / 计数等元数据**直接进 fields**，便于 grep / 聚合。
- `err` 自动把 `Result::Err` 当作 `tracing::error!` 输出（不需要手写 `.map_err`
  里的 log）。

#### DB 查询日志

SeaORM 在 `tracing::log` 桥接下默认按 `debug` level 输出 SQL。联调时：

```bash
# 只看本项目 fn 的 info + SeaORM SQL
RUST_LOG="info,sea_orm=debug,sqlx=debug" dx serve

# 看具体 fn 的 span（按命名前缀 "server::"）
RUST_LOG="info,server::search_query=debug" dx serve
```

生产环境保持 `RUST_LOG=info` 即可，SQL 不会写。

#### 当前已应用的 exemplar

- `module_search::server::search_query`：高 QPS 查询，q skip、kind/limit/q_len/hits 入字段
- `app::server::auth_callback_internal`：复杂多阶段流程（cookie 解密 / token 交换 / DB 写入）
- `module_comments::server::post_comment`：审核 + 权限 + DB 写入的三联

新增 server fn 请照此模板加；改既有 fn 时顺手补上。

## 4. 最佳实践

*   **Tailwind 编译**：修改样式后，在 `crates/app/` 目录下运行 `npm run build`（或 `npm run dev` 开启 watch 模式）。详细流程、主题映射、动态类名处理和 FAQ 请参考 [`docs/TAILWIND_GUIDE.md`](TAILWIND_GUIDE.md)。
*   **后端驱动**：站点级开关与参数放 `assets/site.json`（主题、启用的登录方式、模块开关、审核配置），避免硬编码。

---

希望这份文档能帮助您快速上手！如有疑问，请查阅 `AGENTS.md` 了解更多关于 AI 辅助开发的规范。
