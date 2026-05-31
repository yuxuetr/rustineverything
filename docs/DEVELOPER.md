# Rust in Everything 开发者文档

欢迎来到 **Rust in Everything** 项目！本系统是一个基于 Dioxus 0.7 深度定制的高性能、插件化全栈 Web 应用。本文档旨在帮助开发者理解项目架构、现有功能，并掌握如何开发新插件与集成新功能。

---

## 1. 项目核心功能 (Current Features)

*   **全栈内容管理**：
    *   **Blog 模块**：支持 MDX 语法，具备 Frontmatter 解析、LaTeX 数学公式、代码高亮（Prism.js）、GFM Alert (Admonitions) 支持。
    *   **Podcast 模块**：内置音频播放系统，支持列表切换与元数据管理。
    *   **Doc 模块**：以目录为单位的三级文档树，支持 frontmatter SEO、`sidebar_position` / `sort_children` 控制侧栏。
    *   **Course 模块**： `Course → Chapter → Lesson` 三级模型，适配 Doc / Video / Audio / Code 四种课节布局，含进度与标注系统。
    *   **Forum 模块**：话题 / 回复 / Tag；支持从博客、文档、课节页面直接发起讨论并建立资源引用关联（详见 `docs/FORUM_SPEC.md`）。
*   **WASM 插件系统**：
    *   **动态主题**：通过 WASM 插件实时注入 CSS 变量，支持多主题切换。
    *   **多语言 (i18n)**：基于 Fluent 引擎的 WASM 插件，实现后端驱动的动态翻译。
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
├── assets/                 # 静态资源 (posts, docs, courses, podcasts, images, plugins)
├── crates/
│   ├── app/                # 前端入口 (Dioxus App + Axum Server 路由)
│   ├── core/               # 核心逻辑 (插件加载器、SeaORM Entities、认证、会话)
│   ├── sdk/                # 共享接口 (Plugin & AppModule Trait 定义)
│   ├── modules/            # 业务领域模块 (blog, podcast, course, forum)
│   └── plugins/            # WASM 插件源码 (i18n, theme, *-auth)
└── docs/                   # 开发者文档
```

### 2.2 关键接口：WASM 插件协议

插件系统基于 `wasmi` 运行时，通过 `crates/sdk` 定义的 FFI 内存分配协议进行通信：

*   **alloc/dealloc**：用于在 WASM 线性内存中手动管理字符串空间。
*   **Packed Result**：函数返回 `u64`，高 32 位为指针，低 32 位为长度，实现零拷贝数据传递。

### 2.3 数据库层与连接池

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

### 2.4 本地构建产物路径（共享 target-dir）

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

---

## 3. 插件开发篇 (Plugin Development Guide)

插件是本系统的核心扩展点。所有插件均编译为 WASM 模块，运行在宿主环境的沙箱中。

### 3.1 核心规范：FFI 内存协议 (The Contract)

由于 WASM 沙箱无法直接理解 Rust 的 `String` 或 `Vec<u8>`，插件必须遵循一套底层的内存管理协议来与宿主通信。

#### 必须导出的底层函数
每个插件必须导出以下三个 C 兼容接口（推荐直接使用 `sdk` 提供的默认实现）：

1.  **`alloc(size: usize) -> *mut u8`**
    *   **作用**：由宿主调用，在插件内存中为输入数据预留空间。
2.  **`dealloc(ptr: *mut u8, size: usize)`**
    *   **作用**：由宿主调用，释放不再使用的插件内存，防止内存泄漏。
3.  **业务处理函数 (Entry Point)**
    *   **签名**：`fn func_name(ptr: *mut u8, len: usize) -> u64`
    *   **返回值规范**：返回一个 `u64` 的“打包结果”（Packed Result）。
        *   **高 32 位**：结果字符串在插件内存中的指针地址。
        *   **低 32 位**：结果字符串的字节长度。

---

### 3.2 业务接口规范 (Logical Interfaces)

根据插件用途的不同，需要实现特定的导出函数名和输入输出逻辑。

#### A. 主题插件 (Theme Plugin)
*   **导出函数**：`get_theme_css`
*   **输入**：目前为空（未来可扩展为传入当前配置 JSON）。
*   **输出**：返回合法的 CSS 字符串，通常包含 `:root` 变量定义。
*   **实现要点**：确保包含 `.dark` 选择器的适配。

#### B. 多语言插件 (i18n Plugin)
*   **导出函数**：`translate`
*   **输入**：JSON 字符串，格式为 `{"key": "翻译键", "lang": "语言代码"}`。
*   **输出**：翻译后的纯文本。
*   **实现要点**：建议集成 `fluent-bundle` 提高翻译灵活性。

---

### 3.3 开发流程与代码模板

#### 第一步：配置 Cargo.toml
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
sdk = { path = "../../sdk" }
serde_json = "1.0"
```

