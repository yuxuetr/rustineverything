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
- [x] 访问控制：get_lesson 服务端校验 `free || preview || has_entitlement || admin`，锁定时清空内容 — 提交 `feat(course): access control + paywall`
- [x] Paywall 组件（锁定课节）+ 课程目录 试看/🔒 标记
- [x] Admin 手动授权页 `/admin/entitlements`（列表 + 授予 + 撤销）— 提交 `feat(admin): manual course entitlement grant page`
- M4 完成：付费课程内容模型（已有）+ access_tier/preview + entitlements 表 + get_lesson 服务端鉴权 + Paywall + Admin 授权。线上支付留 M5。

## M5 — 支付集成（微信支付 + 支付宝，国内）
> 设计：[`docs/PAYMENT_SPEC.md`](docs/PAYMENT_SPEC.md)。前置：两网关企业商户号 + 备案 HTTPS 域名。
- [x] **M5a** `orders` 实体 + 迁移；`create_order` / `query_order` server fn（建单 + 状态机；网关 stub）— 提交 `feat(course): orders table + create/query order`
- [x] **M5b** 支付宝接入（page/wap/precreate + `/api/pay/alipay/notify` 验签发货，Axum 原生路由；RSA2 签名/验签单测）— 提交 `feat(pay): Alipay integration`。⚠️ 待沙箱+真实密钥端到端验证
- [x] **M5c** 微信支付 v3 接入（native/h5 + `/api/pay/wechat/notify` 验签+AES-256-GCM 解密；回调用公钥模式）— 提交 `feat(pay): WeChat Pay v3`。⚠️ 待真实商户号端到端验证；平台证书轮换模式可后续扩展
- [x] **M5d** PurchaseModal（选网关 + 跳转/二维码 + 手动刷新解锁）接到 Paywall — 提交 `feat(pay): PurchaseModal`。happy path 待登录+网关凭据
- [x] **M5e** 我的订单页（/me/orders）+ 课程详情购买入口（含已拥有判断）+ 用户菜单入口 — 提交 `feat(pay): my-orders page + course-detail buy entry`
- [ ] **M5e 余项（需真实网关）** 对账定时任务（gateway query 回填/关单）+ 退款（gateway refund + admin）
- 约定：验签是发货前提；金额核验；以 out_trade_no 幂等；回调可重入；密钥经 .env 校验不回显。
- M5 核心完成（M5a–M5d）：双网关下单 + 回调发货 + 购买 UI 全链路打通（服务端单测覆盖签名/验签/解密）。⚠️ 上线需真实商户号 + 公网 HTTPS 回调端到端验证。

## M6 — Pro 订阅会员 ✅
- [x] **M6a** memberships 表 + 实体 + 迁移；is_pro_member；纯函数 can_access_lesson（pro 课程允许有效会员）接入 get_lesson；单测 — 提交 `feat(course): Pro membership model + access control`
- [x] **M6b** my_membership + admin 授予/续期/撤销/列表 + Admin「Pro 会员」区块 + 我的订单页会员横幅 — 提交 `feat(pay): Pro membership management + display`
- [ ] 余项（需网关/订阅模型）：会员**自助购买**（订阅下单，扩展 order 产品类型）+ 自动续费
- 当前可用：运营在 /admin/entitlements 手动开通 Pro 会员（解锁全部 pro 课程）。

### 依赖与排序
- M1/M2 不依赖付费，先上线见效；M3 为 M2 的筛选/精选提供数据；M4 起才动 DB 与鉴权；M5 依赖 M4。
- 顺序：M1 → M2 →（M3 穿插）→ M4 → M5 →（M6 可选）。

---

# 新阶段 — 架构风险治理（2026-07-21 评估落地）

> 来源：2026-07-21 全面架构评估（架构分层 / 可扩展性 / 安全性 / 性能）。
> 原则：增量治理，不推倒现有架构；每任务一提交（无共同作者行）；提交后同步本文档。
> 校验命令沿用「持续约定」章节；涉及 DB 的任务先确认迁移向后兼容（不破坏现有登录用户与已加密 token）。

## 任务清单（按优先级）

### S1 — 安全响应头中间件（风险 R6）✅
- [x] 新增 `crates/app/src/server/security.rs`：`security_headers_mw` 挂在 router 最外层（`main.rs:551`），注入 CSP（保守策略，兼容内联 style/script + WASM + YouTube/Bilibili 嵌入）、`X-Content-Type-Options: nosniff`、`Referrer-Policy: strict-origin-when-cross-origin`、`X-Frame-Options: DENY`；已存在同名头不覆盖
- [x] 运维开关：`CSP_POLICY` 覆盖/置空禁发；`SECURITY_HEADERS_DISABLED=1` 整体禁用；nonce 化方向写入模块注释
- [x] 4 个单测通过（指令存在性 / 无 CSP 基线 / 非法值不 panic）；server + 默认 web 双目标编译通过

### S2 — 应用层限流中间件（风险 R4）✅
- [x] 新增 `crates/app/src/server/rate_limit.rs`：手写 token-bucket per-IP 限流（无新依赖），仅作用于 `/api/*`；`/api/auth/*`、`/api/pay/*` 用更严 sensitive 桶（5 rps/15），其余 20 rps/60；key 取 XFF 首 IP → x-real-ip → global 共享桶
- [x] 容量防御：桶表上限 50k + 10min 过期剪枝 + overflow 折叠桶（防伪造海量 IP 内存放大）；超限 429 + Retry-After
- [x] env 可调：`RATE_LIMIT_{API,SENSITIVE}_{RPS,BURST}`、`RATE_LIMIT_DISABLED=1`；7 个单测通过（burst/回填/隔离/分类/key 提取/非法配置 clamp）

