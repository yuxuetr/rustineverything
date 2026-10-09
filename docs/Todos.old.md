# 开发计划

> 本文档与 `RoadMap.md` 配套：路线图给「为什么 / 做什么」，本文档给「具体要写哪些代码」。
> 每个 Phase 的子任务完成后勾选 `[x]`，未完成保持 `[ ]`。

## Phase 0 — 已完成基线 ✅

> 历史已交付能力的紧凑摘要；细节请查 `docs/*_SPEC.md` 与 git log。

- ✅ Session / JWT / Cookie / 全局用户上下文
- ✅ 评论系统迁移到 PostgreSQL（comments 表 + SeaORM）
- ✅ **MDX 渲染管道已稳定**（`crates/modules/blog/src/markdown.rs`）：frontmatter / GFM / 数学 / Mermaid / 代码 + Copy / 标注 block-id / 7 嵌入组件（PodcastCard / YouTube / Bilibili / Yellow / Green / Blue / Pink / Purple / Underline / Strikethrough）
- ✅ 文档系统 `/docs`：自动扫描三级嵌套、frontmatter、`sidebar_label/position`、`sort_children`，15 测试
- ✅ Podcast 动态化：YAML 元数据 + 自动音频探测，18 测试
- ✅ 课程系统 `/course`：Course→Chapter→Lesson 三级、`Doc|Video|Audio|Code` 自适应、进度跟踪
- ✅ 标注系统 v2（5 色 / 4 visibility / `data-block-id` 回放 / 个人列表页 / 眼睛 toggle），15 测试
- ✅ 论坛 `/topics`：发帖/回复 + 标签 + 资源引用 `ref_kind/ref_path`，18 测试
- ✅ DiscussionPanel 接入 blog / doc / lesson 底部
- ✅ Admin 后台：概览 / 用户角色 / 评论删除 / 话题删除 / 插件列表 / 5 页面 + AdminShell
- ✅ 全站搜索：Tantivy 0.26 + jieba + Cmd+K modal + kind 徽章 + 4 索引源，34 测试
- ✅ 案例展示 `/case`：网格 + 一级分类 + 标签侧栏 + Issue Form 贡献入口，12+ 测试
- ✅ 6 WASM 插件：github/google/discord/twitter auth + theme-ocean + i18n-fluent

---

## Phase 1 — 引擎层重构（8 引擎）

### 1.1 引擎抽象基础
- [ ] `crates/core/src/engines/mod.rs`：`Engine` trait（`name / init / shutdown`）
- [ ] `EngineRegistry`：注册 + 按名称取出 + 生命周期管理
- [ ] `EngineContext`：DB pool / SiteConfig / PluginManager 句柄打包
- [ ] 单测：注册多引擎、重复名报错、shutdown 顺序

### 1.2 PluginEngine（取代 PluginManager，基于 wasmi）
- [ ] `engines/plugin.rs`：基于 **wasmi**，`Module` 缓存 `HashMap<PathBuf, (mtime, Arc<Module>)>`
- [ ] 失效策略：mtime 变更或 admin 显式 reload
- [ ] 能力协商：plugin manifest 声明 capabilities，host 仅在能力匹配时调用
- [ ] ABI 版本：`SDK_ABI_VERSION: u32 = 1`，不匹配拒绝加载
- [ ] 文档化开发者路径：Rust cdylib → `cargo build --release --target wasm32-unknown-unknown` → `assets/plugins/<name>.wasm` → `site.json::plugins` 注册
- [ ] 单测：缓存命中 / 失效 / 不兼容版本拒绝

### 1.3 数据库连接池
- [ ] `engines/db.rs`：`OnceLock<DatabaseConnection>` + `init_pool(url)` + `pool()` getter
- [ ] 全局移除每 server fn 内 `init_db(&db_url)` 调用（约 20+ 处）
- [ ] 性能基准：`scripts/bench_comments.sh` 评论列表 P95 前后对比
- [ ] 文档更新：`docs/DEVELOPER.md` DB 章节

### 1.4 ThemeEngine
- [ ] `engines/theme.rs`：`themes_stack: Vec<String>` 配置
- [ ] `aggregate_css()` 真正聚合多个主题 wasm，后者覆盖前者
- [ ] 用户主题切换 API：`set_user_theme(user_id, theme_id)` + cookie 持久
- [ ] 单测：3 主题叠加、CSS 覆盖顺序、用户偏好回读

