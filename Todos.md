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

### P1 —（按需）apple container 起 PostgreSQL ✅
- [x] 用 `apple container` 起 `postgres:16-alpine` 容器 `rie-postgres`，凭据从 `.env` 的 `DATABASE_URL` 解析（user=postgres / db=github-auth），发布端口 `127.0.0.1:5432`；psql TCP 密码鉴权通过，`dx serve` 启动日志 `schema migrations applied`（9 表）

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

### A4 — 固化模块依赖策略文档 ✅
- [x] `docs/MODULE_SPEC.md` 新增 §11「模块依赖规则」：允许的依赖方向 / 实现手法（插槽 + IoC）/ 当前合规边与例外（moderation）/ 新增交互决策树

---

## 工作流 B：SSR Hydration 优化

> 技术前提：server fn 返回类型已 `Serialize + Deserialize`；`use_server_future` 需置于 `SuspenseBoundary` 内、闭包捕获其依赖的响应式值；错误分支 fail-safe 不 panic。
> 非 SEO / 强交互 / 鉴权页（forum 互动、admin、登录态）保持 `use_resource` 并注释理由。

### B1 — 建立模式与参照迁移（blog 详情页）✅
- [x] `BlogInner` 拆为布局外壳 + `BlogArticle`（`routes/mod.rs:449`）；`BlogArticle` 用 `use_server_future` + `use_reactive!(|id| ...)`，置于 `SuspenseBoundary`（fallback=spinner）内
- [x] base_url 也改 `use_server_future`；错误/空分支 fail-safe（`unwrap_or_default`，无 `unwrap()/expect()`）
- [x] 双编译目标验证：app `--features server`（SSR）构建通过 + `cargo check -p app`（默认 web cfg）通过；clippy `-D warnings` 零告警
- [x] 【运行时已验证 2026-06-26】`dx serve` + `curl -s /blog/welcome` → http=200、含正文、`spinnerOnly=0`

> 迁移配方（后续 B 任务复用）：把取数部分抽成子组件，子组件内 `let x = use_server_future(use_reactive!(|dep| async move { server_fn(dep).await }))?;`（无参数用 `|| async move {...}`），父组件用 `SuspenseBoundary { fallback: |_| rsx!{...}, Child {} }` 包裹；读取 `res()` 得 `Option<T>`，`match` 处理 Some(Ok/Err)/None。

### B2 — 迁移 blog 列表页 ✅
- [x] `BlogIndexInner` 拆为标题外壳 + `SuspenseBoundary{ BlogList }`；`BlogList`（`routes/mod.rs:252`）用 `use_server_future` 取 `list_blog_posts`，标签筛选/分页保留为客户端 signal
- [x] 双编译目标（server + 默认 web）通过，clippy `-D warnings` 零告警
- [x] 【运行时已验证 2026-06-26】`curl -s /blog` → http=200、含文章列表、`spinnerOnly=0`

### B3 — 迁移 5 个内容板块页 ✅
- [x] `ai/web3/wasm/cli/embedded` 统一重写：`*IndexPage` 拆为标题外壳 + `SuspenseBoundary{ *IndexList }`（`use_server_future` 取 `list_*_articles`，筛选/搜索保留客户端 signal）；`*ArticlePage` 拆为外壳 + `*ArticleContent`（`use_server_future` + `use_reactive!(|slug|)` 取 `get_*_article`）
- [x] app `--features server` 构建 + 默认 web check 通过；clippy `-D warnings` 零告警（5 板块 + app）
- [x] 【运行时已验证 2026-06-26】`curl -s /ai` → http=200、含内容、`spinnerOnly=0`；web3/wasm/cli/embedded 为同一模板生成，待服务器再次运行时补 curl

### B4 — 迁移 course 与 docs 页（须在 A2 之后）✅
- [x] `course.rs`：`CoursesIndexPage`→`CoursesList`、`CourseDetailPage`→`CourseDetailLoaded`（get_course）、`LessonPage`→`LessonLoaded`（get_lesson）均走 `use_server_future`；进度/上次学习/标注层等每用户 DB交互保留 use_resource
- [x] `docs.rs`：`Docs`→`DocsIndexInner`（list_doc_tree）、`DocPage`→`DocPageInner`（get_doc_content 走 server_future；树形导航保留 use_resource）
- [x] 本任务不需 DB（内容均读磁盘）；app `--features server` + 默认 web check 通过，clippy `-D warnings` 零告警
- [x] 【运行时已验证 2026-06-26】`curl -s /course`、`/docs` → http=200、含内容、`spinnerOnly=0`