### S3 — 迁移失败降级/严格模式 + 健康检查（风险 R3）✅
- [x] 新增 `crates/app/src/server/health.rs`：启动期记录 `StartupHealth`（db_configured/db_connected/migrations）；`/healthz` 返回 `200 ok` / `503 degraded`（JSON，no-store）；未配置 DATABASE_URL 的纯静态站不算降级
- [x] `STRICT_MIGRATION=1` 时 DB 连接失败 / 迁移失败直接 panic 拒绝启动（生产推荐）；默认保持可用性优先但 /healthz 可观测（`main.rs:161-225`）
- [x] 2 个单测通过（降级矩阵 / 快照语义）

### S4 — JWT 撤销基础 token_version（风险 R1）✅
- [x] 迁移 `m20260721_000008_users_token_version`：`users.token_version int not null default 0`；实体同步加字段（serde default，存量用户/旧 JSON 兼容）
- [x] JWT claims 携带 `tv`（serde default，旧 token → 0 与 DB 默认 0 匹配，存量登录不受影响）；`create_jwt`/`verify_jwt` 贯通；`SessionUser.token_version` 新增（向后兼容单测覆盖）
- [x] 新增 `require_session_verified()`（`session.rs:197`，fail-closed）；写路径接入：forum create_topic/post_reply、comments post_comment、uploads upload_image；`require_admin` 复用同次 DB 查询加版本比对
- [x] `admin_set_user_role` 角色变更时 bump token_version（同角色重复提交不 bump）；全工作区测试 0 失败
- 备注：course 模块的进度写入仍用轻量 `current_session_user`（低风险，后续可按需升级）；登出仍为清 cookie，全局吊销需 bump 版本

### S5 — 独立数据加密密钥 + key-id 密文格式（风险 R2）✅
- [x] `crypto.rs` 重构：`DATA_ENCRYPTION_KEY` 优先（域 tag `data-encryption-v2` 派生），缺省回退 JWT_SECRET 派生并 warn（仅首次）；启动期 + 加密路径均做 placeholder 校验；`.env.example` 补说明
- [x] 新密文格式 `v2:<base64url>`（key-id 前缀，为 v3/多密钥渐进轮换预留）；解密按前缀选密钥，无前缀回退 v1（JWT_SECRET 派生）兼容在途 cookie；新密文不再产出 v1
- [x] 7 个 crypto 单测（含 v1 兼容 / 独立密钥轮换语义）+ 11 个 PKCE cookie 测试全部通过
- 备注：当前 crypto 唯一调用点是短生命周期 PKCE cookie（无长期存量密文），v1 回退路径可在下个版本安全移除

### S6 — 支付回调专项加固审计（风险 R7）✅
- [x] 审计结论：验签/金额核验/幂等快路径已具备；发现 3 个可加固点并全部修复（`course/src/server.rs` notify 两处）
- [x] 原子认领：条件 `UPDATE … WHERE out_trade_no=? AND status!='paid'` 取代「读-判-写」，rows_affected=0 视为已处理——消除并发回调双发货竞态（alipay + wechat）
- [x] alipay：新增 `app_id` 比对（防跨商户应用合法签名串单）；wechat：时间戳 ±5min 新鲜度（缩小重放窗口）+ 解密后 appid/mchid 交叉校验
- [x] 入口审计日志 `target=pay_audit`（关键字段留痕，不含买家敏感信息）；module-course 45 测试全部通过

### S7 — 拆分 app/main.rs router 组装（风险 R8）✅
- [x] 新增 4 个 server 子模块：`seo.rs`（sitemap/feed/robots，`collect_content_entries` 统一条目收集，`collect_board!` 宏只剩一份）、`auth_routes.rs`、`pay_routes.rs`、`static_assets.rs`，各提供 `mount(router, …)` 函数
- [x] `main.rs` 从 737 行减到 ~370 行，router 组装段只剩 9 行引导；新增内容板块只需改 seo.rs 一处，消除 sitemap/feed 漏改不一致风险
- [x] 行为不变：server 构建 + 默认 web check 双目标通过，app 13 测试全过

### S8 — 主题 CSS 防护强化（风险 R5）✅
- [x] 新增 `normalize_css_for_scan`（`plugin_security.rs`）：扫描前解码 CSS hex/字面转义 + 去注释 + 去全部空白 + 小写——封堵 `\75 rl(`、`url( http://`、`url(/**/http://`、`@\69mport` 等混淆绕过（@import/expression 本身已在旧黑名单中）
- [x] 黑名单补 `-moz-binding:`、`url('//`、`url(\"//`；修正 lib.rs / 模块注释的 "allowlist" 误导措辞（明确为 blacklist + fail-closed，白名单解析器列为后续升级方向）
- [x] 新增 8 个测试（7 混淆绕过 + 1 合法 CSS 不误拒），css_sanitize 共 19 测试全部通过

### S9 — 生产路径 unwrap/expect 收敛（风险 R12）
- [ ] 清理 app/core/migration 生产路径 panic 点；workspace 启用 `clippy::unwrap_used`/`expect_used` lint（测试豁免）

### S10 — site.json 读取缓存（风险 R11）
- [ ] `core::settings` 提供 mtime 缓存的统一读取入口，替换 main.rs / feed 等直读点

### 依赖与排序
- S1/S2/S3 为低冲突基础设施，先行；S4/S5 触及 DB 与密钥，居中单独提交；S6 审计后按需改动；S7 结构重构放在安全项之后避免冲突；S8–S10 收尾。
- 顺序：S1 → S2 → S3 → S4 → S5 → S6 → S7 → S8 → S9 → S10。