### 1.5 LayoutEngine
- [ ] `engines/layout.rs`：`LayoutPack` trait（`navbar / footer / page_shell / sidebar`）
- [ ] 默认 `ClassicLayout`（从现有 Navbar 抽出）
- [ ] `LayoutRegistry::active() -> &dyn LayoutPack`
- [ ] App 改用 `engine.layout().navbar()` 而非直接 `Navbar`

### 1.6 ModuleEngine
- [ ] `engines/module.rs`：`ModuleSpec { id, label, icon, routes, nav_position, search_source, sitemap_provider }`
- [ ] 各模块自注册：`BlogModule::register(engine)`
- [ ] `site.json::modules` 段控制 `enabled`
- [ ] 路由生成：仅注册 enabled 模块的 Route 变体（动态注册而非条件编译）

### 1.7 ContentEngine 占位
- [ ] `engines/content.rs`：`ComponentRegistry` 骨架 + `inject_seo(meta, url)` 签名（实现在 Phase 2）

### 1.8 ModerationEngine 占位
- [ ] `engines/moderation.rs`：`Verdict` 枚举 + `ModerationStage` trait（实现在 Phase 4）

### 1.9 SearchEngine 重构
- [ ] `SearchSource` trait：`fn collect() -> Vec<IndexedDocument>`
- [ ] 各模块自注册 source；删除 `indexer::collect_documents` 中硬编码 4 源
- [ ] 单测：注册 0/1/N 个 source 都能正常索引

### 1.10 AuthEngine 改名整理
- [ ] 把 `AuthService` 改名为 `AuthEngine`，实现 `Engine` trait
- [ ] state CSRF / PKCE 加固在 Phase 7 完成

### 1.11 App crate 瘦身
- [ ] `crates/app/src/server/mod.rs` 减至 ≤ 200 行
- [ ] 评论 / 文档 / 上传 server fn 下沉到对应模块
  - [ ] 新建 `crates/modules/docs`，迁移 `list_doc_tree / get_doc_content / DocMeta / DocTreeNode` + 路由 `Docs/DocPage/TreeSection`
  - [ ] 评论迁移到新建 `crates/modules/comments`
  - [ ] 上传迁移到 `crates/modules/uploads`

### 1.12 验收门禁
- [ ] `cargo test --features server --workspace` 全绿
- [ ] `crates/app/src/server/mod.rs` ≤ 200 行
- [ ] 评论列表 P95 延迟下降 ≥ 50%（基准对比）
- [ ] `docs/ENGINES_SPEC.md` 完成

---

## Phase 2 — MDX 开放注册 + SEO 完善

> 目标：保留现有稳定 MDX；让模块/插件可贡献组件；前端 SEO 一次到位。
> **现有功能不重写**：`crates/modules/blog/src/markdown.rs` 的解析与渲染逻辑全部保留。

### 2.1 widgets crate 与 mdx 模块迁移
- [ ] 新建 `crates/widgets/Cargo.toml`（依赖 dioxus / pulldown-cmark / pulldown-latex / serde_yaml）
- [ ] 把 `crates/modules/blog/src/markdown.rs` 搬到 `crates/widgets/src/mdx.rs`
- [ ] 解耦：移除对 `rustineverything-module-podcast::PodcastCard` 的硬依赖（改由模块通过 ComponentRegistry 注册）
- [ ] 保留 `Markdown` 组件名（向后兼容）；新名 `MdxRenderer` 等价 alias
- [ ] 路由层调整：`Blog` / `DocPage` / `Lesson` import 自 widgets
- [ ] 单测保持现有渲染分支覆盖

### 2.2 ComponentRegistry 开放注册
- [ ] `MdxComponent` trait：`name() -> &'static str` + `render(attrs: &HashMap<String, String>) -> Element`
- [ ] `ComponentRegistry`：`register / lookup / list`
- [ ] 把 `render_mdx_registry()` 的 if-else 链改写为 registry 查询
- [ ] 现有 7 嵌入组件作为默认注册项保留：`PodcastCard / YouTube / Bilibili / Yellow / Green / Blue / Pink / Purple / Underline / Strikethrough`
- [ ] 单测：注册新组件可被 MDX 渲染；未知组件降级为可见占位 + 控制台告警

