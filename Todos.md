# 开发计划 — 重构冲刺：业务模块解耦 + SSR Hydration 优化

> 上一阶段（Phase 8 安全 & 性能硬化 + Phase 9）归档在 [`Todos.refactor-backup.md`](docs/archive/todos/Todos.refactor-backup.md)。
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
- 不做（当前版本，2026-10-20 重估）：`ecosystem/domain` 标签贯通 docs/blog/course、`/ai` 子领域 tag 筛选。`/ai` 只有 1 篇文章（candle-local-llm，已带 llm/inference 标签），各领域 1–2 篇、博客 2 篇、课程 1 门，筛选没有可筛的东西。重估条件：`[ "$(find assets/topics/ai -name index.md | wc -l)" -ge 8 ]` 退出码为 0（4 个子领域各约 2 篇）。已反向验证：现状为 1，造 8 篇时为 0

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
- [x] **M5e-对账（payment 侧，2026-08-04）** 查单能力：alipay `alipay.trade.query`（ACQ.TRADE_NOT_EXIST → 非成功事件供安全关单）+ wechat `GET /v3/pay/transactions/out-trade-no`（GET 空体签名）落在 trait 预留的 `build_query/parse_query` 上；`host::execute_query`；`OrderStore` 扩 `list_stale_pending`/`close_order`；`reconcile.rs`（成功→统一流水线回填；未支付超窗→条件关单，竞态让位回调；查单错误绝不误关）；payment 33 测（+9：查单解析×2 + 对账决策/回填/关单/竞态/金额拒/otn 防御）
- [x] **M5e-对账（接线，2026-08-04）** course `reconcile_pending_orders_once`（`server.rs:1845`：单轮≤50 单，逐单查网关→回填/关单；Unconfigured 静默跳过，查询失败绝不误关）+ app main.rs 启动期 tokio 定时器（server+payments 门控；PAY_RECONCILE_INTERVAL_SECS 默认 300s 下限 60s，MIN_AGE 300s，CLOSE_AFTER 7200s，PAY_RECONCILE_DISABLED=1 关）；.env.example 补支付对账开关段；双组合（含 no-payments server）编译 + clippy 零告警
- [x] **M5e-退款（2026-08-04）** trait 新增 `build_refund/parse_refund`（默认 Unsupported）+ 中立 `RefundRequest/RefundResult`（success|processing 受理）；alipay `alipay.trade.refund`（同步到账，fund_change Y/N 均幂等视为已退）+ wechat `/v3/refund/domestic/refunds`（PROCESSING 即受理，异步到账）；`host::execute_refund`；course `admin_list_orders`（最近 100 条含昵称）+ `admin_refund_order`（仅 paid；退款单号 `{otn}R1` 网关幂等；受理后条件 UPDATE paid→refunded + 撤销 source=purchase 权益，admin_grant 不动；pay_audit 留痕含操作人）；/admin/entitlements 新增「订单/退款」区块（payments 门控）；payment 36 测（+3 退款）；⚠️ 真实网关端到端验证仍待商户凭据
- 约定：验签是发货前提；金额核验；以 out_trade_no 幂等；回调可重入；密钥经 .env 校验不回显。
- M5 核心完成（M5a–M5d）：双网关下单 + 回调发货 + 购买 UI 全链路打通（服务端单测覆盖签名/验签/解密）。⚠️ 上线需真实商户号 + 公网 HTTPS 回调端到端验证。

## M6 — Pro 订阅会员 ✅
- [x] **M6a** memberships 表 + 实体 + 迁移；is_pro_member；纯函数 can_access_lesson（pro 课程允许有效会员）接入 get_lesson；单测 — 提交 `feat(course): Pro membership model + access control`
- [x] **M6b** my_membership + admin 授予/续期/撤销/列表 + Admin「Pro 会员」区块 + 我的订单页会员横幅 — 提交 `feat(pay): Pro membership management + display`
- 不做（当前版本，2026-10-20 重估）：会员自助购买与自动续费。Pro 会员只解锁 `access_tier: pro` 的课程，而现在没有这种课程（唯一的付费课 rust-basics 是 `paid`），卖会员等于卖一个什么也不解锁的东西。下单地基已就绪（`orders.product_kind`，打赏阶段加的），到时做一次性购买 N 个月只需加一个 `ProductKind`。重估条件：`grep -lqE '^access_tier: *pro' assets/courses/*/course.yaml` 退出码为 0。已反向验证：现状为 1，加一门 pro 课时为 0
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

### S9 — 生产路径 unwrap/expect 收敛（风险 R12）✅
- [x] 审计结果：core/app/migration 生产路径仅 2 处 `expect`（BASE_URL 启动门禁，有意 fail-fast），均加 `#[allow]` + 理由注释保留；build.rs 整文件豁免（构建期 panic 是惯例）
- [x] workspace lints：`[workspace.lints.clippy] unwrap_used/expect_used = "warn"`（配合 -D warnings 即拒绝）；app-core / app / migration 三 crate 接入 `[lints] workspace = true`，其余 crate 渐进；测试代码 crate 根 `#![cfg_attr(test, allow(...))]` 豁免
- [x] 顺带清理 S4 遗留死代码（forum 本地 require_session 包装）；clippy 零警告，三 crate 188 测试全过

### S10 — site.json 读取缓存（风险 R11）✅
- [x] 新增 `SiteConfig::load_cached(path) -> AppResult<Arc<SiteConfig>>`（`settings.rs`）：按 (path, mtime) 缓存，mtime 未变只走一次 metadata 系统调用；mtime 变化（admin 保存/手改）自动重读，无需显式失效
- [x] 热路径接入：主题 CSS、布局、主题列表、站点配置、auth service、feed、module/content-transformer 引擎慢路径、启动 plugins_lock 共 9 处直读点改用 load_cached；`from_file` 保留给写后重读场景（admin/moderation hook 未动）
- [x] 3 个新单测（同 mtime 命中 Arc::ptr_eq / mtime 变化重读 / 缺文件报错）；settings 19 测试全过

### 依赖与排序
- S1/S2/S3 为低冲突基础设施，先行；S4/S5 触及 DB 与密钥，居中单独提交；S6 审计后按需改动；S7 结构重构放在安全项之后避免冲突；S8–S10 收尾。
- 顺序：S1 → S2 → S3 → S4 → S5 → S6 → S7 → S8 → S9 → S10。

## 阶段验收（2026-07-21）
全部任务 S1–S10 已完成并逐个提交（无共同作者行）。
- 最终验证：全工作区 `cargo test --features server` **693 通过 / 0 失败**；`cargo clippy --workspace --features server --all-targets` 零警告；app 默认 web 目标 check 通过。
- 安全：安全头（S1）+ 限流（S2）+ JWT 即时吊销（S4）+ 独立加密密钥/密文版本化（S5）+ 支付回调加固（S6）+ CSS 反混淆（S8）。
- 运维：/healthz + STRICT_MIGRATION（S3）；结构：main.rs 组装拆分（S7）；质量：unwrap/expect lint（S9）；性能：site.json mtime 缓存（S10）。
- 遗留可选项：course 进度写入升级 verified 会话；admin/moderation 的 site.json 读取接入 load_cached；CSS 白名单解析器；CSP nonce 化；其余 crate 接入 unwrap lint。均非阻塞。

## 运行时验证与配置收尾（2026-07-24）
环境：apple container `rie-postgres`（postgres:16-alpine）+ `dx serve` 编译产物直跑 server 二进制。全部项目通过：
- **迁移（S4）**：启动日志 `Applying migration 'm20260721_000008_users_token_version'` → `schema migrations applied`；psql 确认 `users.token_version` 列存在、default 0。
- **/healthz（S3）**：`{"status":"ok","db":"connected","migrations":"applied"}`。
- **安全头（S1）**：`curl -I /` 四个头全部在线（CSP 完整策略 / nosniff / strict-origin-when-cross-origin / DENY）。
- **限流（S2）**：`/api/auth/login/github` 连发 20 次 → 前 15 个放行（= sensitive burst），第 16 起精确 429。
- **SSR**：`/blog` 首屏含正文。
- **密钥（S5）**：本地 `.env` 已注入 `DATA_ENCRYPTION_KEY`（openssl 生成，未入库）；重启后触发 PKCE 加密，日志无回退 warn = 独立密钥生效。
- **配置收尾**（本次提交）：docker-compose.yml app 环境新增 DATA_ENCRYPTION_KEY / STRICT_MIGRATION（compose 默认 1）/ RATE_LIMIT_* / CSP_POLICY 透传；`.env.example` 补运维开关文档。
- 待办（需公网环境）：浏览器 Console 确认无 CSP violation；gateway 反代环境下确认 /healthz 放行与 XFF 链。

---

# 新阶段 — 支付模块抽取为可选 crate（2026-07-27 评审结论）

> 背景：支付代码目前内嵌在 `module-course`（alipay.rs / wechat.rs / notify 两处 ~90% 重复）。
> 形态评审结论：**可选 crate（feature 门控）优于 WASM 插件**——支付是第一方代码，
> 沙箱收益用不上；trait 边界提供编译期安全，且与 `module-moderation` 可选基设模式同构
> （MODULE_SPEC §11.3 已合规）。trait 边界 = 未来 WASM ABI 边界，两路线不冲突。
> 原则：每任务一提交（无共同作者行）；提交后同步本文档；行为不变（纯结构重构，
> 不改支付语义）；S6 加固成果（原子认领/金额核验/pay_audit）必须完整保留并收敛进公共流水线。
> 校验命令沿用「持续约定」；额外要求：启用/禁用 payments feature 两种组合均须编译通过。

## 任务清单

### PM1 — 新建 `crates/modules/payment`（module-payment）骨架 ✅
- [x] `PaymentProvider` trait（`payment/src/lib.rs:161`）：`build_order` / `parse_order_response` / `parse_notify` + `build_query` / `parse_query`（默认 `Unsupported`，对账预留）；签名、验签、解密不入 trait（宿主 `crypto` 模块承担，WASM 化红线预留）
- [x] 中立类型：`OrderRequest` / `PayRequest`（url/method/headers/body/sign_payload）/ `PayAction`（PayUrl | QrCode）/ `OrderCall`（Direct | Http）/ `NotifyPayload`（Form | Json）/ `PaymentEvent`（out_trade_no/amount_cents/status/txn_id）/ `PayError`（5 类语义错误）
- [x] features：`alipay`、`wechat`（默认全开）+ `server`（重依赖门控，web 目标只编译类型与 trait）；`[lints] workspace = true`；登记 workspace members；crypto/alipay/wechat/pipeline 占位模块待 PM2/PM3 填充；3 单测 + clippy --all-features -D warnings 零告警

### PM2 — 迁移网关实现 ✅
- [x] `course/src/alipay.rs` → `payment/src/alipay.rs`（`AlipayProvider`：page/wap 签名直跳 = Direct，qr precreate = Http），`course/src/wechat.rs` → `payment/src/wechat.rs`（`WechatProvider`：native/h5 = Http + Authorization 头）；纯搬移 + 适配，协议逻辑不变；course 调用点机械替换 `crate::alipay/wechat` → `module_payment::alipay/wechat`（行为不变，PM4 再改 provider+pipeline）
- [x] crypto 工具迁入 `payment/src/crypto.rs`（decode_key / rsa2_sign / rsa2_verify / aes256_gcm_decrypt）；旧 5 个签名/验签/解密单测全部随迁通过，另补 7 个 Provider 适配单测（build_order 场景分派 / 响应解析 / notify 中立化 + 严格 yuan_to_cents），payment 共 15 测
- [x] 配置加载（env 读取 + 缺项降级 None）随迁；密钥仍经 .env 不回显；course 剔除 rsa/sha2/base64/reqwest/url/aes-gcm 依赖（server feature 改级联 `module-payment/server`）；course 40 测 + app server 构建 + 默认 web check 通过，clippy -D warnings 零告警

