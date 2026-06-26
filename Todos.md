# 开发计划 — 重构冲刺：业务模块解耦 + SSR Hydration 优化

> 上一阶段（Phase 8 安全 & 性能硬化 + Phase 9）归档在 [`Todos.refactor-backup.md`](Todos.refactor-backup.md)。
> 本文档承接 2026-06-26 整体架构评估的两项结论，聚焦 **(A) 业务模块编译期解耦** 与 **(B) SSR hydration 优化**。

## 本阶段目标

1. **模块解耦**：内容模块只依赖 `app-core` / `sdk` / `widgets`；跨模块 UI 组合上提到组合根 `app`；数据源依赖显式化。
2. **SSR hydration**：把 SEO/首屏关键的内容承载页从 `use_resource` 迁移到 `use_server_future`，消除首屏 spinner、二次抓取与 hydration 不匹配。

**不在本阶段范围**（留作后续独立计划）：
- 5 个内容板块 crate（`modules/{ai,cli,embedded,wasm,web3}`）合并去重
- 插件 ABI v2（Extism / wit-bindgen）
- 桌面 / 移动端迁移

---

## 持续约定

- **每任务一提交**：每完成一个任务，`cargo fmt` + 构建 + 测试 + `clippy -D warnings` 通过后提交一次。
- **提交信息不带共同作者行**（不加 `Co-Authored-By`）。
- **提交后同步本文档**：勾选完成项 + 补实际落点（文件:行号 / 测试名），再开下一个任务。
- **构建命令**统一带 `CARGO_TARGET_DIR=/Users/hal/.target`；新代码不使用 `unwrap` / `expect`。
- **数据库（按需）**：需要 DB 时用 macOS `apple container` 起 `postgres:16`，环境变量从 `.env` 读取（不回显密钥）。内容页 SSR 验证只读 `assets/`，不需要 DB。

### 校验命令

- 格式化：`cargo fmt`
- 构建：`CARGO_TARGET_DIR=/Users/hal/.target cargo build -p app --features server`
- 测试：`CARGO_TARGET_DIR=/Users/hal/.target cargo test --features server --workspace -- --test-threads=1`
- Lint：`CARGO_TARGET_DIR=/Users/hal/.target cargo clippy --workspace --features server --all-targets -- -D warnings`
- SSR 烟测（B 类）：`dx serve --package app` 后 `curl -s http://127.0.0.1:8080/<路由>`，断言首屏 HTML 含正文/列表文本而非 loading spinner。

---

## 前置任务

### P0 — 备份并重建 Todos.md
- [x] 备份当前 `Todos.md` → `Todos.refactor-backup.md`
- [x] 按本计划重写 `Todos.md`（A/B 全部任务 + 持续约定）
- [x] 提交（无共同作者行）

### P1 —（按需）apple container 起 PostgreSQL
- [ ] 首个需要 DB 的任务前：从 `.env` 读取 `POSTGRES_USER/PASSWORD/DB`，`container run` 起 `postgres:16`，映射 `127.0.0.1:5432`，等健康后用 `DATABASE_URL` 验证连通

---

## 工作流 A：业务模块解耦

### A1 — 移除 forum 的死依赖（blog / course）✅
- [x] 确认 `forum` 全树无 `module_blog::` / `module_course::` 使用（grep 0 命中）
- [x] 从 `crates/modules/forum/Cargo.toml` 删除 `module-blog` / `module-course` 依赖及 `server` feature 中的 `module-blog/server` / `module-course/server`
- [x] 保留 `module-moderation`（可选）；`module-forum` + `app` 全量构建通过，forum 21 测试通过

### A2 — 上提 docs 的跨模块 UI 组合到 app 层 ✅
- [x] `DocPage` 改造为接受 `footer: Element` 插槽（`docs.rs:153`），组件本身不再 `use module_course` / `use module_forum`
- [x] `app/src/routes/mod.rs` 的 `DocPage` 包装组件把 `AnnotationLayer` + `DiscussionPanel` 作为 `footer` 插槽传入（`routes/mod.rs:196`）
- [x] 从 `crates/modules/docs/Cargo.toml` 删除 `module-blog`（同为死依赖）/ `module-course` / `module-forum` 依赖
- [x] app（server）+ docs（server/client）构建通过，clippy `-D warnings` 零告警；标注层/讨论面板仍在 docs 文章页身位渲染

### A3 — 解耦 search → cases 数据源 ✅（选择：IoC 倒置）
- [x] 新增 `app-core::engines::doc_source`：中立 `ExternalIndexedDoc` + `register_doc_source` / `collect_registered_docs` 注册表（同 `ComponentRegistry` IoC 模式）
- [x] `indexer.rs` 的 `collect_cases` → `collect_registered_external`，改读 core 注册表；从 `module-search/Cargo.toml` 删除 `module-cases` 依赖及 `module-cases/server`
- [x] `app/src/main.rs` 启动期注册 cases 来源（server-only）；core 155 + search 64 测试通过，clippy `-D warnings` 零告警

### A4 — 固化模块依赖策略文档
- [ ] 在 `docs/MODULE_SPEC.md` 增补「模块依赖规则」+ 当前合规边与例外

---

## 工作流 B：SSR Hydration 优化

> 技术前提：server fn 返回类型已 `Serialize + Deserialize`；`use_server_future` 需置于 `SuspenseBoundary` 内、闭包捕获其依赖的响应式值；错误分支 fail-safe 不 panic。
> 非 SEO / 强交互 / 鉴权页（forum 互动、admin、登录态）保持 `use_resource` 并注释理由。

### B1 — 建立模式与参照迁移（blog 详情页）
- [ ] `BlogInner` 引入 `use_server_future` + `SuspenseBoundary`
- [ ] `curl /blog/<slug>` 断言首屏 HTML 含正文；沉淀「迁移配方」

### B2 — 迁移 blog 列表页
- [ ] `BlogIndexInner` 的 `list_blog_posts` → `use_server_future`，保留标签筛选/分页客户端交互
- [ ] 烟测 `/blog`

### B3 — 迁移 5 个内容板块页
- [ ] `ai/web3/wasm/cli/embedded` 的 `*IndexPage`（`list_*_articles`）与 `*ArticlePage`（`get_*_article`）→ `use_server_future`
- [ ] 烟测各板块首屏

### B4 — 迁移 course 与 docs 页（须在 A2 之后）
- [ ] `course.rs` 列表/详情/课时 + `docs.rs` 的 `list_doc_tree` / `get_doc_content` → `use_server_future`
- [ ] 课时页若涉及 DB 按 P1 起 DB 验证；烟测 `/course`、`/docs/<path>`

### B5 — 迁移 cases 页
- [ ] `cases.rs` 的 `*IndexPage` / `*DetailPage` → `use_server_future`
- [ ] 烟测 `/case`、`/case/<slug>`

### B6 — 评估 App 级与非内容页
- [ ] 评估 `get_aggregated_theme_css`（可受益 server-future）与 `get_current_user`（宜保持客户端）
- [ ] forum/admin 等显式保留 `use_resource` 并注释理由

---

## 依赖与排序

- P0 最先；P1 在首个需要 DB 的任务前按需执行。
- **A2 必须在 B4 之前**（docs 插槽改造影响其 SSR 迁移）。
- 建议顺序：P0 → A1 → A2 → A3 → A4 → B1 → B2 → B3 → B4 →（按需 P1）→ B5 → B6。