### 2.3 SEO frontmatter 完善
- [ ] 扩展 `PostMetadata`：增加 `image / author / og_type / canonical / date / tags`
- [ ] 实现 `inject_seo(meta, current_url) -> Element`，注入：
  - [ ] `<title>` `<meta name="description">` `<meta name="keywords">`
  - [ ] `<meta property="og:title">` `og:description` `og:image` `og:url` `og:type`
  - [ ] `<meta name="twitter:card">` `twitter:title` `twitter:image`
  - [ ] `<link rel="canonical">`
  - [ ] `<script type="application/ld+json">` Article schema (author / datePublished / image / headline)
- [ ] DocPage / Blog / CourseDetail / Lesson / CaseDetail 页面统一调用 `inject_seo`
- [ ] 单测：无 frontmatter 时不注入空 meta；image 缺失时 og:image 退到站点默认；canonical 自动生成

### 2.4 Sitemap & RSS Feed & robots
- [ ] 新 server fn `GET /sitemap.xml`（返回 `application/xml`）
  - [ ] ModuleEngine 收集所有内容页路径
  - [ ] 包含：博客文章、文档叶子、课程 lesson、案例、公开论坛话题
  - [ ] `lastmod` 取自 frontmatter `date` 或文件 mtime
- [ ] 新 server fn `GET /feed.xml`：博客 Atom feed（最近 50 篇）
  - [ ] 用 `atom_syndication` crate 或手写
  - [ ] 包含 title / description / link / pub_date / author / category
- [ ] `/robots.txt` 静态文件指向 `/sitemap.xml`
- [ ] 单测：sitemap 包含已发布的所有 URL；feed 通过 W3C feed validator

### 2.5 模块 MDX 组件注册
- [ ] `BlogModule::register_components(engine)` 注册 `<Comment>`
- [ ] `ForumModule::register_components` 注册 `<Discussion>`（替代当前底部 DiscussionPanel 直接调用）
- [ ] `PodcastModule::register_components` 注册 `<PodcastCard>`（迁移自 widgets/mdx.rs）
- [ ] `CourseModule::register_components` 注册 `<Annotation>` `<CourseProgress>`
- [ ] App 启动时自动调用所有 enabled 模块的 `register_components`

### 2.6 文档
- [ ] `docs/MDX_SPEC.md`：当前 MDX 已支持的全部语法 + 自定义组件指南 + frontmatter 字段
- [ ] `docs/SEO_SPEC.md`：注入规则、sitemap、feed、JSON-LD schema
- [ ] `docs/components/<Component>.md`：8+ 内置组件 API doc

### 2.7 验收门禁
- [ ] `cargo test -p rustineverything-widgets` 全绿
- [ ] `assets/posts/welcome/index.mdx` 渲染像素级一致
- [ ] Lighthouse SEO 评分 ≥ 95
- [ ] 第三方 demo 添加新 MDX 组件 ≤ 50 行
- [ ] sitemap 通过 google sitemap test
- [ ] feed 通过 W3C feed validator

---

## Phase 3 — 主题 / 布局 / 模块开关

### 3.1 主题栈
- [ ] ThemeEngine 支持 `themes: ["base", "ocean"]` 列表
- [ ] CSS 聚合顺序：先列在前 → 后列覆盖
- [ ] 用户在 navbar 切换主题（下拉菜单 + cookie 持久）

### 3.2 新主题插件
- [ ] `crates/plugins/theme-sunset`（暖色调）
- [ ] `crates/plugins/theme-forest`（自然绿）
- [ ] `crates/plugins/theme-monochrome`（黑白灰）
- [ ] `crates/plugins/theme-catppuccin`（Catppuccin Macchiato）
- [ ] 每个主题 dark + light 双套变量
- [ ] `scripts/build_themes.sh` 一键构建所有主题 wasm

### 3.3 Layout 包
- [ ] `crates/layouts/classic`：当前 Navbar/Footer 抽出
- [ ] `crates/layouts/magazine`：杂志风（侧栏卡片 + 大图）
- [ ] `crates/layouts/docs`：仿 Docusaurus（左导航 + 右目录）
- [ ] `crates/layouts/minimal`：极简（无 Footer / 单层 Navbar）
- [ ] LayoutEngine 切换接口 + admin 设置页