### B5 — 迁移 cases 页 ✅
- [x] `CaseDetailPage`→`CaseDetailLoaded`（get_case 走 `use_server_future`）；`CasesIndexPage` 结果网格抽为 `CasesGrid`（`use_server_future` + `use_reactive!(|query,category,tag|)`，筛选 signal 作 prop），搜索/标签/分类 UI chrome 保留客户端
- [x] app `--features server` + 默认 web check 通过，clippy `-D warnings` 零告警
- [x] 【运行时已验证 2026-06-26】`curl -s /case` → http=200、含列表、`spinnerOnly=0`

### B6 — 评估 App 级与非内容页 ✅
- [x] 结论：`get_aggregated_theme_css` 与 `get_current_user` 均保留 `use_resource`（App 根不宜整体挂起；登录态不宜烘进可缓存 HTML），已在 `main.rs` 加详细理由注释
- [x] forum (`TopicsIndexPage`) / admin (`AdminDashboardPage`) 加保留 `use_resource` 的理由注释（强交互/鉴权、非 SEO）
- [x] app `--features server` 构建 + clippy `-D warnings` 零告警；全工作区 `cargo test --features server --workspace` 全部通过（0 failed）

---

## 验收状态（2026-06-26）
所有任务（P0 / A1–A4 / B1–B6）已完成并逐个提交（无共同作者行）。
- 模块解耦：forum 不再依赖 blog/course；docs 不再依赖 course/forum/blog；search 经 core IoC 注册表解耦 cases；策略写入 `MODULE_SPEC.md` §11。
- SSR：blog（列表+详情）、5 内容板块、course（列表/详情/课时）、docs（首页+详情）、cases（列表+详情）均迁到 `use_server_future` + `SuspenseBoundary`；鉴权/强交互页保留 use_resource 并注释理由。
- 验证：双编译目标（server + 默认 web）构建、clippy `-D warnings` 零告警、全量测试 0 failed。

## 运行时验证补充（2026-06-27）
- DB 环境：`apple container` 起 `rie-postgres`（凭据解自 `.env::DATABASE_URL`），`dx serve` 启动迁移成功（9 表）。
- SSR 首屏：`/blog`、`/blog/welcome`、`/ai`、`/case`、`/docs`、`/course` 均 http=200、含正文、`spinnerOnly=0`。
- DB 功能（评论/标注读取路径）：插入样本后 `POST /api/comments/list`、`/api/annotations/list`、`/api/annotations/config` 均返回 200 + 正确数据（含 users 关联解析的作者名）。
- 残留（可选，需重启 dx serve）：web3/wasm/cli/embedded 单独 curl；浏览器 Console 确认无 hydration mismatch 警告。

---

## 依赖与排序

- P0 最先；P1 在首个需要 DB 的任务前按需执行。
- **A2 必须在 B4 之前**（docs 插槽改造影响其 SSR 迁移）。
- 建议顺序：P0 → A1 → A2 → A3 → A4 → B1 → B2 → B3 → B4 →（按需 P1）→ B5 → B6。

---

# 新阶段 — 站点重设计：双生态首页 + 导航 + 付费课程

> 设计文档：[`docs/SITE_REDESIGN_SPEC.md`](docs/SITE_REDESIGN_SPEC.md)。
> 定位：Rust 工业用途社区，围绕 **Rust 生态 + AI 生态** 两大支柱；案例为差异化核心，课程为变现核心。
> 已完成（前置 UI 修复）：导航断点回退 lg + 首页模块卡网格（提交 `fix(ui): restore desktop nav...`）。