#### 第二步：实现逻辑 (src/lib.rs)
```rust
use sdk::{alloc, dealloc};
use std::slice;

#[no_mangle]
pub unsafe extern "C" fn your_custom_function(ptr: *mut u8, len: usize) -> u64 {
    // 1. 获取并解析宿主传入的字符串
    let input_bytes = slice::from_raw_parts(ptr, len);
    let input_str = std::str::from_utf8(input_bytes).unwrap_or("");

    // 2. 执行你的业务逻辑
    let result_str = format!("Hello, {}! This is from WASM.", input_str);

    // 3. 将结果写回插件内存并封包返回
    let res_bytes = result_str.into_bytes();
    let res_len = res_bytes.len();
    let res_ptr = alloc(res_len);
    slice::from_raw_parts_mut(res_ptr, res_len).copy_from_slice(&res_bytes);

    ((res_ptr as u64) << 32) | (res_len as u64)
}
```

---

### 3.4 插件开发 Check-list

在发布插件前，请检查以下事项：
- [ ] **编译目标**：是否使用了 `--target wasm32-unknown-unknown`？
- [ ] **内存安全**：是否所有通过 `alloc` 分配的结果内存最终都能通过宿主的 `dealloc` 调用被释放？
- [ ] **无 IO 限制**：插件是否避免了直接的文件系统读写或网络请求（这些应由宿主完成并通过参数传入）？
- [ ] **性能优化**：对于主题插件，CSS 是否进行了压缩？

---

### 4.4 多平台 OAuth 适配器集成 (OAuth Adapter Integration)

对于需要支持 **GitHub, Google, Wechat, QQ, Feishu** 等多种登录方式的场景，系统采用“宿主驱动 + 插件适配”的混合模式。

#### 为什么使用插件处理 OAuth？
1.  **字段映射解耦**：GitHub 使用 `login` 字段，Google 使用 `name` 字段，通过插件统一映射为 `nickname`。
2.  **动态准入规则**：可以在不重启服务器的情况下，通过更新 WASM 插件来调整特定平台的登录门槛（例如仅允许特定域名的 Google 账号登录）。
3.  **零停机热扩容**：新增登录平台只需上传新的 `.wasm` 适配器。

#### OAuth 适配器接口规范 (Auth Adapter Specification)
开发者需要实现以下导出函数：

1.  **`get_provider_metadata`** (无输入 -> JSON)
    *   **输出**：包含 `auth_url`, `token_url`, `default_scopes` 的 JSON。
2.  **`transform_profile`** (Raw Profile JSON -> Standard User JSON)
    *   **输入**：第三方平台返回的原始 User Profile 字符串。
    *   **输出**：标准化用户对象：
        ```json
        {
          "external_id": "12345",
          "nickname": "RustDev",
          "avatar_url": "https://...",
          "email": "dev@rust.app",
          "raw_data_summary": "..."
        }
        ```

#### 集成 Check-list
- [ ] **环境隔离**：Secret 和 ClientID 仍由宿主环境变量持有，插件仅负责“逻辑转换”。
- [ ] **错误处理**：对于非法的 Profile 结构，插件应返回清晰的错误 JSON。
- [ ] **状态校验**：如 Wechat 登录涉及的 UnionID 处理，应在插件逻辑中明确。

---

## 5. 最佳实践：插件化建议


*   **调试插件**：可以利用 `crates/core/src/lib.rs` 中的测试用例进行 WASM 插件的单元测试。
*   **Tailwind 编译**：修改样式后，在 `crates/app/` 目录下运行 `npm run build`（或 `npm run dev` 开启 watch 模式）。详细流程、主题映射、动态类名处理和 FAQ 请参考 [`docs/TAILWIND_GUIDE.md`](TAILWIND_GUIDE.md)。
*   **后端驱动**：尽量通过 `assets/site.json` 配置应用行为，避免硬编码，以便通过插件系统进行扩展。

---

希望这份文档能帮助您快速上手！如有疑问，请查阅 `AGENTS.md` 了解更多关于 AI 辅助开发的规范。