### 3.4 模块开关
- [ ] `site.json::modules.{blog,podcast,course,forum,cases,docs,ai,web3,embedded,wasm,cli}.enabled`
- [ ] ModuleEngine 仅注册 enabled 模块
- [ ] 关闭模块后：路由 404 / 导航不显示 / 搜索源剔除 / sitemap 不收录
- [ ] 18 单测覆盖：单模块开关、组合、依赖关系（如关闭 blog 时课程的"相关博客"也隐藏）

### 3.5 文档
- [ ] `docs/THEME_SPEC.md`：变量约定 / 叠加规则 / 自定义指南
- [ ] `docs/LAYOUT_SPEC.md`：LayoutPack trait / 槽位说明
- [ ] `docs/MODULE_SPEC.md`：site.json 字段 / 注册流程

### 3.6 验收门禁
- [ ] 关闭 forum 模块编译通过、路由不漏
- [ ] 切换 layout 不需重启 dx serve
- [ ] 4 主题切换在前端可见 + 持久

---

## Phase 4 — 互动 + 内容审核（个人站安全底座）

### 4.1 模块层接 ModerationEngine
- [ ] BlogModule.post_comment 提交前调 ModerationEngine
- [ ] ForumModule.create_topic / post_reply 提交前调
- [ ] CourseModule 标注 create / update 前调
- [ ] UploadModule（图片）提交前调（VLM 路径）
- [ ] 共用类型：`Submission { kind: Comment|TopicCreate|TopicReply|Annotation|Upload, content: String, ctx }`

### 4.2 ModerationEngine 框架
- [ ] `engines/moderation.rs`：`ModerationStage` trait（`evaluate(submission, ctx) -> StageVerdict`）
- [ ] `Verdict { score: f32, label: Allow|Flag|Block, reason: String, stage: String }`
- [ ] `Pipeline`：串行 + 早停（任意 Block 即终止）
- [ ] 阶段类型：`LLMStage`（文本审核）+ `VLMStage`（图片审核）；**不引入 RuleStage / 规则关键词**
- [ ] 阈值配置在 admin：`block_above`、`flag_above`

### 4.3 ModerationProvider WASM ABI
- [ ] SDK 新增：`ModerationEndpoint { url, method, headers, secret_env }`
- [ ] 三函数：`get_endpoint() / map_request(content_json) / map_verdict(response_json)`
- [ ] 宿主负责 HTTP 调用 + 超时（默认 5s）+ 重试（默认 1 次）
- [ ] 失败 = fail-open（Allow + log），不阻塞用户

### 4.4 内置审核插件（全部 LLM/VLM，wasmi 运行时）
- [ ] `crates/plugins/moderation-openai`：OpenAI Moderation API（文本）+ GPT-4o-mini（视觉）
- [ ] `crates/plugins/moderation-anthropic`：Claude prompt-based 评估（文本）+ Claude vision（视觉）
- [ ] `crates/plugins/moderation-llamaguard`：本地 ollama / llama.cpp（OpenAI 兼容 API），作 fallback
- [ ] **不开发规则关键词插件**：评论 / 话题文本统一走 LLM；上传图片统一走 VLM

### 4.5 VLM 图片审核
- [ ] 上传走 ModerationEngine
- [ ] `moderation-openai` 扩展支持 `gpt-4o-mini` 视觉模型
- [ ] `moderation-anthropic` 扩展支持 Claude vision

### 4.6 数据库
- [ ] 新表 `moderation_log`（id / submission_id / stage / score / label / reason / created_at）
- [ ] 新表 `moderation_decisions`（submission_id / final_label / human_override_by / created_at）
- [ ] 新表 `moderation_queue`（pending Flag 列表）
- [ ] SeaORM Entity 三件套

### 4.7 Admin 审核界面
- [ ] `/admin/moderation/queue`：待人工复核
- [ ] `/admin/moderation/log`：全链路审计日志（按 submission 分组）
- [ ] `/admin/moderation/policy`：阈值与启用插件配置
- [ ] 复核操作（Approve / Reject / Delete）写审计

### 4.8 文档
- [ ] `docs/INTERACTION_SPEC.md`
- [ ] `docs/MODERATION_SPEC.md`：流水线 / 阈值 / 插件 / fail-open 策略
- [ ] `docs/plugins/moderation-openai.md` 等 4 篇插件文档

### 4.9 验收门禁
- [ ] 评论审核 P95 ≤ 1.5s
- [ ] 模拟违规内容触发 Block / Flag 路径正确
- [ ] LLM 超时 / 失败时用户体验不受影响
- [ ] admin 队列可见 Flag 内容并可复核