## M1 — 导航重构：双生态 mega 菜单 ✅
- [x] taxonomy 单一源 `crates/app/src/taxonomy.rs`（rust={embedded,web3,wasm,cli}，ai={llm,inference,agent,rust-ai}；backend 与 AI 子领域筛选留 M3）
- [x] `classic.rs` 顶层改为 `Rust 生态▾  AI 生态▾  案例  课程  博客  论坛`；播客并入移动抽屉；保留右侧控件；新增 on_course gating
- [x] `EcosystemMenu` 组件（`components/ecosystem_menu.rs`）：三栏（应用领域 / 学习资源 / 生态简介+精选案例 CTA），纯 CSS group-hover + group-focus-within 展开
- [x] 响应式：<lg hamburger 抽屉内按生态分组（标题+领域）+ 内容入口；桌面/移动均浏览器验证（M1 用分组列表，accordion 折叠 + Esc 关闭留作打磨）
- [x] i18n 新键 `nav.eco.*` / `mega.*` / `nav.ai.*` / `nav.course/web3/wasm/cli`（zh/en parity 通过）；重建 minified CSS（root==crate）；clippy 新代码零告警
- 备注：精选案例列 M1 用静态 CTA，M2/M3 接 `cases.favorite` 实时数据。

## M2 — 首页重排
- [x] Hero 文案/CTA 更新（查看案例 + 查看课程 + 进入文档）— 提交 `feat(home): hero CTAs + dual-ecosystem pillars`
- [x] `EcosystemPillars`（Rust 生态 | AI 生态 两张大卡 + 子领域 chips，链接领域路由）
- [x] 现有 11 卡模块网格下移为「按领域浏览」（home.browse.*）
- [x] `FeaturedCases`（`list_cases` + `favorite` 取 6，CaseCard 含封面/分类徽章/stars/描述）— 提交 `feat(home): featured cases + course showcase`
- [x] `CourseShowcase`/`CourseCard`（cover/级别/课时数；资源徽章 🎬📄🎧💻 + 价格/层级留 M4）
- [x] `CommunityFeed`（最新博客 use_server_future + 论坛热帖 use_resource，2 列）— 提交 `feat(home): community feed + richer footer`
- [x] footer 加厚（品牌简介 + 内容 / 社区 分栏 + 底部版权条）
- M2 完成：首页 Hero → 两大生态 → 精选案例 → 课程 → 社区动态 → 按领域浏览 → 加厚 footer，全部浏览器验证。

## M3 — 分类法统一（部分完成）
- [x] `cases.category` → 生态派生映射（taxonomy::ecosystem_of_case_category，单一来源）— 提交 `feat(taxonomy): ecosystem landing pages`
- [x] 生态落地页 `/ecosystem/:id`（生态简介 + 领域入口 + 该生态精选案例过滤）；pillars 加「进入生态」入口
- [x] 领域 board（/embedded /ai /web3 /wasm /cli）保留为领域落地页（M1 起即由导航/pillars 指向）
- [ ] 延后（内容相关）：`ecosystem/domain` 标签贯通 docs/blog/course；`/ai` 子领域 tag 化筛选（需先给 ai 文章打 llm/inference/agent/rust-ai 标签）

## M4 — 课程付费地基（不接网关也能线下售卖）
- [x] `course.yaml` 加 `access_tier`(free|paid|pro)/`price`/`currency`；Lesson frontmatter 加 `preview`（首页 CourseCard 显价格徽章；rust-basics 示例 paid）— 提交 `feat(course): access tier + preview metadata`
- [x] Entitlement 表（SeaORM 实体 + 迁移）+ server fns（mine/list/grant/revoke）— 提交 `feat(course): entitlements table + server fns`
- [ ] 访问控制：`可看 = free || preview || has_entitlement`，server fn（get_lesson）二次校验
- [ ] Paywall 组件 + 锁定课节覆盖层 + 侧栏锁图标
- [ ] Admin 手动授权页（[ADMIN_SPEC](docs/ADMIN_SPEC.md)）

## M5 — 支付集成
- [ ] 接支付网关（国内 微信/支付宝；海外 Stripe/Paddle/LemonSqueezy）+ webhook→entitlement
- [ ] 订单/支付记录表；幂等与回调校验

## M6 —（可选）Pro 订阅会员
- [ ] 会员层级 + 订阅状态 + 看全部 pro 课程的权益解析

### 依赖与排序
- M1/M2 不依赖付费，先上线见效；M3 为 M2 的筛选/精选提供数据；M4 起才动 DB 与鉴权；M5 依赖 M4。
- 顺序：M1 → M2 →（M3 穿插）→ M4 → M5 →（M6 可选）。