### PM3 — 统一 notify 流水线 `payment/src/pipeline.rs` ✅
- [x] `process_event`：验签/解密留在宿主侧 → 非成功状态查单前确认 → 查单 → 金额核验（DB 快照为准，`decide` 纯函数）→ 幂等快路径 → 原子认领（条件 UPDATE，rows_affected=0 确认不发货）→ 发货 → pay_audit 日志（S6 成果完整收敛，失败原因串与旧 wechat 应答一致）
- [x] 发货用注入回调 `on_paid(user_id, course_slug)`，不反向依赖 course entitlement；存储经 `OrderStore` trait（find_order / claim_paid）抽象，`sea_orm::DatabaseConnection` 自带实现（订单实体已下沉 app-core）。注：故意不用 sea-orm mock feature（会经 feature 统一禁用 `DatabaseConnection: Clone` 破坏 app-core 连接池）
- [x] 9 个流水线单测（纯 mock store + mock 发货回调）：decide 矩阵 / happy path / 幂等重放不二次发货 / 金额不匹配拒 / 并发认领败方不发货 / 非成功零 DB / 查无此单 / UPDATE 失败 / 发货失败；payment 共 24 测 + clippy -D warnings 零告警

### PM4 — course/app 接线（feature 门控）✅
- [x] `module-course` 改 `optional = true` 依赖 module-payment + fast_qr；新增 `payments = ["dep:module-payment", "dep:fast_qr"]`，server 改弱依赖 `module-payment?/server`；订单类型/订单 fns/notify 整区 payments 门控；`create_order` 改走 `PaymentProvider::from_env` + `host::execute_order`（payment 新增 host 执行器发送 PayRequest；kind 标签 qrcode/h5/redirect 保持旧协议；pay_err 展开内层文案保响应一致）；两个 notify handler 保留宿主侧验签/app_id/时间戳/解密/appid+mchid 校验后，`parse_notify` 中立化 → `pipeline::process_event`（发货回调 = grant_entitlement_internal）；课程详情/Paywall 购买入口收敛为 `PurchaseEntry` shim（关闭时退化「联系管理员开通」）；wechat `parse_order_response` 透传网关 message（与旧 post_v3 错误文案一致）
- [x] app 新增 `payments` feature（default = ["web", "payments"]）级联 `module-course/payments`；`pay_routes::mount` 条件编译（关闭时恒等，/api/pay/* 不注册）；/me/orders 路由保留但页面退化占位；classic/minimal 用户菜单「我的订单」`cfg!` 隐藏
- [x] 行为基线：默认组合路由/响应/日志 target（pay_audit）不变；双组合验证：默认（payments 开）clippy -D warnings 零告警 + course 40 测/payment 24 测 + app server 构建 + web check；禁用 payments（course server-only / app --no-default-features 的 server 与 web）均编译通过

### PM5 — 验证与文档 ✅
- [x] 双 feature 组合验证：默认（含 payments）app server 构建 + web check；禁用 payments（app --no-default-features 的 server / web + course server-only）均编译通过；全量 `cargo test --features server --workspace`（含 payment 24 / course 40）**0 失败**；`cargo clippy --workspace --features server --all-targets -- -D warnings` 零警告；ci.yml build-server 补 no-payments 双目标 check（server + web）
- [x] 文档：PAYMENT_SPEC.md 新增 §14「架构：PaymentProvider trait + 可选 crate」（分层职责 / feature 矩阵 / WASM 化 5 条安全红线：密钥不进沙箱、验签解密在宿主、收款方字段宿主注入、金额核验/认领/发货不下沉、强制 SHA256 lock）；MODULE_SPEC.md §11.1/§11.3 补 module-payment 可选基设例外

### 依赖与排序
- PM1 → PM2 → PM3 → PM4 → PM5 严格串行（同一批文件连续改动，不并行）。
- 风险提示（已落地 2026-08-04）：M5e 余项（对账/退款）已按计划直接写在 PaymentProvider trait 上（build_query/parse_query + build_refund/parse_refund），见下方阶段验收；WASM 化（Step 2）仅在出现第三网关/社区贡献需求时启动。

## 阶段验收（2026-08-04）
全部任务 PM1–PM5 已完成并逐个提交（无共同作者行）。

### M5e 余项补完（2026-08-04，同日三提交）
对账 + 退款代码落地（均写在 PaymentProvider trait 预留方法上，验证 trait 边界可扩展性）：
- 对账：双网关 build_query/parse_query + host::execute_query + reconcile 决策（回填走同一 notify 流水线，S6 语义在对账路径同样生效；关单只覆盖 pending，误关也可被迟到合法回调原子认领救回）+ app 启动期定时器（PAY_RECONCILE_* 可调）。
- 退款：双网关 build_refund/parse_refund + admin_refund_order（网关幂等退款单号 / 条件 UPDATE / 仅撤销 purchase 权益）+ /admin/entitlements 订单区块。
- 验证：payment 36 测；全工作区测试 0 失败；workspace clippy -D warnings 零警告；启用/禁用 payments 组合编译通过；PAYMENT_SPEC §9/§12/§13/§14 同步。
- ⚠️ 仍待商户凭据：对账/退款/下单/回调的真实网关端到端验证。
- 结构：支付从 module-course 抽为可选 crate module-payment（trait + 中立类型 + crypto/host 宿主工具 + 统一 notify 流水线）；两处 ~90% 重复的 notify 处理收敛为 `pipeline::process_event`，S6 成果（验签前提/金额核验/原子认领/pay_audit）完整保留。
- 门控：course `payments` feature + app 默认开；关闭时 /api/pay/* 不注册、购买 UI 退化「联系管理员开通」，admin 手动授权链路不受影响。
- 验证：全工作区测试 0 失败；clippy -D warnings 零警告；启用/禁用 payments × server/web 四组合编译通过；CI 新增 no-payments 门禁。
- 行为基线：路由/响应文案/日志 target 与重构前一致（纯结构重构）。⚠️ 网关端到端（沙箱/真实商户号）验证仍待 M5b/M5c 标注的前置条件具备后进行。

---

# 新阶段 — 迁移到自研组件库 dioxus-ui（2026-10-08）

> 分支 `feat/dioxus-ui-migration`。详细计划与决策（D1–D6）见 [`docs/DIOXUS_UI_MIGRATION.md`](docs/DIOXUS_UI_MIGRATION.md)；
> 组件库问题记入 [`docs/DIOXUS_UI_FEEDBACK.md`](docs/DIOXUS_UI_FEEDBACK.md)（FB-NN 编号，临时处理在代码注释里标注编号）。
> 每任务一提交；验收含亮/暗截图对比、无 hydration 警告、SSR 首屏烟测。

- [x] U0 — 迁移计划 + 反馈记录 + 本清单
- [x] U1 — 接入 `dioxus-shadcn` 0.6 依赖、shadcn token（默认值 = 站点现有 stone/orange 配色）、构建时生成 `@source`；外观不变
  - 落点：`Cargo.toml` workspace dep（dioxus 仍 0.7.9）；`crates/app/tailwind-input.css` token + `@theme inline`；`scripts/tw-sources.mjs` → `crates/app/tailwind-sources.css`（gitignore）；`package.json` build/dev 先跑脚本，build 加 `--minify`（与已提交产物格式一致）
  - 验证：9 个页面 × 亮/暗 像素对比，差异全部是动画帧（流光按钮、spinner、渐变字）；生成脚本反向检查两条均退出 1；clippy 除既有 `llm` lint 外零警告
  - 旧变量别名未在 U1 切换（实验结论见计划 D2），随 U2 做
  - 顺带发现的既有问题（不在本任务范围）：
    - CSP 无 `'unsafe-eval'` → 全站 `document::eval` 失效且 panic，暗色切换在生产不可用（自 S1 起）；组件库同样受影响，见 FB-02，**阻塞 U4/U8**
    - Dockerfile 未 COPY `crates/sdk-macros`、`crates/gateway`（2026-06-01 后新增），镜像构建应已失败
    - `crates/app/build.rs` 先把 root `assets/` 正向同步到 `crates/app/assets/`，任何 cargo 构建都会用旧的 `assets/tailwind.css` 覆盖刚编译的产物；`npm run build` 后需手动 `cp crates/app/assets/tailwind.css assets/`
    - rustc 1.99 的 clippy 对 `crates/llm/src/lib.rs:184`（`async_trait` 展开）报 `double_must_use`
      - 已修（2026-10-09，c40c14d）：async-trait 0.1.89 → 0.1.92 后不再触发，CI 的 `-D warnings` 不需要 `-A` 也能通过
- [x] U2 — 三个主题插件改输出 shadcn token，旧 `--color-*` 改为 token 别名，重编 wasm，THEME_SPEC 更新
  - 决定（2026-10-08）：ocean 主色保持品牌橙（不再用 hue 250 蓝），sunset / catppuccin 保留各自主色
  - 落点：`crates/plugins/theme-{ocean,sunset,catppuccin}/src/lib.rs` 输出 17 个 token（亮/暗各一套）；`assets/css/main.css` 旧变量改为别名；`tailwind-input.css` 删旧 `@theme` 默认值；`examples/plugin-theme-purple`、`docs/PLUGIN_DEV.md`、`docs/THEME_SPEC.md` §12「Token 契约」同步；`app-core` 主题测试断言 `--primary:`
  - 验证：3 主题 × 亮/暗，`--primary`、`--background`、别名 `--color-bg`、body 底色均取插件值（**需绕过 CSP 验证**，见下）；app-core plugin/theme 66 测通过；clippy 同 U1
  - ⚠️ 发现：CSP 下 App 根组件的 `document::eval` 在页面加载时即 panic，wasm 运行时失效，主题 CSS 根本不注入、ThemePicker 不显示——**自 S1 起站点一直没有应用任何主题插件**（`bypassCSP` 对照实验确认；站点尚未上线）。见 E1
- [x] E1 — 站点自身的 `document::eval` 改为 web-sys 直接调用，CSP 保持不含 `'unsafe-eval'`（2026-10-08 决定）。归入下方安全整改阶段 B1
- [x] U3 — admin（`admin.rs` + `admin_entitlements.rs`）：Button / Input / Textarea / NativeSelect / Tabs / Table / Spinner / Badge
  - 落点：admin 内容区包 `DensityProvider(Compact)`（否则 `Sm` 按钮被 `min-h-10` 撑高）；页签只用 `tabs_list_class` / `tabs_trigger_class`（Tabs 组件挂载即 eval，FB-14）；复选框保留原生（Checkbox 走 eval，FB-02）；状态标签改 Badge 语义色；表格行显式 `border-border`（FB-15）
  - 验证：8 个 admin 页面在严格 CSP 下无 pageerror（仅既有 SEC-23 字体报错）；审核页签切换、全选启用批量按钮、单条「通过」、角色修改并刷新后保持、阈值输入均可用；亮 / 暗截图检查。测试数据（2 条审核记录、临时 admin 角色）已清理
- [x] U4 — course + 支付（`course.rs` + `pay_ui.rs`）：Card / Button / Badge / Table / Dialog
  - 落点：课程卡片 / 付费墙 / 下载列表 / 标注分组用 `card_class`；按钮、徽章、加载圈、进度条（`Progress`，轨道用 `bg-border`，ocean 的 `--muted` 近白）、订单表换组件；支付方式二选一用 Outline 按钮 + `aria-pressed`（ToggleGroup 走 eval）；章节折叠、代码页签保持手写（Accordion / Tabs 走 eval）
  - 验证：严格 CSP 下课程列表、详情、付费墙、试看课节、我的订单无 pageerror；购买弹窗打开、切换渠道、关闭按钮可用；亮 / 暗截图；SSR 首屏含正文；`--no-default-features` 编译通过
  - 未做：支付模态改 Dialog（Escape 关闭、焦点管理）——待上游 FB-02 发版，记为 U4b
- [x] U-0.6.2 — dioxus-shadcn 升级到 0.6.2（FB-02 已修，解除 U4b/U8/U9 阻塞；删除 FB-15 的逐行 `border-border`；严格 CSP 下 /admin/entitlements 无新报错）
- [x] U-0.6.3 — dioxus-shadcn 升级到 0.6.3（上游修复 FB-17/18/19/21，FB-13 已修、FB-20 改为文档说明）。删除本站的临时处理：Dropdown 的 `fixed`、搜索 Command 只在打开时挂载、导航栏按钮的 `min-h-*`。浏览器验证：菜单与触发器对齐；⌘K 异步结果自动高亮第一项，↓ 和回车正常，关闭后再打开也正常；导航栏按钮高度不变。`Sm` 按钮在默认密度下变为 32px
- [x] U3b — admin 审核页签 → Tabs（FB-14 已修；「全部」的值改为 `all`；严格 CSP 下点击与方向键切换、请求参数验证通过）
- [x] U4b — 支付模态 → Dialog，支付方式选择 → ToggleGroup
  - 落点：`pay_ui.rs`；表单只在打开时挂载（重开即重置）；ToggleGroup 受控、忽略空值，保证总有一个网关
  - 验证：严格 CSP 下 Escape 关闭、焦点回到购买按钮、Tab 限制在弹窗内、方向键切换焦点；亮 / 暗截图
  - 行为变化：点遮罩不再关闭（Dialog 默认，避免扫码时误关）
- [x] U5 — forum + 评论：Button / Input / Textarea / Card / Empty / Alert
  - 落点：话题卡片 / 引用卡片 / 回复框 / 评论框用 `card_class`；标签与标签云用 `badge_class`（链接）；输入框、文本域、登录按钮、加载圈、空状态、错误提示（`Alert` Destructive）换组件；`btn-flow` 特效按钮保留；编辑 / 预览切换保留下划线样式，颜色改 token
  - 验证：严格 CSP 下发帖（含标签与预览）、回复、博客评论（预览 + 发布）均可用，失败时 Alert 显示错误；亮 / 暗截图；测试话题、回复、评论已删除
  - 发现：ocean 暗色的 `--card`（slate-900 藏青）与页面 stone 底色不协调——插件 token 与 slate→stone 色阶映射混用，留给 U10 评估；debug 构建类名覆盖日志过多（FB-16）
  - 副作用：U3 测试改角色使本地 user 2 的 `token_version` 变为 2，该账号本地已有会话需重新登录
- [x] U6 — cases / docs / podcast / search 列表部分
  - 落点：案例 / 文档 / 播客卡片用 `card_class`；计数与标签 chip 用 `badge_class`；加载圈、空状态（`Empty`）、错误（`Alert`）、案例搜索框（`Input`）、播客翻页（Ghost `Button`）、搜索 kind 过滤（`aria-pressed`）换组件；蓝色强调改 `primary`；案例分类配色、播客播放器深色卡片保留
  - 验证：严格 CSP 下案例搜索 / 空状态 / 标签筛选、文档首页与详情、播客标签筛选、搜索过滤与结果均可用，无 pageerror；SSR 首屏含案例与文档卡片
- [x] U7 — 5 个内容板块（先 ai，再套用其余 4 个）
  - 落点：搜索框 `Input`、子主题 chip（`badge_class` + `aria-pressed`）、文章卡片 `card_class`、标签 / 子主题徽章、加载圈、空状态、加载失败 `Alert`；先改 ai 验收，再用同一脚本套用到 cli / embedded / wasm / web3
  - 验证：严格 CSP 下 5 个板块的筛选、搜索、空状态、文章详情、未找到文章的错误提示均可用，无 pageerror；SSR 首屏含卡片
  - 发现（既有，未修）：`/wasm` 整页请求返回 404——路由与 Dioxus 静态目录 `/wasm/`（wasm 产物）冲突，只能经客户端路由进入；生产环境未验证。板块 crate 单独 `cargo test --features server` 编译失败（其 server feature 未开 `app-core/server`），不带 feature 时 13 测通过
  - 复查（2026-10-08）：`/wasm` 404 只在 debug（`dx serve` 把 `public/wasm/` 整个挂在 `/wasm`）；release 无该目录，`/wasm`、`/wasm/:slug` 均 200 且 SSR 含正文，记入 DEVELOPER.md 2.3。5 个板块 crate 单独 `--features server` 测试现均通过（各 15 测）
- [x] U8 — 全局弹层：auth_modal / search 模态 → Dialog(+Command)；主题/语言/用户菜单 → Dropdown；移动端菜单 → Sheet
  - [x] U8a — auth_modal → Dialog（新增 `auth.close` 文案；Escape、焦点限制与回位验证通过）
  - [x] U8b — search 模态 → Dialog + Command + ToggleGroup（回车只跳站内路径，`is_site_path` 带测试；发现 FB-17 Command 在关闭的 Dialog 里键盘失效 → 只在打开时挂载；FB-18 异步结果不自动高亮）
  - 顺带修复：`tailwind-input.css` 只扫了 6 个 module crate，docs/search/5 个板块独有的 16 个类此前没有生成（ffaaf91）
  - [x] U8c — theme_picker / lang_picker / 用户菜单 → Dropdown（两个布局的用户菜单合并为 `UserMenu`；主题、语言为受控单选组；键盘、Escape、外部点击、跳转验证通过；发现 FB-19 菜单在 flex 行里定位偏移 → 内容加 `fixed`）
  - [x] U8d — 移动端菜单 → Sheet（右侧抽屉，放在 header 之外以免被 backdrop-filter 限制；新增 `nav.menu`；375px 下 Escape、焦点回位、点链接跳转并关闭验证通过，无横向滚动）
- [x] U9 — 生态 mega 菜单：评估 NavigationMenu 与现有纯 CSS 方案（D5），不劣于现状才替换（结论：保留 CSS 方案——关闭 JS 时 CSS 菜单可悬停 / 聚焦展开，NavigationMenu hydration 前打不开；重估条件写入 D5；上游建议 FB-20）
- [x] U10 — 收尾：清理无用类名与 CSS、评估移除色阶映射、更新 TAILWIND_GUIDE、反馈汇总
  - 无用 CSS：`main.css` 的 `btn-flow` / `text-flow*` 仍在用；`--color-*` 别名是 THEME_SPEC 写明的第三方插件兼容接口，保留
  - 色阶映射：保留。存量 `slate-*` 约 1100 处 / 25 个文件，删除即全量改写；新代码改用 token（写入 TAILWIND_GUIDE 2.1/2.2）。副作用：主题 token 为冷色时组件与手写 stone 区域冷暖不一——2026-10-08 决定统一为暖色：ocean 的亮 / 暗中性色 token 改为 stone 色阶（主色不变），带测试
  - TAILWIND_GUIDE：token 优先、扫描范围、组件用法与 FB 绕法
  - FB 汇总表：DIOXUS_UI_FEEDBACK.md 开头；待上游 FB-17/18/19/20、FB-13 部分修复
  - 全量校验：fmt、workspace clippy（`-A clippy::double_must_use`，llm 既有）、workspace 测试 738 通过（修复了 U2 遗留的示例插件测试）、wasm / 无 payments 构建；严格 CSP 下 14 个页面无新报错
- [x] U10+ — 复查遗漏（2026-10-08）：4 处手写加载圈改为 `Spinner`（3917978）；`/wasm` 404 确认只在 debug（66280e7）；TAILWIND_GUIDE / THEME_SPEC 去掉「主题插件」说法
- U11 — 剩余原生控件（去留表见 DIOXUS_UI_MIGRATION.md §4 U11）
  - [x] U11a — admin 复选框 / 设置开关 → Checkbox / Switch（e2b264a：全选在部分选中时显示半选；设置开关拿 `bool`，去掉字符串猜值；workspace 639 测通过、clippy 无告警、CSS 含组件类名。⚠️ admin 页浏览器验证待做——需本地管理员会话）
  - [x] U11b — forum 提交、博客翻页、搜索入口 → Button（53a57c0：提交保留 `btn-flow`；翻页加 aria-label + i18n；搜索入口 32px 不撑高导航栏，点击打开搜索框、焦点进入输入框。发现 FB-21：默认密度下 `Sm` 实际 40px。论坛提交按钮需登录，浏览器未验）
  - [x] U11c — classic / minimal 导航栏按钮 → Button（ghost / icon）（4569177：尺寸保持原 36/32px（classic）与 28px（minimal），用 `min-h-*` 覆盖密度最小高度（FB-21）；暗色开关补 aria-label + aria-pressed（新 key `nav.toggle_dark`）。浏览器验证两种布局：导航栏高度不变，暗色切换、移动端菜单、登录弹窗均正常）
  - [x] U11d — 博客 / podcast / cases 标签筛选 → `badge_class` + `aria-pressed`（与 U7 一致）（6551fb7：博客去掉硬编码 `bg-blue-600`，改用与 podcast 相同的 chip 样式；cases 分类选中保留 `btn-flow`，标签未选中保留各自配色；三处均补 `type=button` + `aria-pressed`。浏览器验证三页点击后 aria-pressed 正确切换、cases 标签再点取消。附带发现：`dx serve` 下 podcast 音频 404——build.rs 不同步 `audio/` 到 crates/app/assets，只影响开发态，Docker 用完整 assets 不受影响）
  - [x] U11e — 编辑 / 预览切换、课程代码页签 → Tabs（54e3853：评论、论坛发帖的编辑 / 预览与课程代码文件条改为 Tabs，带 role=tab / aria-selected / 方向键；只挂当前面板。浏览器验证代码页签（临时把付费课节设为试看，已还原）时发现：切换文件后代码不变、控制台 wasm-bindgen 报错——Prism 换掉了 <code> 的文本节点，原地更新写空；CodePanel 按文件加 key 重挂后修复，点击与方向键均切换且高亮。评论 / 论坛编辑器需登录，浏览器未验）
  - [x] U11f — 课程章节折叠 → Collapsible；课程标注开关 → Button（修暗色）（763e9b7：章节与课节视频块改 Collapsible，箭头由 data-state 驱动；标注眼睛按钮改 Outline/Icon Button 走主题变量，暗色不再白底。顺带修 build.rs 正向同步用 root 旧 tailwind.css 覆盖新编译产物的问题 59c3e5b。课节视频块无现成内容，未浏览器验证）
  - [x] 课节页标题重复显示两次（9dd54cf：读课节时拆掉正文开头的 `# 标题`；frontmatter 标题优先，缺失时用该 H1，再退到 slug。注意标注 block id 按顶层块序号分配，正文增删块会让已有标注错位）
  - [x] 标注错位（abf2617：annotations.js 先核对 exact_text，对不上按原文 + 前后文在全部块里重找，原文已删则不画；浏览器内 5 种情形前后对比验证）
  - [x] 我的标注跳回原文不准（2c2a0cf：链接改 `#anno-{id}`，闪烁重新定位后的标注本身；原文已删不闪；旧 `#b{n}` 仍兼容）
  - [x] 静态资源缓存头（e997ff6：`/js` → `no-cache`（未变 304）；`/courses` 按用户鉴权 → `private, no-cache`，防 CDN / 反代缓存付费课时发给未购用户；其他目录保持默认）


---

# 新阶段 — 安全整改（2026-10-08，上线前完成）

> 详细发现（SEC-01 ~ SEC-22，含核实状态）与做法见 [`docs/SECURITY_REMEDIATION.md`](docs/SECURITY_REMEDIATION.md)；dioxus-ui 的问题见 `docs/DIOXUS_UI_FEEDBACK.md` FB-02 ~ FB-12。
> 每项一提交；每个修复附「修复前失败」的测试，先确认红再修到绿。

### 阶段 A — 可被直接利用的漏洞（最先做）
- [x] A1 — SEC-01 LaTeX `\text{}` 存储型 XSS：MathML 白名单过滤后再输出（829ffa3：改为渲染前转义文本类事件与错误信息，6 条单测）
- [x] A2 — SEC-02 `/courses` 静态目录绕过付费墙（课时目录文件按 get_lesson 规则鉴权，无权 404；另修课程测试改 cwd 的竞争）
- [x] A3 — SEC-03 `get_doc_content` 路径穿越（复用 safe_join_under，移入 app_core::utils）
- [x] A4 — SEC-04 OAuth profile 不查状态 / `external_id` 回退 `"0"`（宿主侧修复；插件未改，见计划 A4 说明）

### 阶段 B — 站点加固
- [x] B1 — SEC-07 / SEC-18 站点 `document::eval` → web-sys（即 E1）（另替换 `document::Title`；`widgets::browser` + 源码扫描测试 `csp_no_eval`；严格 CSP 下浏览器验收）
- [x] B2 — SEC-09 登出改 POST；不可信内容图片只允许 `/uploads/`（3ccb475：只收 POST，303 回首页，GET 405、`Sec-Fetch-Site: cross-site` 403，菜单经 `browser::post_navigate` 提交表单；ee19c46：不可信图片必须是 uploads 生成的文件名形状，外部 / data / 其他站内路径一律拒绝；路由与 URL 单测先红后绿，浏览器验收登出）
- [x] B3 — SEC-08 主题 CSS 白名单化；收紧 `img-src`（28ed724：cssparser 分词的白名单 `check_theme_css` 替换黑名单，at-rule / 函数 / `url()` 目标均白名单，拒 `<` / bad token / 深嵌套，字节聚合路径补检查，内置三主题过检测试且反向验证；2b9ed37：`img-src` 只留 self、data: 与四个 OAuth 头像 CDN；浏览器验收三主题生效）
- [x] B4 — SEC-11 `site_theme` cookie 读取侧校验（99deeb3：`engines::theme::is_theme_filename`（`[A-Za-z0-9_-]+\.wasm`）同时用于 `theme_with_override` 与 `/api/theme/set`，非法 cookie 忽略并 warn；单测先红后绿，curl 验收 `../`、`.wasm.bak` 回落默认主题。未做「在主题列表内」：指向非主题插件只会让该用户自己的主题 CSS 为空，缓存键数量受 plugins 目录文件数约束）
- [x] B5 — SEC-10 gateway 不覆盖应用 CSP（1fef74c：删除 gateway 的 CSP 定义与 `CSP_POLICY` 读取，CSP 原样转发；通用安全头仅在应用未下发时补，HSTS 仍由 gateway 设；`apply_security_headers` 两条单测先红后绿；DEPLOY_GUIDE 表格更新。未起真实 Pingora + TLS 做端到端）
- [x] B6 — SEC-13 `require_writer` 回查数据库（dbc9a2b：`require_writer` 改走 `require_session_verified`；同文件的 `require_admin_user`（授予 / 撤销权益、订单管理）同样只信 JWT，换成 `session::require_admin`（回查 role + token_version）；`create_order` 写路径也改为 verified。无现成请求上下文 + DB 的测试设施，红绿用真实服务 + Postgres 验证：u2 旧 tv token 写入、u1 自称 admin 读权益列表，修前均通过，修后分别被拒；有效 token 仍可写）
- [x] B7 — SEC-17 不可信内容的 mermaid 按代码显示（cd81f5f：`renders_as_diagram(lang, untrusted)`，用户内容的 mermaid 走 `CodeBlock`（`language-mermaid`，引导脚本只扫 `.mermaid`）；单测先红后绿；浏览器验收：论坛帖显示为代码、无 SVG，welcome 文章两张图照常渲染；用户内容的原始 HTML 本就按文本输出，无法注入 `class="mermaid"`）
- [x] B8 — SEC-19 / SEC-20 HSTS、Permissions-Policy、`ws:` 仅开发；删 `/api/echo`；裁剪公开配置（ca5307c：应用侧补 HSTS（与 gateway 同值）与 Permissions-Policy（关 camera / microphone / geolocation / payment / usb / browsing-topics，不动 fullscreen）；`connect-src` 仅 debug 构建放行 `ws: wss:`；删 `/api/echo` 与 Echo 组件；`/api/site/config` 无调用方且返回整份 site.json，直接删除而非裁剪 DTO；头部单测先红后绿，curl 验收两端点不再可用、新头已下发）

### 上线前收尾（2026-10-09）
- [x] SEC-23 核实为开发环境独有：Google Fonts `@import` 来自 `dx serve` 注入的 toast 模板，release 产物无此请求（002cf09，仅文档）
- [x] B 站嵌入改为 https（d179733：`bilibili_embed_url`，单测先红后绿）
- [x] 删除 `annotations.js` 中无元素使用的 `.rie-anno-toggle` 样式（6dc48f1）
- [x] 粗体 / 斜体 / 删除线全站不生效：`render_tag` 缺 Strong / Emphasis / Strikethrough 分支，落到无样式 `span`（05c4bc8：输出 strong / em / del；widgets 加 dev 依赖 dioxus-ssr，渲染测试先红后绿，可信与不可信内容各测一遍）
- [x] 登录态浏览器验收（GitHub 登录，2026-10-09）：`/me/annotations` 创建 / 列出 / 删除；论坛发帖（强调样式正确、`<script>` 被剥离）；admin 7 页（概览 / 用户 / 评论 / 话题 / 审核 / 审核设置 / 课程权益）均 200、无新增控制台错误；审核设置表单控件齐全且有 label（未提交，保存会改 `assets/site.json`）；测试帖经 `/admin/topics` 删除
- [x] 管理端删除话题 / 评论加确认框（AlertDialog，取消在前、删除为危险样式，文案写明删除对象与回复数；浏览器验收：取消后仍在、确认后删除、评论页同样弹框）
- [x] 不存在的话题回 404：详情页改用 use_server_future + SuspenseBoundary，未找到时提示并 `commit_http_status(404)`（原先 200 + 永久加载态）；列表页仍按 B6 走 use_resource。curl：`/topics/999999` 404、`/topics/1` 200；客户端从列表点进详情正常
- [x] SEC-24 路径穿越（排查 404 时发现，高）：博客 `get_blog_content` 的 id 与课程 `get_lesson` 的 slug 未校验，未登录可读付费课时全文（43a35f4：`app_core::utils::is_safe_slug`，博客 `post_file` 与课程三个读取函数入口校验；测试先红后绿；live 复测两种请求均被拒）
- [x] 博客 / 文档不存在时回 404（原先 200 + 页面显示 server fn 原始报错，博客还挂评论区）：两个 server fn 改返回 `Option`，新增 `widgets::NotFound`（404）/ `LoadFailed`（500，通用提示），论坛详情改用 NotFound；评论与讨论只挂在存在的文章下。curl：缺失 404、正常 200、无原始报错；浏览器：客户端跳转与水合正常
- [x] 5 个板块模块（ai / embedded / wasm / web3 / cli）的内联 slug 校验改用 `app_core::utils::is_safe_slug`，规则只留一处；行为不变（bebd571）
- [x] 审核队列「拒绝（删除内容）」与「批量拒绝」加确认框（原先单击即删业务内容）：`ConfirmDelete` 增加 `label` / `variant` / `disabled`，批量确认框写明条数（0c568c9）。本地无审核 stage、队列为空，审核页按钮未在浏览器点验；共用组件在评论页回归通过
- [x] `docs/RoadMap.md` 仍把 WASM 插件写成现状 → 移到 `docs/archive/` 并加归档说明，指向 ENGINES / MODULE / DEVELOPER 文档；其余文档在 R1–R5 已改过（ac13174）

### 待评估 — 插件改为编译期依赖（2026-10-08 提出）
- [x] P-EVAL — 采纳方案 A：第一方插件全部编译进宿主，移除 WASM 运行时（2026-10-08 决定）。依据：10 个插件全部在本仓库构建、同一作者；认证插件只是端点常量 + 十几行字段映射，主题是 CSS 字符串；i18n 插件生产无调用方（`/api/i18n/translate` 无客户端调用）；content-toc 与审核插件均未启用（`content_transformers: []`、site.json 无 moderation 块）；宿主插件设施 + SDK 约 3,400 行。剩余两个「高」（SEC-05 / SEC-06）只因运行时加载插件而存在，且认证做成插件无法安全：端点收回宿主后插件仍决定 `external_id`。C1 / C2 / C4 / C5 / C6、D1 随之取消，C3 保留。重估条件：`assets/plugins/` 中出现无对应源码 crate（`crates/plugins/` 或 `examples/`）的 wasm，或站点要作为产品给他人部署

### 阶段 R — 插件编译进宿主（P-EVAL 方案 A）
- [x] R1 — 认证内置：`Provider` 枚举（github / google / discord / twitter）给出固定 https 端点、展示信息与 `map_profile`（缺 uid 报错，不再 `unwrap_or(0)`）；`AuthService` 改用枚举，按凭据配置启用；删 4 个 auth 插件 crate 与 wasm（SEC-05）（a18de30：`core::auth::provider` 内置四份 `ProviderSpec` 常量与字段映射，`map_profile` 取不到可用 uid 返回 `None`；登录 / 回调只接受 site.json 启用的 provider，`auth.providers` 改为 id 列表；顺带删掉无调用方的 `AuthEngine` 与 sdk 的 auth 插件类型；AUTH_GUIDE 重写。provider 映射与启用规则 6 条新单测；live 验收：四个按钮、github / twitter（含 S256 challenge）跳转到固定端点、未知 provider 与缺 PKCE cookie 的回调被拒。未做真实第三方账号的完整登录往返）
- [x] R2 — 主题内置：内置主题表（ocean / sunset / catppuccin），site.json、管理端设置与 `site_theme` cookie 改用主题 id，只接受表内 id；删 3 个主题插件 crate、`examples/plugin-theme-purple` 与 wasm（d2a8ad7：CSS 移到 `engines/themes/*.css` 经 `include_str!` 嵌入 `THEMES` 表；`resolve_theme` 只认表内 id，cookie 为旧 wasm 文件名 / 路径 / 其他插件时 warn 并回退；site.json 的 `themes` / `active_theme` 合为单个 `theme`（每个主题都给全套 token，栈无意义）；删 PluginManager 的主题 CSS 函数与缓存、admin 主题行；构建脚本 / CI / Dockerfile 不再构建主题；THEME_SPEC 重写，顺带修正 ENGINES_SPEC 中 R1 漏改的 AuthEngine。新单测 5 条（含 cookie 表外值回退）；live 验收：三主题 CSS、旧文件名与路径 cookie 回退 ocean、`/api/theme/set` 拒绝表外值、浏览器 picker 切到 Sunset 后 `--primary` 与 cookie 生效）
- [x] R3 — 删除未使用的插件能力：i18n 插件与 `/api/i18n/translate`；content-transformer 引擎、`content_transformers` 配置、各模块 `apply_default_pre` 调用与 content-toc 插件（c6a495f：删 i18n-fluent / content-toc 两个 crate 与 wasm、`/api/i18n/translate`（gateway 写路径表同步去掉）、`ContentTransformerEngine`、`content_transformers` 配置与 8 个内容模块的调用、SDK 的 transformer ABI 与 i18n / content-transformer 能力；`build_themes.sh` 已无可构建的插件，连同 Dockerfile 步骤与 CI `build-wasm-plugins` job 一并删除；删 CONTENT_TRANSFORMER_SPEC。插件运行时测试改用审核 wasm 作夹具（故意改错期望值确认真的执行而非跳过）；697 通过（减少的是被删代码的测试），web 警告 12；live：translate 返回 405，blog / docs / course 正文照常渲染）
- [x] R4 — 审核内置：deepseek 的提示词构造 / 结论解析移入 moderation crate，经 `crates/llm` 调用，替换 `plugin_stage`；删 `examples/plugin-moderation-deepseek`（c48264d：新增 `LlmModerationStage`（`build_messages` / `parse_verdict`），链接提示复用宿主 `extract_urls`；用原 wasm 对同一批 4 条提交 + 6 条模型回复逐条对比输出一致后删除插件源码；site.json `moderation.plugins` 改为 `llm_review` 开关（旧字段忽略、不会开启 LLM），管理端表单改为勾选框；删 `PluginModerationStage`、SDK 审核 ABI 常量与 `ModerationVerdict`；失败仍 fail open（C3 处理）。wasm 移到 `crates/core/tests/fixtures` 供运行时测试用到 R5。新单测 11 条，693 通过；live LLM 测试 7 条通过（辱骂 / 仿冒链接 Block，正常文本 / 图片 Allow）；管理端表单未在浏览器验收）
- [x] C3 — SEC-12 审核失败默认 fail closed（送人工复核）（在 R4 的内置 stage 上做）（a7a9f89：LLM 调用失败 / 回复读不出结论时按 `site.json::moderation.on_llm_failure` 处理——`review`（默认）记 Flag 照常发布并进审核队列，`reject` 记 Block 拒绝提交，不提供放行；warning 带原因与进程内累计失败次数；管理端表单加下拉。修流水线选结论：先比 label 再比分数，分数 0 的失败 Flag 不再被黑名单 stage 的 Allow(0) 盖掉。live 图片测试原先靠 fail-open 通过（模型服务下载不了 Wikimedia 图，`invalid_image_url`），改用内联 PNG data URL 后得到真实 Allow。新单测 4 条，697 通过；live LLM 测试 7 条通过；管理端表单未在浏览器验收）
- [x] R5 — 移除插件运行时：`PluginManager` / 插件引擎 / `plugin_security` / wasmi 依赖、SDK 的 ABI 部分与 `sdk-macros`（共享类型留在 `sdk`）、管理端插件上传 / 重载 / 列表、`/plugins` 公开页、`plugins_lock` 与 `lock_plugins`；删 `PLUGIN_DEV.md` / `PLUGIN_ABI.md`，README 与架构文档改写（SEC-06 / 14 / 15 / 16 / 22 的攻击面随之消失）（6bf761c：删 `PluginManager` / `engines::plugin` / `plugin_security` / 夹具 wasm、wasmi / cssparser / wat 依赖、`AppError::Plugin`；settings 删 `plugins_lock` 与无读取方的 `paths`（旧 site.json 仍可解析）；sdk 只留 `ModerationSubmission` / `ImageRef` / `AppModule`，删 `sdk-macros`；删 `/plugins`、`/api/plugins/public-list`、管理端插件页与 list / reload / upload；`lock_plugins` 本就不存在，只删了文档引用；顺带删 llm 里只为老插件存在的字符串 content 反序列化。docs：删 PLUGIN_DEV / PLUGIN_ABI，改写 README、DEVELOPER、AUTH_SPEC、ADMIN_SPEC、OPERATIONS 等。+4,042 行删除；639 通过，web 告警 12→5，gateway 5 通过；live：`/plugins`、`/admin/plugins` 404，插件 API 405，首页 / 博客 / 文档 / 论坛 / 管理端正常）

### 阶段 E — dioxus-ui（在 dioxus-ui 仓库修复）
- [x] E-1 — 把 FB-02 ~ FB-12 同步到 dioxus-ui 的待办 / RFC，按其流程修复发版（上游 0.6.1 去除 eval、0.6.2 安全加固；FB-09/12/16 上游 wontfix 并给出理由；本站升级到 0.6.2，各条状态见 DIOXUS_UI_FEEDBACK.md）

### 阶段 O — 页面与板块优化（2026-10-19 走查）
方案与证据见 [`docs/SITE_OPTIMIZATION_PLAN.md`](docs/SITE_OPTIMIZATION_PLAN.md)（E1–E15 为证据编号，D1–D4 为待作者决定项）。每条任务一个提交；带「需决定」的任务先等对应 D 项确认。
- [x] O1 — 板块共享组件：`widgets::board::{BoardIndex, BoardArticle}`，5 个 `<board>.rs` 改为调用；不合并 crate（E5）。验收：5 板块落地页 / 文章页渲染与改前一致（SSR HTML diff 只差预期部分），各板块单测通过
  - 完成：共享实现放在新 crate `crates/board`（widgets 不依赖 shadcn / i18n，单独成 crate 更干净）：`Board` 常量 + 一对带板块参数的 server fn（`list_board_articles` / `get_board_article`，整个 `topics/` 一份 mtime 缓存）+ `BoardIndex` / `BoardArticle`。5 个板块 crate 只剩 `lib.rs`（`BOARD` 常量 + 两个一行页面 + `problems()` 自检单测），净删约 3000 行。8 个页面 SSR HTML 与改前逐字相同（仅 hydration 调试类型名不同）
  - 发现：dev 下 `/wasm`、`/wasm/:slug` 404——dioxus-server 在 debug 构建把 `public/wasm` 目录整体 `nest_service` 到 `/wasm`，抢走路由；release 按文件逐个挂载，不受影响。仅影响本地开发，不改路由。重估条件：release 构建下 `curl -o /dev/null -w '%{http_code}' $BASE/wasm` 不为 200
- [x] O2 — 板块文章页：frontmatter 走 `inject_seo`；不存在的 slug 回 404（server fn 改返回 `Option` + `widgets::NotFound`）；卡片与返回链接改 `Link`；文章头显示日期与标签（E1/E3/E4）。验收：curl 缺失 slug 404、正常页 `<title>` 为文章标题且带 description / canonical；单测先红后绿
  - 完成：`get_board_article` 改返回 `Option`（缺失 / 非法标识 → `None` → `NotFound` 404；IO 错 → `LoadFailed` 500，不再把原始报错渲染给访客）；文章页走 `inject_seo`（`get_seo_base_url` 从 app 移到 `widgets::seo`，供模块 crate 使用，端点不变）；卡片与返回链接改 `Link`（浏览器验证 SPA 跳转不整页刷新）；文章头显示日期与标签。顺带修复：`cli/clap-derive-subcommands` 描述里未转义的 `"` 让 frontmatter 整体解析失败（列表标题为空、日期丢失），加了扫描真实 assets 的内容校验单测（先红后绿）
- [x] O3 — `widgets::PageMeta` + 列表 / 落地页元信息：首页、5 板块、案例列表与详情、课程列表与详情、播客、论坛、文档首页（E1）。验收：脚本 curl 上述路径 `<title>` 互不相同且非站名
  - 完成：`widgets::PageMeta { title, description, path }`（自带 SuspenseBoundary 取 `BASE_URL`；`<title>` 统一为 `{title} - 站名`，`page_title()` 也给文档页用）。接入首页、博客列表、文档首页、播客、课程列表 / 详情、案例列表 / 详情、论坛列表 / 话题详情、5 板块、生态页。顺带：案例 / 课程 / 生态页「不存在」改用 `NotFound` 回 404（原为 200），案例读取出错回 500。验收脚本 `scripts/check_page_titles.sh`（14 个路径 title 互不相同且非纯站名，退出码 0；注意 `LC_ALL=C`，否则 uniq 会把 CJK 标题判为相同）
  - 已知：SPA 导航后 `<head>` 里的 canonical 会累积（Dioxus head 元素卸载不移除，博客文章页原本如此）。SSR HTML 只有一个，不影响爬虫。重估条件：`curl -s $URL | grep -c 'rel="canonical"'` 大于 1
- [x] O4 — sitemap / feed 覆盖 docs、案例详情、课程详情（不含课时）；布局加 `rel="alternate"` feed 链接（E2）。验收：sitemap 条目数 = 各来源计数之和（单测）
  - 完成：`collect_detail_paths` 把文档（有正文的节点）、案例详情、课程详情加进 sitemap（不带 lastmod）。**不进 feed**：feed 是按日期的「新内容」，这三类没有日期，进 feed 只会是噪音（与方案原文「sitemap / feed」不同，有意收窄）。布局 `<head>` 加 Atom `rel=alternate`。sitemap 由 38 → 64 条。验收单测用真实 assets 独立计数比对（docs 数 = 磁盘 index 数；先红：镜像残留旧 `_meta.json` 时 7 ≠ 34）
  - 顺带发现并修复（单独提交）：`build.rs` 镜像只拷不删，dev 下仍读到已删的旧文档与 phase R 插件 wasm（fdd4d16）；Dockerfile 漏了 O1 新增的 `crates/board`（3161834）
- [x] O5 — 论坛与首页热帖区分「加载失败」与「空」，标签栏失败不再停在「加载中」；B6 结论不变（仍为 use_resource）（E8）。验收：停 `rie-postgres` 后页面显示失败提示，起库后正常
  - 完成：论坛列表 / 标签栏 / 标签页 / 我的话题与首页「论坛热帖」保留 `Result`，`Err` 显示「加载失败」（列表带重试按钮，`resource.restart()`），空列表才显示空状态；仍是 use_resource。浏览器验证：停 `rie-postgres` → 列表「加载失败，请稍后重试」+ 重试、首页「论坛暂时无法加载」；起库后 1 话题 2 标签正常
- [x] O6（技术部分；D2 内容待定） — 播客改 `use_server_future`；修复 `/audio` 在 dev 404（先查生产 Dockerfile 行为再定方案）；`audio_url` 校验文件存在，缺失时不渲染播放器（E6/E7）。需决定 D2（占位数据去留）。验收：SSR HTML 含节目标题；可播放的期能播放
  - 完成：播客改 `use_server_future`（SSR HTML 含 3 期标题与播放器；默认选中第一期改为渲染期计算，去掉 use_effect）；`Episode.url` 改 `Option`，本地 `/audio/…`、`/podcasts/…` 与相对路径只在文件存在时给出，缺失时显示「本期音频暂未上线」，不再给一个 404 地址（删掉了 `audio.mp3` 兜底；先写测试）；dev 下 `/audio` 404：生产 Dockerfile 把根 assets 整体拷到 /app/assets，生产不受影响，问题只在 dev 镜像跳过 `audio/`，`build.rs` 改为在镜像里软链回根 `assets/audio`。浏览器验证可播放、切换节目正常
  - D2（作者确认为演示数据，9a3a2c6）：删去与第一期共用音频的两期；保留的一期时长改为实测 19:38，去掉占位嘉宾，日期改为音频文件 mtime 2026-04-13
- [x] O7 — 案例卡无 cover 时不渲染封面区；板块子分类 chip 只显示有文章的（E9/E10）。需决定 D4
  - 完成（D4 按推荐：无封面不渲染；D1 只做 chip 收敛，不加「持续更新中」文案）：案例列表卡、详情页、首页精选三处没有 cover 时不画封面区（详情改单列；「精选」角标挪进标签行）；删掉不再使用的 `placeholder_gradient`。板块 chip 只显示有文章的子主题（`Board::subtopics_with_articles`，先红后绿），/embedded 由 7 个降为 3 个。顺带：`stars` 全为 0（未采集），列表 / 详情 0 时不再显示「★ 0」，与首页一致；删掉无引用的 `case.added_on` 文案
- [x] O8 — 导航与页脚：桌面导航加播客；手机站名不截断；页脚删重复标语、加「专题」列与 RSS（E11/E12）。验收：390 / 1280 两种宽度截图
  - 完成：桌面导航加「播客」（受开关）；<sm 时主题 / 语言收进移动抽屉、登录只留图标（带 aria-label），站名去掉 `max-w-32 truncate`——390px 下完整显示（141px，无横向滚动），抽屉内切语言验证可用；页脚删底部重复标语（改为 `© Rust in Everything`），新增「专题」列（5 板块，受开关）与社区列 RSS（`a href=/feed.xml`，服务端路由不走 Router）。截图：390 顶栏 / 抽屉、1280 导航 / 页脚
- [x] O9 — 首页删除「按领域浏览」区块（E13）。需决定 D3
  - 完成（D3 按推荐：删除）：首页去掉「按领域浏览」11 卡网格及其 `ModuleCard` / `module_icon` / `SectionTitle`（已无其他调用）与 `home.browse.*` / `home.mod.*` 文案。入口由生态卡、导航 mega 菜单、页脚「专题 / 内容」列覆盖。重估条件：首页去掉生态卡或导航 mega 菜单时，入口需要另有承载
- [x] O10 — 搜索：弹窗文案接入 i18n，筛选补 5 个板块，课程标题 / 简介进索引（E14）
  - 完成：弹窗与导航搜索按钮文案接入 i18n（`search.*`，中英）；筛选补课程与 5 个板块（板块用 `{board}.label`；服务端 `normalize_kind` 原来只认 4 种，板块筛选根本不生效，一并修，`BOARD_KINDS` 提到 crate 根共用）；课程经 `doc_source` 注册表进索引（只收标题 / 简介 / 标签，与 cases 同一 IoC 方式）；徽章补 COURSE、嵌入式改 EMBEDDED 与其余一致；搜索失败不再把原始错误渲染给访客。顺带修复（单独提交）：英文检索大小写敏感（"candle" 搜不到 Candle 案例），分词器加 LowerCaser 并改名触发存量索引重建
  - 注意：索引在启动时只复用、不重扫（设计如此，见 OPERATIONS.md）；已部署环境要看到课程结果，需 admin 调一次 `POST /api/search/reindex`——本次分词器改名会让存量索引在下次启动时自动全量重建，所以这次部署无需手动操作
- [x] O11 — 其余硬编码文案接入 i18n：博客 / 文档 / 课程不存在提示，播客 6 处（E15）。验收：切到 EN 后上述页面无中文残留
  - 完成：`widgets::LoadFailed` 改为调用方传文案（widgets 无 i18n）；博客 / 文档（首页、卡片、侧栏、不存在）/ 课程（列表、详情、目录、不存在）/ 播客（筛选、翻页 aria、PodcastCard 3 处）/ 评论区（10 处）/ 关联讨论面板（4 处）接入中英文。浏览器 EN 验收 9 个路径：剩余中文只有内容本身（文档 / 课程 / 播客数据）与下面刻意不做的支付弹窗
  - 有意不做（当前版本）：课时页、付费墙、标注、`pay_ui`（购买弹窗 / 我的订单）仍为中文（2026-10-20 重估条件触发：Stripe 上线，付费墙与 `pay_ui` 已在 I5 翻译）——支付只有支付宝 / 微信、以人民币计价，面向中文用户；课时内容本身也是中文。重估条件：出现非人民币支付渠道（`grep -rn 'provider ==' crates/modules/course/src/pay_ui.rs` 出现 alipay / wechat 之外的值），或 `assets/courses/*/` 下出现英文课程（`course.yaml` 含 `lang: en`）

### 阶段 L — 功能分层（Cargo features，2026-10-20）
设计与证据见 [`docs/FEATURE_TIERS.md`](docs/FEATURE_TIERS.md)。四层：basic（无 feature）→ `accounts` → `course` → `payments`；`tips` 待 DL6 确认。默认构建（`web` + `payments`）行为不变。作者未答复的三个问题按推荐处理：做成通用框架（basic 不需要数据库）、标注归课程层（DL3）、打赏需登录且待确认后开工（DL6）。
- [x] L1 — 去掉 cases / course 对 blog 的无用依赖；案例详情 `DiscussionPanel` 改由 app 经 `footer` 插槽注入。验收：`cargo tree -p module-cases` 无 forum / blog；案例详情 SSR HTML 不变
  - 完成（d77cdbf）：`CaseDetailPage` 加 `footer` 插槽，讨论面板由 app 注入，只在案例存在时渲染；cases 删 forum / blog 依赖，course 删 blog 依赖（两者代码都没引用 blog）。`cargo tree` 确认无残留；`/case`、`/case/bat`、`/docs` SSR HTML 与改前相同（只差 hydration 调试信息里的源码行号）
- [x] L2 — `register_compiled_modules`：编译进来的模块登记进 ModuleEngine；hero / 生态菜单的课程、案例、文档链接按启用列表显示。验收：单测
  - 完成（c14751a）：core 加 `register_compiled_modules`，`default_module_specs` 只返回登记过的（未登记 = 全部；纯函数 `only_compiled` 单测，故意不过滤时变红）；app `main` 两端登记。三份「拉启用列表」合成 `use_enabled_modules`。按启用列表显示：hero 三个按钮、mega 菜单「学习资源」与精选 CTA、首页精选案例 / 课程区、社区动态两栏、生态页领域与案例区。浏览器验证（site.json 关 course / cases / forum）：首页与 `/ecosystem/rust` hydration 后指向这三者的链接为 0，社区动态只剩博客栏。注意：SSR 首帧仍按「编译进来的全集」渲染、hydration 后收敛（导航原本如此）；L3 起编译期关掉的模块在 SSR 首帧就不出现
- [x] L3 — `course` 层（先切课程：课程依赖登录，先切 accounts 会出现「有课程无登录」的非法组合）：课程 / 标注 / 订单 / 权益后台 / `/courses` 鉴权 / sitemap / 搜索 / 首页课程区按 feature。验收：`accounts` 构建运行时 `/course` 404、博客无标注
  - 完成（7a1f755）：`module-course` 改可选依赖，`course` feature，`payments` 隐含 `course`，默认构建不变。课程页、我的标注 / 订单、权益后台移到 `routes/course.rs`；未编译时 `absent_pages!` 生成同名 404 组件（`Route` 各层一致，DL1）。`/courses` 静态目录、sitemap 课程项、课程进搜索、标注层、首页课程区、用户菜单「我的标注」、管理后台「课程权益」、搜索「课程」筛选都按编译结果出现。顺带：search 两处重复的 kind → 模块映射合成 `module_of_kind`。验收（`dx serve --web --no-default-features`）：`/course`、`/me/orders`、`/admin/entitlements` 404，`/api/courses/list` 未注册（405），`/courses/*` 不提供，博客无标注按钮、搜索无「课程」；完整构建同样路径 200、标注按钮与「课程」筛选在，`check_page_titles.sh` 通过；工作区测试与两档 clippy 通过。注意：dx 要用 `--web` 指定平台，否则 `--no-default-features` 时报 “Could not automatically detect target triple”
- [x] L4 — `accounts` 层：论坛 / 评论 / 上传 / 管理后台可选依赖，页面 404 桩，登录入口与启动校验按 feature。验收：basic 不配 `DATABASE_URL` / `JWT_SECRET` 能启动并浏览内容页，`/topics` `/admin` 404；完整构建不变
  - 完成（06ab3dc）：论坛 / 评论 / 管理后台 / 上传改可选依赖，`accounts` feature，`course` 隐含它，默认构建不变。basic 层：启动不校验 `JWT_SECRET` / `DATA_ENCRYPTION_KEY`、不连库不迁移（`/healthz` 报 `db=disabled`，原设计即视为正常）；不注册登录路由与 auth server fn，无登录按钮 / 弹窗；论坛与管理后台路由 404（`routes/accounts.rs` + `absent_pages!`）；文档 / 博客 / 案例的讨论面板与博客评论区不渲染。顺带修两处 basic 才暴露的问题：搜索在论坛关闭时仍去连库取话题（改为先看模块开关）；basic 没有管理员能调 `/api/search/reindex`，启动时做一次增量刷新（实测旧索引落后 17 篇）。`verify_jwt` 在未配 `JWT_SECRET` 时返回错误而不是在请求里 panic（伪造 session cookie 能走到这条路径）。验收：停 `rie-postgres`、`JWT_SECRET` / `DATABASE_URL` 置空跑 basic，内容页 200，论坛 / 管理 / 课程 404，论坛 API 未注册，伪造 cookie 返回错误、日志无 panic，浏览器无登录 / 评论 / 讨论 / 「话题」筛选；完整构建对照：同样路径 200、评论 / 讨论 / 登录都在，标题脚本通过，工作区测试与 clippy 通过
  - 注意：本地两档共用 `crates/app/data/search-index`，basic 启动刷新会删掉话题 / 课程条目；切回完整版前删掉该目录让它全量重建。生产只部署一档，不受影响
- [x] L5 — About 页：`/about` 渲染 `assets/pages/about.md`，不存在时 404，页脚按存在与否显示
  - 完成（1cce4d9）：`/about` 读 `assets/pages/about.md`，frontmatter 的 title / description 经 `PageMeta` 进 head（`关于 - Rust in Everything`），文件不存在 404、读失败 500。`about` 作为内置模块登记：页脚「社区」列链接与 sitemap 条目随 site.json 开关。默认内容只描述站点覆盖的内容，需作者按需改写。顺带发现：`widgets::Markdown` 会用 frontmatter 标题覆盖页面 `<title>`，所以 About 传入去掉 frontmatter 的正文（博客文章页的 `<title>` 也因此不带站名）。后续 bd37daa：`inject_seo` 与 `Markdown` 都改走 `page_title()`，文章 / 文档 / 板块文章标题都带站名，About 不再需要去 frontmatter；`check_page_titles.sh` 加博客文章并要求每页带后缀（改前在 `/blog/welcome` 变红）。验收：200 / 标题 / description / sitemap 1 条 / 页脚链接；移走文件 404、放回 200；标题脚本通过
- [x] L6 — 分层门禁 `scripts/check_tiers.sh`（四档 × server / web 两端 `cargo check`）接入 CI，Dockerfile `ARG FEATURES`，文档。验收：反向验证（故意加一处无门控引用，脚本失败）
  - 完成（5c3343d；前置 78be668 清掉 web 端 5 个既有 dead-code warning，门禁才能把 warning 当失败）：`scripts/check_tiers.sh [档位…]` 对 basic / accounts / course / full 各跑 server、web 两次 `cargo check -p app`，有 error 或 warning 即退出 1（本机全量约 33s）。CI 用它替换原先的完整版 + 两条 no-payments 检查。反向验证：无门控 `use module_forum` → basic 两端 error 变红；去掉课程专用 import 的 cfg → accounts/server 因 unused import 变红；还原后全绿。Dockerfile 加 `ARG APP_FEATURES` 传给 `dx bundle`；README「功能分层」表 + 提交前检查、DEPLOY_GUIDE 档位构建说明
- [x] L7 — 测 basic 与完整版的二进制大小 / 依赖数 / 冷构建时间，决定是否拆 app-core（DL5），结论写回设计文档
  - 完成：server 端依赖 515（完整）/ 488（basic）/ 382（basic 若拆 core 并去掉 migration，`cargo tree --prune` 估算）；release 二进制 50.0 MB / 30.9 MB，basic 里 sea-orm / sqlx / migration 约 2.3 MB、jwt / oauth2 / reqwest / rustls 约 0.6 MB（`nm` 符号间距估算）。结论：当前版本不拆 core——basic 已能无库运行，收益只有约 22% 依赖数与约 3 MB，代价是 core 两个新 feature + search 重建索引接口门控 + migration 依赖改走 core。重估条件（可执行）与数据表写在 FEATURE_TIERS.md DL5
- [x] T — 打赏（见 FEATURE_TIERS.md §6，DT1–DT5；2026-10-20 作者确认：登录、固定档位 ¥5/10/20/50，海外 $1/3/5/10）
  - [x] T1 — 订单 `course_slug` → `product_ref` + `product_kind`；`on_paid` 收快照，`deliver()` 按种类分派；退款按种类撤权益
    - 完成（ad65a9a）：迁移 000009（改名 + 加列，存量默认 course）；`course::product::ProductKind`（`grants_entitlement`）；`pipeline::process_event` / `reconcile_order` 的 `on_paid` 改收 `OrderSnapshot`，三个回调 + 对账共用 `deliver()`（未知种类返回错误）；退款只对课程撤权益；`OrderInfo` / `AdminOrderInfo` 带种类与 `product_ref`（未知种类留痕后不列出）。验证：本地库迁移后 5 条旧订单均为 course，`/api/orders/mine` 与后台列表返回新字段；工作区 615 测、四档门禁、clippy 通过。`deliver()` 的未知种类分支要连库，没有单测，靠 `ProductKind::parse` 单测覆盖
  - [x] T2 — `create_tip_order` + 档位 + 来源校验 + `OrderRequest.return_path`
    - 完成（f261cfc）：`product::{TIP_TIERS, tip_pricing, is_tip_source}`，报价直接走 `Pricing::quote`；`/api/orders/tip`；课程与打赏共用 `checkout_preamble` / `start_payment`；支付宝 `return_url`、Stripe `cancel_url` 改为订单的 `return_path`（课程 `/course/<slug>`，打赏回原页面）。单测：4 档 × 地区 × 网关报价、来源白名单（含 `/blog/../x`、外链）、Stripe cancel_url、支付宝 return_url；去掉 slug 校验或改报价规则时对应测试变红。本地实测：Stripe 海外第 2 档建单 $3（product_ref `/blog/welcome`），档位 9 / 外链来源 / 未登录被拒，课程下单不变
  - [x] T3 — 打赏卡片（博客文章末尾、播客页）+ 弹窗支持打赏 + 我的订单显示 + 中英文案
    - 完成（52313be）：`pay_ui::TipJar`（无网关不显示；未登录点按钮开登录弹窗，登录状态由 app 的 `TipCard` 传入）；`PurchaseForm` 改收 `Item`（课程 / 打赏），打赏多一排档位、按地区显示 ¥ 或 $，付款后显示感谢不刷新；我的订单「内容」列显示「打赏 · 来源页」；`PodcastPage` 加 `footer` 插槽；`pay.tip.*` 中英文案。浏览器验证：未登录卡片 + 提示 + 登录弹窗；登录后弹窗 ¥5/10/20/50，选 ¥20 金额跟着变，切到海外变 $1/3/5/10；播客页有卡片；我的订单显示打赏行；控制台只有已知的 dx 字体与 B 站嵌入报错。注意：工作区测试要 `--test-threads=1`（widgets 组件注册表是全局的，并行时偶发失败，CI 已这样跑）
  - [x] T4 — 管理后台订单列表显示种类与来源
    - 完成（e9b76df）：`/admin/entitlements` 订单表「课程」列改为「内容」，显示「课程 · slug」/「打赏 · 来源页」；打赏同样可全额退款（DT5：不另做打赏列表）。浏览器验证：管理员看到课程行与 `/blog/welcome` 的 $3 打赏行
  - [x] T5 — Stripe 测试模式端到端（海外 $3、大陆 ¥10、退款一笔打赏）
    - 完成（2026-10-20，顺带 8291784 改后台退款说明）：用户 1 海外在 `/blog/welcome` 打赏 $3，Stripe 收银台显示「Tip · Rust in Everything $3.00」，测试卡 4242 付款后回到我的订单显示已支付，回调写入 `pi_`，没有生成权益；用户 2（已购课）大陆在 `/podcast` 打赏，收银台「打赏 · Rust in Everything CN¥10.00」，点 Back 回到 `/podcast`（cancel_url 生效），再付一次成功；管理员退这笔 ¥10，Stripe 退款 succeeded（带 out_trade_no），本地订单 refunded，用户 2 的 rust-basics 权益仍在。未验证：支付宝 / 微信直连打赏（无商户号）
- 打赏（T）完成：T1–T5

### 阶段 I — 海外支付（Stripe + 按地区给出支付方式，2026-10-20）
设计见 [`docs/PAYMENT_SPEC.md`](docs/PAYMENT_SPEC.md) §15。作者决定：接 Stripe；课程另设美元价，海外按美元付。打赏（T）继续留在待办。
- [x] I1 — `price_usd` 字段 + `Region` + `quote(course, provider, region)` + `methods_for(region, configured)`。验收：单测覆盖大陆 / 海外 × 三个网关 × 有无美元价
  - 完成（e23e396）：`course::pricing` 模块：`Region`（时区判断，港澳台按海外）、`Price::label`（`¥99` `¥99.50` `$14.99`，原来按整元截断）、`Pricing::{list_price, quote}`、`methods_for`；`course.yaml` 加 `price_usd`，课时改带整份 `Pricing`。单测 7 个（含读 yaml）；把 Stripe 报价改成忽略地区时 `quote_matrix` 变红。示例课程设 `price_usd: 1499`
- [x] I2 — `StripeProvider`（`stripe` feature）：Checkout Session 下单、webhook 解析、PaymentIntent 搜索查单、退款；`verify_signature`。验收：单测（独立 HMAC 向量；篡改 body、过期时间戳必须失败）
  - 完成（726c21c）：`module-payment::stripe`（默认开启的 `stripe` feature，依赖 hmac + sha2）。回调只有 `checkout.session.completed` / `async_payment_succeeded` 且 `payment_status == paid` 才算成功；查单无结果 → `not_found`（对账超窗关单）；退款要 PaymentIntent id，`RefundRequest` 加 `txn_id`。单测 8 个；验签向量由 Python 独立算出；删掉时间戳检查或让 HMAC 比较恒真都会让测试变红
- [x] I3 — 接线：`create_order(region)`、`payment_providers()`、对账 / 退款分派、`/api/pay/stripe/notify`、订单带币种、`.env.example`。验收：未配 Stripe 时行为与现在一致；用测试签名构造的 webhook 走通流水线
  - 完成：`host::from_env(name)` 取代下单 / 对账 / 退款三处 `match provider`（host 执行器改收 `?Sized`）；`create_order` 加 `region`，金额与币种取自 `Pricing::quote`，并在建单前确认网关已配置（原来未配置时会留下一张 pending 单）；`payment_providers()`；订单列表（我的 / 管理）带币种。本地实测（假密钥 + 自签 webhook）：错 secret / 过期时间戳 400，无关事件与 unpaid 200 不发货，金额不符 500 不发货，paid → 订单 paid、存 `pi_` 流水号、写 purchase 权益，重放幂等；测试数据已清理。四档检查、clippy、613 个测试通过
- [x] I4 — 界面：时区判断地区，课程详情 / 付费墙 / 首页课程卡 / 购买弹窗按地区报价与列支付方式，可手动切换；我的订单按币种显示。验收：浏览器模拟两个时区截图
  - 完成（c0b95b9）：`pricing::use_region()`（全局信号；SSR 与首帧按大陆默认，hydration 后读 localStorage 的手动选择，没有则按 `widgets::browser::time_zone()`）+ `set_region`。课程详情、付费墙、购买按钮、首页课程卡按地区报价；弹窗列 `methods_for(地区, 已配置网关)`，金额按所选网关的 `quote`，底部「🌐 海外 · 切换到中国大陆」；我的订单与管理订单按订单币种显示金额与渠道名。Chromium 实测：Asia/Shanghai → ¥99，America/New_York → $14.99；切换后价格与方式随之变化，刷新后保留。本地只配了 Stripe，所以大陆也只列出「银行卡 / 国际支付」（三网关排序见单测）
- [x] I5 — 购买弹窗与我的订单 i18n（阶段 O 的重估条件已触发）。验收：EN 下弹窗与订单页无中文
  - 完成（10d8286）：购买弹窗、付费墙、「在线购买未开通」提示、我的订单接入中英文；`create_order` / `query_order` 面向用户的错误改返回 `pay.err.*` key，弹窗经 `t()` 翻译（非 key 的网关原文原样显示）。`site_lang=en` 实测：弹窗、付费墙、订单页无中文，未登录点支付显示 “Please sign in to buy.”。仍为中文（当前版本不做）：课时页外框（如「← 返回课程目录」）与标注——课时内容本身是中文；网关返回的错误原文（如「Stripe 下单失败: …」）面向运维
- [x] I6 — Stripe 测试模式端到端（下单 → 支付 → 回调发货 → 对账 → 退款）。**需要作者提供 `sk_test_` 密钥与 webhook secret**
  - 完成（2026-10-20，本地 dx + Stripe 测试模式，`stripe listen` 转发 webhook）：纽约时区英文界面购买 → 跳 Stripe Checkout（$14.99）→ 测试卡 4242 付款 → 回到我的订单显示 Paid，课程显示已拥有、锁定课节解锁；订单 USD 1499、存 `pi_` 流水号、写 purchase 权益，webhook 应答 200。停掉 webhook 转发后再付一单：订单停在 pending，约 2 分钟后对账经 PaymentIntent 搜索回填并发货。管理员退款：Stripe 上退款 succeeded（1499 usd，metadata 带单号），本地订单 refunded、purchase 权益撤销，重复退款被拒。未付款的单对账时保持 pending，无告警。尚未验证：支付宝 / 微信渠道下的 Stripe（需在 Stripe 后台开通）与生产域名的 webhook

### 阶段 D — 上线配置补全（2026-10-20 重估）
重估结论：功能侧没有现在该做的（M3、M6 见上，均写了重估条件）。剩下的上线阻碍是外部条件（商户号、公网域名、Stripe 正式 webhook），但照着仓库文档部署时有两处会卡住，属于现在能修的：
1. **支付宝 / 微信的环境变量没有写进部署文档。** `.env.example` 只有 Stripe 段；`PAYMENT_SPEC.md` §8 漏了 `WECHAT_PLATFORM_PUBLIC_KEY`（代码必填，缺了微信不启用）；`DEPLOY_GUIDE.md` 完全没提支付（回调地址、Stripe webhook 事件、后台开通支付宝 / 微信）。
2. **配置错了没有任何提示。** 三个网关的 `config()` 在缺项或值非法（如 `WECHAT_API_V3_KEY` 不是 32 字节、密钥不是 base64）时都只返回 `None`，启动日志不说哪些网关启用了，下单时只报「未配置」，运维无从知道是哪一项。
- [x] D1 — 启动时按网关打一行日志：已启用 / 未配置（相关变量都没设）/ 配置有误（设了一部分，列出缺的变量名；都设了仍失败则说值无效）。只输出变量名，不输出值。验收：单测覆盖三种状态；本地启动日志能看到 stripe 已启用
  - 完成（0920338）：`host::config_status()` / `log_config_status()`，各网关在 `config()` 旁声明 `REQUIRED_ENV`；app 启动时调用（payments feature）。单测 2 个：四种状态；`REQUIRED_ENV` 与 `config()` 实际读取的变量一致（删掉 `WECHAT_PLATFORM_PUBLIC_KEY` 或让共用的 `PAY_NOTIFY_BASE` 算作已配置都会变红）。本地 dx 实测：正常为 alipay / wechat 未配置、stripe 已启用；临时只设 `ALIPAY_APP_ID` → warn 列出缺的两个密钥变量；微信六项都设但 APIv3 密钥长度不对 → warn 值无效。619 个测试通过，clippy 与四档检查通过
- [x] D2 — 文档：`.env.example` 补支付宝 / 微信段；`PAYMENT_SPEC.md` §8 补 `WECHAT_PLATFORM_PUBLIC_KEY` 与密钥格式；`DEPLOY_GUIDE.md` 新增「在线支付」一节（features、回调地址、Stripe webhook、上线前检查）。验收：文档里的变量名与代码中读取的完全一致（`grep` 对比）
  - 完成（0cbaa55）：`.env.example` 支付段重写（原 Stripe 段错放在 LLM 设置下，移回来；补支付宝 / 微信全部变量、密钥格式、32 字节约束）；`PAYMENT_SPEC.md` §8 补 `WECHAT_PLATFORM_PUBLIC_KEY`，标题里不实的「启动期 assert_not_placeholder 校验」改为启动日志；`DEPLOY_GUIDE.md` 新增 §7 在线支付（各网关变量与回调地址、启动日志怎么看、Stripe 正式模式 webhook 与开通支付宝 / 微信、上线检查），原 §7–§12 顺延，安全 checklist 加一行。三份文档里的变量名与 `crates/modules/payment/src` 读取的逐一比对一致
- 阶段 D 完成：D1–D2。余下上线事项都要外部条件：支付宝 / 微信商户号与真实付款验证、生产域名与 Stripe 正式 webhook、公网环境的 CSP / healthz / XFF 检查、关于页文案

### 阶段 C — 默认配色改为石墨 + 青色（2026-10-20）
作者在四套候选（现状橘色 / 石墨 + 青色 / 板岩 + 靛蓝 / 石墨 + 深铁锈红）的亮暗截图里选了石墨 + 青色。现状的问题：
1. **整站偏暖。** 中性色是 stone（暖灰），主色 orange-600 饱和度高，观感偏生活类站点；科技感主要来自冷灰中性色。
2. **默认主题名不副实。** 主题菜单显示 "Ocean"，实际是橘色。换成青色后名字正好对上，id 与 label 不改（site.json 与访客 cookie 照旧有效）。
3. **换主题换不干净。** `tailwind-input.css` 把所有 `blue-*` 类映射成橘色，另有几处手写的橘色 / 蓝色强调。这些不跟随 `--primary`，切到 Catppuccin 等主题后仍是橘色。

设计：
- 中性色 stone → zinc：`tailwind-input.css` 的 `slate-*` 映射改指 zinc，`:root` / `.dark` 默认 token 与 `ocean.css` 的中性色取 zinc 同一档位（档位不变，只换色阶）。
- 主色：亮色 cyan-700（白字，对白底 ≥ 4.5:1），暗色 cyan-400（字用 zinc-950）。ring、chart-1、sidebar-primary 同步换。
- 删掉 `blue → orange` 映射，`blue-*` 恢复为真正的蓝色：NOTE 提示框、标注色板里的「blue」本来就该是蓝色。
- 手写的「强调色」改用 `primary`（标题悬停、案例详情分类标签、正文链接）。分类徽章（案例分类、搜索结果类型）是分类色，不是强调色，保持原样；amber 用作警告提示的也保持原样。
- sunset / catppuccin 两个主题不动。手写的 `slate-*` 类在这两个主题下会从 stone 变成 zinc，差别很小，可以接受。

- [x] C1 — 默认 token、`ocean.css`、中性色映射改为 zinc + cyan，删掉 blue→orange 映射；`theme.rs` 单测改为断言 zinc / cyan；`THEME_SPEC.md` 同步。验收：单测在旧值下变红；重建 Tailwind 后课程页主按钮亮色为 cyan-700、暗色为 cyan-400
  - 完成（cbeba1c）：`tailwind-input.css` 默认 token 与 `ocean.css` 中性色改 zinc、主色亮 cyan-700 / 暗 cyan-400（暗色字 zinc-950），ring、chart-1、sidebar 同步；`slate-*` 映射改指 zinc，删掉 blue→orange 映射。`theme.rs` 两个单测改断言 zinc / cyan，旧值下变红。dx 实测课程页主按钮亮色 oklch(52% 0.105 223) / 暗色 oklch(78.9% 0.154 211.5)。`THEME_SPEC.md`、`TAILWIND_GUIDE.md`、README 同步
- [x] C2 — 手写强调色改用 `primary`（`routes/mod.rs` 标题悬停、`cases.rs` 详情分类标签、`mdx.rs` 正文链接）。验收：仓库里除分类徽章和警告提示外，不再有 `orange-*` / `blue-*` 的手写强调色
  - 完成（137451a）：三处改为 `text-primary` / `bg-primary/10` / `decoration-primary`。剩下的 `orange-*` / `blue-*` 都是分类色（案例分类、搜索结果类型）、标注色板的「blue」和 NOTE 提示框。顺带修了一个已有问题（4526a64）：Tailwind 没有扫描 `crates/widgets` 与 `crates/board`，正文里的 NOTE / TIP / WARNING 提示框和链接样式从未生成 CSS，现已加入 `@source`
- [x] C3 — 走查：首页、生态页、课程详情 / 课时、案例、博客正文、搜索、论坛、购买弹窗、管理后台，亮暗各截图；检查主按钮、链接、焦点环对比度。验收：无残留橘色强调，暗色下主按钮文字清晰
  - 完成（2026-10-20）：首页、生态页、课程详情、课时、案例列表 / 详情、博客正文、论坛、我的订单、管理后台订单，亮暗各截图，并用脚本扫描页面上色相 20°–50° 的可见元素：剩下的只有代码高亮（Prism）与分类徽章，没有橘色强调。对比度：亮色白字 / cyan-700 5.28:1，暗色 zinc-950 字 / cyan-400 11:1，cyan-400 链接在 zinc-900 卡片上 9.8:1，都过 WCAG AA（原橘色按钮白字只有 3.58:1）。暗色截图里标签发灰是 150ms 颜色过渡没走完，稳定后为白字 zinc-800 底。clippy、619 个测试、四档检查通过。未走查：购买弹窗（与课程页同用 primary token）
- 阶段 C 完成：C1–C3

### 阶段 H — 支付与安全加固（2026-10-20 审计）
评估与设计见 [`docs/AUDIT_2026-10-20.md`](docs/AUDIT_2026-10-20.md) §2；不做的项与重估条件见 §4。
- [x] H1 — 退款订单不得被成功回调改回 paid：`claim_paid` 只认领 `pending` / `closed`，`decide` 对 `refunded` 返回 `AckRefunded`。验收：单测在旧逻辑下变红
  - 完成（b162734）：新增 `NotifyDecision::AckRefunded`（确认收到 + pay_audit 告警，不认领不发货）；认领条件 UPDATE 由 `!= paid` 改为 `IN (pending, closed)`。单测 `refunded_order_acks_without_claim_or_delivery` 在旧逻辑下编译失败 / 会认领并发货。`PAYMENT_SPEC.md` §9 同步
- [x] H2 — Stripe `charge.refunded`（全额）与 `charge.dispute.created` → 订单 `paid → refunded` + 撤销 purchase 权益；与管理员退款共用 `mark_refunded`；DEPLOY_GUIDE / PAYMENT_SPEC 补 webhook 事件。验收：本地自签 webhook 实测两种事件撤销权益，重放幂等，部分退款不撤销
  - 完成（ae6bcb9）：`stripe::reversed_payment_intent`（纯函数，单测覆盖全额 / 部分退款 / 拒付 / 无关事件）；`handle_stripe_notify` 先分流到 `stripe_reversal`，按 `provider_txn` 找订单后调用从管理员退款抽出的 `mark_refunded`。本地 dx + 自签 webhook 实测：部分退款 200 不变；全额退款 → 订单 refunded、purchase 权益删除；重放 200 无变化；拒付同样撤销；未知 PaymentIntent 200 + 告警。测试数据已清理。`DEPLOY_GUIDE.md`、`PAYMENT_SPEC.md` §15、`.env.example` 的 webhook 事件补上两项
- [x] H3 — 购买发货冲突时不覆盖已有权益来源（保护 admin_grant）。验收：先赠后买再退款，权限仍在
  - 完成（015aca9）：`grant_entitlement_internal` 对 `purchase` 用 `ON CONFLICT DO NOTHING`（`exec_without_returning`，冲突不报错），管理员授予仍覆盖。本地自签 webhook 实测：已有 admin_grant 的课付款后来源仍为 admin_grant，退款后权限保留；无权益的课付款写 purchase，退款后删除；退款后重放成功回调订单仍为 refunded、不再发货（顺带端到端验证 H1）。未做旧代码下的反向运行：旧代码 `update_columns(Source)` 会把来源改成 purchase，退款按 source=purchase 删除
- [x] H4 — `/api/orders/create`、`/api/orders/tip` 归入严格限流桶。验收：`classify_path` 单测
  - 完成（cf69a20）：`classify_path` 的严格前缀加上两条下单路径（默认 5 rps / 15 突发，每 IP）。单测新增三条断言，旧代码下变红；`/api/orders/mine` 仍走默认桶
- [x] H5 — compose 里应用端口只绑 127.0.0.1，DEPLOY_GUIDE 说明。验收：`docker compose config` 输出 host_ip 127.0.0.1
  - 完成（fba9c59）：ports 改为 `${APP_BIND:-127.0.0.1}:${APP_PORT:-8080}:8080`（网关上游本来就是 127.0.0.1:8080）；`.env.example` 加 `APP_BIND` 说明；`DEPLOY_GUIDE.md` §11 安全 checklist 加一行。本机没有 docker，未跑 `docker compose config`，只用 YAML 解析确认了端口串
- [ ] H6 — 依赖升级到修复版本（rustls ≥0.23.46、h2 ≥0.4.16、quinn-proto ≥0.11.18 等）。验收：`cargo audit` 只剩无修复版本的条目
- [ ] H7 — YouTube / Bilibili 嵌入 id 只允许 `[A-Za-z0-9_-]`。验收：单测，带 `?` / `&` / `/` 的 id 不渲染 iframe

### 阶段 G — SEO 与加载性能（2026-10-20 审计）
评估与设计见 [`docs/AUDIT_2026-10-20.md`](docs/AUDIT_2026-10-20.md) §3。英文独立 URL 不做（§4，内容全为中文，带可执行重估条件）。
- [ ] G1 — 课节页加 `PageMeta`（标题、描述、canonical），不存在的课节返回 404。验收：curl 实测
- [ ] G2 — `<html lang="zh-CN">`。验收：SSR 输出含 lang
- [ ] G3 — 头部脚本 `defer`，mermaid 只在页面出现 `.mermaid` 时加载。验收：无图表页不请求 mermaid；含图表与代码的页面仍正常渲染
- [ ] G4 — 响应压缩（tower-http `CompressionLayer`）。验收：带 `Accept-Encoding` 时 HTML / CSS 返回压缩
- [ ] G5 — 静态资源缓存头：先在 release 下测，带哈希的资源若不是长缓存再改。验收：release 实测记录
- [ ] G6 — sitemap 补 `/ecosystem/*` 与免费试看课节。验收：sitemap 含这些 URL
- [ ] G7 — `/blog` 加 h1、案例详情正文标题降级、尾斜杠 301、Atom 日期改 RFC 3339。验收：curl 实测
- [ ] G8 — JSON-LD：课程详情 Course、首页 WebSite。验收：SSR 输出含对应 `@type`，JSON 可解析