---

## Phase 5 — 插件生态（自用 + 朋友贡献）

### 5.1 插件 manifest 与版本
- [ ] SDK 增加 `pub const SDK_ABI_VERSION: u32 = 1;`
- [ ] `PluginManifest` 增加 `abi_version: u32` + `capabilities: Vec<String>`
- [ ] 插件需导出 `get_manifest()` 函数
- [ ] PluginEngine 加载时校验 ABI / 重复 capability
- [ ] 全部插件**统一 wasmi 运行时**；文档明确推荐 **Rust → wasm** 路径

### 5.2 Hot reload
- [ ] admin `/admin/plugins` 增加上传按钮
- [ ] 上传 wasm → 沙箱校验 → 写入 `assets/plugins/` → PluginEngine reload
- [ ] 失败回滚到上一个版本

### 5.3 脚手架 CLI（自用方便）
- [ ] 新 crate `tools/dx-plugin`：`cargo install --path tools/dx-plugin`
- [ ] `dx-plugin new auth <name>` 从模板生成 cdylib 项目
- [ ] 模板覆盖：auth / theme / moderation / mdx-component / notification

### 5.4 示例插件项目
- [ ] `examples/plugin-theme-purple`：自定义主题完整 demo
- [ ] `examples/plugin-auth-feishu`：飞书登录
- [ ] `examples/plugin-moderation-haiku`：Claude Haiku 轻量 LLM 审核示例（示范 ModerationProvider ABI）
- [ ] `examples/plugin-mdx-quiz`：MDX 组件示例
- [ ] `examples/plugin-notification-discord`：Discord webhook

### 5.5 文档
- [ ] `docs/PLUGIN_DEV.md`：从零开发插件指南（30 分钟上手）
- [ ] `docs/PLUGIN_ABI.md`：ABI 规范 + 版本兼容性
- [ ] `docs/PLUGIN_RECIPES.md`：常见插件模板

### 5.6 验收门禁
- [ ] 自测 30 分钟内做出新主题
- [ ] admin 上传 wasm 不需重启
- [ ] 不兼容版本被拒绝并给出升级提示

### 5.7 插件市场（项目开源后启用）
- [ ] `assets/plugins/registry.json`：维护已审核插件清单
- [ ] 前端 `/plugins` 页：列出已审核插件 + 源码链接 + 安装指引
- [ ] `.github/ISSUE_TEMPLATE/add-plugin.yml`：第三方提交表单（repo URL / 类别 / 描述 / wasm 哈希）
- [ ] 提交流程：
  - [ ] 必须附 **源代码（GitHub repo）+ wasm 产物 + manifest**
  - [ ] 仓库维护者审核：源代码安全性 / ABI 兼容性 / capability 合规
  - [ ] 通过后 PR 添加到 `registry.json`
- [ ] 未审核插件**不在前端展示**，避免供应链风险

---

## Phase 6 — 内容板块扩展（推送内容驱动）

### 6.1 嵌入式模块 `embedded`
- [ ] `crates/modules/embedded`：lib + page + server + text
- [ ] 内容分类：no_std 基础 / Embassy / RTIC / 平台（stm32 / esp32 / rp2040 / arduino-rs）
- [ ] 种子 MDX 内容（按节奏推送）
- [ ] `docs/EMBEDDED_SPEC.md`

### 6.2 AI 模块 `ai`
- [ ] `crates/modules/ai`：lib + page + server + text
- [ ] 内容分类：candle / burn / llm 推理 / tokenizers / ort / 多模态
- [ ] 种子 MDX 内容（按节奏推送）
- [ ] `docs/AI_SPEC.md`

### 6.3 Web3 模块 `web3`
- [ ] `crates/modules/web3`：lib + page + server + text
- [ ] 内容分类：alloy（EVM）/ solana-sdk / anchor / substrate / cosmwasm
- [ ] 种子 MDX 内容（按节奏推送）
- [ ] `docs/WEB3_SPEC.md`

### 6.4 WASM 模块 `wasm`
- [ ] `crates/modules/wasm`：专题（wasm-bindgen / wasmtime / wasi / 组件模型 / 浏览器集成）
- [ ] 种子 MDX 内容
- [ ] `docs/WASM_SPEC.md`

### 6.5 CLI 模块 `cli`
- [ ] `crates/modules/cli`：CLI 工具开发（clap / ratatui / indicatif / dialoguer / shadow-rs）
- [ ] 种子 MDX 内容
- [ ] `docs/CLI_SPEC.md`

### 6.6 Cases 联动
- [ ] cases 增加自动按 module 归类的二级分类
- [ ] 每个新模块至少接入 3 个真实案例

### 6.7 验收门禁
- [ ] 5 个新模块 `cargo test -p` 通过（每模块 ≥ 12 测试）
- [ ] ModuleEngine 一键开关
- [ ] 案例库自动归类正确
- [ ] sitemap / feed 自动包含新模块内容

---

## Phase 7 — 个人站可托管（精简版）

### 7.1 数据库 migration
- [ ] 新建 `crates/migration`（基于 `sea-orm-migration`）
- [ ] 把 `init.sql` 拆成 migration 文件
- [ ] CI / 启动时自动迁移
- [ ] 删除 `init.sql`

### 7.2 Auth 加固
- [ ] state CSRF：短 TTL store（5 分钟）+ 回调强校验
- [ ] PKCE 持久化：加密 cookie（不再依赖进程内 HashMap）
- [ ] JWT 密钥强制 env：缺失 panic
- [ ] `BASE_URL` 强制配置，不再 fallback localhost

### 7.3 搜索持久化
- [ ] `MmapDirectory` 替代 `RAMDirectory`
- [ ] 增量索引：模块通知 SearchEngine 单条更新
- [ ] admin 触发 reindex 不阻塞主流程

### 7.4 审计日志（轻量）
- [ ] admin 写操作落 `admin_audit_log` 表
- [ ] 审核决策 / 人工复核全记录
- [ ] `/admin/audit` 页面查询

### 7.5 部署样板
- [ ] `Dockerfile`（多阶段，alpine 运行时即可）
- [ ] `docker-compose.yml`：app + postgres + ollama
- [ ] `scripts/deploy.sh`

### 7.6 CI
- [ ] `.github/workflows/ci.yml`：fmt + clippy + test + build wasm + build app
- [ ] `.github/workflows/release.yml`：tag → docker image

### 7.7 基础日志
- [ ] `tracing` + `tracing-subscriber` 全 server fn 标记
- [ ] 错误日志归档（本地文件 rotation 即可）

### 7.8 文档
- [ ] `docs/DEPLOY_GUIDE.md`
- [ ] `docs/OPERATIONS.md`：备份 / 恢复 / 常见故障

### 7.9 验收门禁
- [ ] CI 全绿（fmt + clippy + test + build）
- [ ] `docker compose up` 一键启动 + 自动迁移
- [ ] JWT / state / PKCE 三项加固完成

> 删除项（个人站过度工程）：~~`tower-governor` 限流~~ / ~~OpenTelemetry / Grafana~~ / ~~distroless~~ / ~~OWASP Top10 强制清单~~

---

## 跨阶段持续任务

- [ ] **代码规范**：消除所有非测试代码中的 `unwrap` / `expect`（用户规则）
- [ ] **Rust target dir**：构建产物路径设为 `/Users/hal/.target`
- [ ] **debug 习惯**：所有新增 server fn 打印请求 / 响应 / 数据库查询便于联调
- [ ] **每个模块**：完成后立刻更新本 Todos.md + 写 `docs/<MODULE>_SPEC.md`
- [ ] **测试 + 编译通过**才允许 commit；commit 信息附 `Co-Authored-By: Oz <oz-agent@warp.dev>`

## 进度跟踪

| Phase | 状态 | 起 | 止 | 备注 |
|---|---|---|---|---|
| 0 | ✅ 完成 | — | 2026-05-02 | 基线已建（含已稳定 MDX 管道） |
| 1 | ⏳ 计划 | | | 8 引擎 + DB pool 是后续一切前置 |
| 2 | ⏳ 计划 | | | MDX 仅做注册开放 + SEO 完善（不重写） |
| 3 | ⏳ 计划 | | | 配置化运营 |
| 4 | ⏳ 计划 | | | **个人站开放互动的安全底座** |
| 5 | ⏳ 计划 | | | 自用为主 + 朋友贡献 |
| 6 | ⏳ 计划 | | | 内容广度（推送驱动） |
| 7 | ⏳ 计划 | | | 个人站可托管（精简） |
