# 开发计划（v2.1 — 补充 Dioxus 原生化与 WASM 内存安全）

> 旧版已备份至 `Todos.old.md`。本版本（v2.1）在 v2 基础上补充：
> 1. 安全加固与性能热点前置（不再等到 Phase 7）
> 2. Phase 1 拆分为 3 个可独立交付的子阶段，降低单次重构风险
> 3. 引擎层分批落地，先 3 个核心引擎，后 5 个补齐
> 4. 删除/降级对个人站过度设计的任务
> 5. **v2.1 新增**：Dioxus 渲染原生化（去 JS eval）+ WASM 通信重构（引入 Extism/wit-bindgen）+ 安全补遗（OAuth state CSRF、DB 事务、上传校验、token 加密）

---

## Phase 0 — 已完成基线 ✅

> 细节请查 `docs/*_SPEC.md` 与 git log，此处仅列条目。

- ✅ Session / JWT / Cookie / 全局用户上下文
- ✅ 评论系统迁移到 PostgreSQL（comments 表 + SeaORM）
- ✅ MDX 渲染管道已稳定（520 行，frontmatter / GFM / 数学 / Mermaid / 代码 + Copy / 标注 block-id / 7 嵌入组件）
- ✅ 文档系统 `/docs`：三级嵌套、frontmatter、sidebar_label/position、sort_children，16 测试
- ✅ Podcast 动态化：YAML 元数据 + 自动音频探测，18 测试
- ✅ 课程系统 `/course`：三级结构、Doc|Video|Audio|Code 自适应、进度跟踪
- ✅ 标注系统 v2（5 色 / 4 visibility / data-block-id 回放 / 个人列表页），15 测试
- ✅ 论坛 `/topics`：发帖/回复 + 标签 + ref_kind/ref_path，18 测试
- ✅ DiscussionPanel 接入 blog / doc / lesson
- ✅ Admin 后台：5 页面 + AdminShell
- ✅ 全站搜索：Tantivy 0.26 + jieba + Cmd+K + 4 索引源，34 测试
- ✅ 案例展示 `/case`：网格 + 分类 + 标签 + Issue Form，12+ 测试
- ✅ 6 WASM 插件：github/google/discord/twitter auth + theme-ocean + i18n-fluent

---

## Phase 1A — 安全加固 + 性能热点（最高优先级）

> 目标：修复评估发现的安全隐患和性能瓶颈，不涉及架构重构，可快速交付。

### 1A.1 安全修复（紧急）
- [x] **JWT Secret 强制配置**：`get_jwt_secret()` 缺失 `JWT_SECRET` 时 panic 而非 fallback 默认值
- [x] **删除 Token 日志泄露**：`auth/mod.rs:187` 的 `println!("[Auth] Token response: {:?}")` 移除或替换为脱敏日志
- [x] **Cookie 加固**：生产环境 Set-Cookie 增加 `Secure` 标志（通过 `BASE_URL` 是否 https 判断）
- [x] **PKCE Store TTL**：`PKCE_STORE` 增加过期清理（5 分钟 TTL），防止内存泄漏
- [x] **BASE_URL 强制配置**：缺失时 panic，不再 fallback `localhost:8080`
- [x] 单测：JWT Secret 缺失 panic / PKCE 过期清理 / Cookie Secure 标志

### 1A.2 数据库连接池（性能关键）
- [x] 新建 `crates/core/src/db/pool.rs`：`OnceCell<DatabaseConnection>` 单例
- [x] `init_pool(url: &str)` 应用启动时调用一次，`get_or_init_pool() -> DatabaseConnection` 全局获取
- [x] 逐模块替换 `init_db(&db_url)` 调用（约 20+ 处）：
  - [x] `app/src/server/mod.rs` 中的评论 / 文档 / 上传
  - [x] `app/src/main.rs` 中的 auth callback
  - [x] `modules/forum/src/server.rs`
  - [x] `modules/course/src/server.rs`
  - [x] `modules/admin/src/server.rs`
  - [ ] `modules/cases/src/server.rs`（cases 不依赖 DB）
  - [x] `modules/search/src/indexer.rs`
- [ ] 性能基准：`scripts/bench_comments.sh` 评论列表 P95 前后对比
- [ ] `docs/DEVELOPER.md` DB 章节更新

### 1A.3 PluginManager 缓存
- [ ] `PluginManager` 增加 `Module` 缓存：`HashMap<PathBuf, (SystemTime, Module)>`
- [ ] `call_with_string()` 先查缓存，mtime 不变则复用 `Module`
- [ ] 避免每次 `fs::read()` + `Module::new()` 的开销（i18n / theme 高频调用）
- [ ] `invalidate(path)` 方法供 admin 显式刷新
- [ ] 单测：缓存命中 / mtime 变更失效 / invalidate 后重加载

### 1A.4 高优先级安全补遗（评估补充发现）
- [x] **OAuth `state` 参数 CSRF 校验**：当前 `auth/mod.rs` 生成 state 但回调路径**完全未验证**；新增 state store（HashMap + 5 分钟 TTL）+ 回调强校验，state 不匹配直接 401
- [x] **用户创建事务化**：`sync_user_to_db` 中 `user::insert` + `user_identity::insert` 包入 SeaORM 事务（`db.begin().await?`），避免 identity 失败时残留孤儿 user
- [x] **图片上传校验**：`upload_image` 增加 MIME 嗅探（白名单 png/jpg/gif/webp）+ 文件大小上限（5MB）+ 安全文件名（移除 `..`、`/`、`\`，限制扩展名）
- [x] **`access_token` 加密存表**：`user_identities.access_token` 当前明文落库；用 `JWT_SECRET` 派生密钥做 AES-GCM 加密；解密失败时强制重新登录
- [x] **删除 dead code**：`crates/plugins/prefix-plugin/` 是 hello-world demo，未在 site.json 引用；移到 `examples/` 或直接删除
- [ ] 单测：state 校验失败拒绝登录 / 大文件上传拒绝 / 非白名单 MIME 拒绝 / 加密 token 可解密回原值 / 事务回滚正确

### 1A.5 Dioxus 渲染原生化（去 JS 依赖）
- [x] `app/src/main.rs`：移除通过 `dioxus::document::eval` 动态创建 `<style>` 标签的 JavaScript 注入逻辑
- [x] 使用原生 RSX 语法重构：在虚拟 DOM 中直接渲染 `<style id="wasm-theme-style">{theme_css}</style>`
- [x] 验证：确保切换主题时无闪烁，且去除对浏览器 DOM API 的直接依赖
- [ ] 同步排查：`markdown.rs` 中 Prism / Mermaid 的 `dioxus::document::eval` 调用是否可用 `document::Script` 或 hydration-safe 方式替代（desktop / mobile 平台需求）

### 1A.6 验收门禁
- [ ] `cargo test --features server --workspace` 全绿
- [ ] 评论列表 P95 延迟下降 ≥ 50%
- [ ] JWT Secret 未配置时服务启动即失败
- [ ] `grep -r "Token response" crates/` 返回空
- [ ] OAuth 不带合法 state 的回调请求被拒绝（curl 模拟验证）
- [ ] 主题切换不再触发 JS eval（DevTools Console 无 `Injecting CSS` 日志）
- [ ] `crates/plugins/prefix-plugin/` 已移除或归入 `examples/`

---

## Phase 1B — App Crate 拆分（758 行 → ≤ 200 行）

> 目标：把 `app/src/server/mod.rs` 中混合的 5 个领域拆分到独立模块。

### 1B.1 文档模块独立
- [ ] 新建 `crates/modules/docs/`（Cargo.toml + lib.rs + server.rs + docs.rs）
- [ ] 迁移：`DocMeta / DocTreeNode / DocContentResponse / parse_doc_frontmatter / extract_doc_info / scan_doc_dir` + 16 个测试
- [ ] 迁移路由组件：`Docs / DocPage / TreeSection` 从 `routes/mod.rs` 移到 `modules/docs/src/docs.rs`
- [ ] `routes/mod.rs` 改为 `use rustineverything_module_docs::docs::*`
- [ ] workspace Cargo.toml 注册新 crate
- [ ] 验证：16 个文档系统测试通过

### 1B.2 评论模块独立
- [ ] 新建 `crates/modules/comments/`
- [ ] 迁移：`Comment` struct + `get_comments / post_comment` server fn
- [ ] `CommentBox` 组件调用改为引用新模块

### 1B.3 上传模块独立
- [ ] 新建 `crates/modules/uploads/`
- [ ] 迁移：`upload_image` server fn

### 1B.4 App server/mod.rs 精简
- [ ] 仅保留：站点配置 / i18n / 主题 CSS / Auth 辅助 / echo（约 150 行）
- [ ] 移除所有内联的评论 / 文档 / 上传逻辑
- [ ] 抽出 `get_asset_root()` 为公共工具（当前在 `app/src/main.rs` 与 `server/mod.rs` 重复定义）

### 1B.5 统一错误类型
- [ ] 新建 `crates/core/src/error.rs`：`pub enum AppError { Db(sea_orm::DbErr), Plugin(String), Auth(String), Io(std::io::Error), Validation(String) }`
- [ ] 实现 `From<AppError> for ServerFnError`，逐步替换全代码 `Box<dyn std::error::Error>`（约 30+ 处）
- [ ] 错误信息不向客户端暴露内部细节（数据库错误等只返回"内部错误"，详情写日志）

### 1B.6 验收门禁
- [ ] `wc -l crates/app/src/server/mod.rs` ≤ 200
- [ ] `cargo test --features server --workspace` 全绿
- [ ] 所有页面功能不变
- [ ] `grep -rE 'Box<dyn .*Error' crates/` 显著减少

---

## Phase 1C — 引擎层抽象（3 核心 + 5 占位）

> 目标：建立引擎注册机制，先实现 PluginEngine / DB Engine / ModuleEngine 三个核心引擎。

### 1C.1 引擎抽象基础
- [ ] `crates/core/src/engines/mod.rs`：`Engine` trait（`name() / init() / shutdown()`）
- [ ] `EngineRegistry`：注册 + 按名取出 + 初始化顺序管理
- [ ] `EngineContext`：包含 DB pool handle / SiteConfig / PluginManager
- [ ] 单测：注册多引擎、重复名报错

### 1C.2 PluginEngine（替代 PluginManager）与 ABI 重构
- [ ] `engines/plugin.rs`：将 1A.3 的缓存 PluginManager 升级为 `PluginEngine`，实现 `Engine` trait
- [ ] **【新增】WASM 通信重构**：引入 `Extism` 或 `wit-bindgen` 替换当前手动 `alloc`/`dealloc` 的内存管理模式（评估两者：Extism 更易上手 + 跨语言；wit-bindgen 更标准化 + WASM Component Model）
- [ ] **【新增】清理安全隐患**：移除 `crates/sdk/src/lib.rs` 及所有 `plugins/` 目录下的 `unsafe extern "C"` 和指针 `<<32 | len` 打包解包逻辑
- [ ] **【新增】WASM 输出大小限制**：返回数据超过阈值（默认 8MB，可配置）拒绝读取，防止恶意/失控插件让宿主 OOM
- [ ] SDK 新增 `pub const SDK_ABI_VERSION: u32 = 1`
- [ ] `PluginManifest` 增加 `abi_version: u32` + `capabilities: Vec<String>`
- [ ] 所有插件新增 `get_manifest()` 导出函数
- [ ] 加载时校验：ABI 版本不匹配 → 拒绝 + 日志告警
- [ ] 能力协商：manifest.capabilities 标明插件类型（auth / theme / i18n / moderation 等）
- [ ] 迁移现有 6 个插件到新 ABI（github/google/discord/twitter auth + theme-ocean + i18n-fluent）
- [ ] 单测：ABI 版本不兼容拒绝 / 能力查询 / manifest 解析 / 大体积 JSON（>1MB）传递无溢出 / 输出超阈值被截断

### 1C.3 ModuleEngine（模块注册 + 开关）
- [ ] `engines/module.rs`：`ModuleSpec { id, label, routes, nav_position, enabled }`
- [ ] 各模块实现自注册：`BlogModule::register(engine)` 等
- [ ] `site.json::modules` 段控制 `enabled`
- [ ] 导航生成仅包含 enabled 模块
- [ ] 搜索索引源仅采集 enabled 模块
- [ ] 单测：开关模块后导航 / 搜索源 / sitemap 联动

### 1C.4 其余 5 引擎占位
- [ ] `engines/theme.rs`：ThemeEngine 骨架（封装现有 `aggregate_theme_css`）
- [ ] `engines/layout.rs`：LayoutEngine 骨架 + `LayoutPack` trait 定义
- [ ] `engines/content.rs`：ContentEngine 骨架 + `ComponentRegistry` 签名
- [ ] `engines/moderation.rs`：ModerationEngine 骨架 + `Verdict` 枚举 + `ModerationStage` trait
- [ ] `engines/auth.rs`：AuthEngine（`AuthService` 重命名，实现 `Engine` trait）
- [ ] `engines/search.rs`：SearchEngine 封装（`SearchSource` trait + 自注册取代硬编码 4 源）

### 1C.5 验收门禁
- [ ] `cargo test --features server --workspace` 全绿
- [ ] `docs/ENGINES_SPEC.md` 完成（8 引擎职责 + 接口 + 交互图）
- [ ] 关闭 forum 模块后导航 / 搜索 / 路由均正确响应

---

## Phase 2 — MDX 开放注册 + SEO

> 目标：MDX 解析器不重写，仅开放组件注册 + SEO 一次到位。

### 2.1 widgets crate 迁移
- [ ] 新建 `crates/widgets/`，将 `modules/blog/src/markdown.rs` 搬到 `crates/widgets/src/mdx.rs`
- [ ] 移除 `markdown.rs` 对 `rustineverything-module-podcast::PodcastCard` 的直接依赖
- [ ] `Blog / DocPage / Lesson` 改为从 widgets 引入 `Markdown`
- [ ] 单测保持现有覆盖

### 2.2 ComponentRegistry
- [ ] `MdxComponent` trait：`name() -> &'static str` + `render(attrs) -> Element`
- [ ] `ComponentRegistry`：`register / lookup / list`
- [ ] `render_mdx_registry()` if-else 链改为 registry 查询
- [ ] 现有 7 嵌入组件作为默认注册项
- [ ] 各模块 `register_components()`：PodcastCard / Discussion / Annotation 等
- [ ] 单测：新组件注册可渲染 / 未知组件降级占位

### 2.3 SEO 注入
- [ ] `PostMetadata` 扩展：image / author / canonical / date / tags
- [ ] `inject_seo(meta, url) -> Element`：title / description / og:* / twitter:* / canonical / JSON-LD
- [ ] 所有内容页统一调用
- [ ] 单测：缺失字段不注入空 meta / canonical 自动生成

### 2.4 Sitemap & Feed
- [ ] `GET /sitemap.xml`：ModuleEngine 收集全部内容页
- [ ] `GET /feed.xml`：博客 Atom feed（最近 50 篇）
- [ ] `/robots.txt` 指向 sitemap
- [ ] 单测：URL 完整性 / feed 格式

### 2.5 文档
- [ ] `docs/MDX_SPEC.md`
- [ ] `docs/SEO_SPEC.md`
- [ ] `docs/components/<Component>.md`（8+ 组件）

### 2.6 验收门禁
- [ ] `cargo test -p rustineverything-widgets` 全绿
- [ ] welcome 示例渲染一致
- [ ] Lighthouse SEO ≥ 95
- [ ] 新增 MDX 组件 ≤ 50 行

---

## Phase 3 — 主题 / 模块开关配置化

> 目标：站点形态完全由 site.json 决定。Layout 简化为 1 默认 + 1 备选。

### 3.1 ThemeEngine 完整实现
- [ ] 主题栈 `themes: ["base", "ocean"]`，后者覆盖前者
- [ ] 用户 navbar 切换主题 + cookie 持久

### 3.2 新主题插件（2 个即可）
- [ ] `theme-sunset`（暖色调，dark + light）
- [ ] `theme-catppuccin`（Catppuccin Macchiato，dark + light）
- [ ] `scripts/build_themes.sh` 一键构建

### 3.3 Layout 精简
- [ ] 从现有 Navbar/Footer 抽出 `ClassicLayout`（默认）
- [ ] `MinimalLayout`（极简，无 Footer / 单层 Navbar）
- [ ] LayoutEngine 切换：site.json 配置 + admin 设置页

### 3.4 模块开关
- [ ] `site.json::modules.{blog,podcast,course,forum,cases,docs,...}.enabled`
- [ ] 关闭后：路由 404 / 导航隐藏 / 搜索源剔除 / sitemap 不收录
- [ ] 单测：单模块开关 + 组合测试

### 3.5 文档
- [ ] `docs/THEME_SPEC.md`
- [ ] `docs/MODULE_SPEC.md`

### 3.6 验收门禁
- [ ] 关闭 forum 编译通过、路由不漏
- [ ] 2 主题 + 2 布局切换正常 + 持久

---

## Phase 4 — 内容审核（LLM/VLM）

> 目标：评论/话题/上传走统一审核流水线。全部走模型，不用规则关键词。

### 4.1 ModerationEngine 实现
- [ ] `ModerationStage` trait：`evaluate(submission) -> StageVerdict`
- [ ] `Verdict { score, label: Allow|Flag|Block, reason }`
- [ ] Pipeline 串行 + 早停
- [ ] 阈值配置：`block_above` / `flag_above`

### 4.2 模块接入
- [ ] 评论 / 话题创建 / 话题回复 / 标注 / 上传 → ModerationEngine hook
- [ ] 超时/失败 = fail-open（Allow + log）
- [ ] **XSS 防护**：所有用户提交的 Markdown 在渲染前过滤危险 HTML 标签 / 属性（白名单：a, p, h1-h6, ul, ol, li, code, pre, blockquote, img(src/alt), br, hr, em, strong）；禁止 `<script>` `<iframe>` `on*=` 内联事件
- [ ] **MDX `dangerous_inner_html` 审计**：`markdown.rs` 中 math / mermaid / 嵌入 HTML 的 `dangerous_inner_html` 来源是否可控；用户内容不允许走该路径

### 4.3 ModerationProvider WASM ABI
- [ ] `get_endpoint() / map_request() / map_verdict()` 三函数
- [ ] 宿主负责 HTTP + 超时 5s + 重试 1 次

### 4.4 内置审核插件
- [ ] `moderation-openai`：OpenAI Moderation API + GPT-4o-mini 视觉
- [ ] `moderation-anthropic`：Claude 文本 + 视觉
- [ ] `moderation-llamaguard`：本地 ollama（fallback）

### 4.5 数据库 + Admin
- [ ] `moderation_log / moderation_decisions / moderation_queue` 三张表
- [ ] Admin：队列 / 审计日志 / 阈值配置 / 复核操作

### 4.6 文档
- [ ] `docs/MODERATION_SPEC.md`

### 4.7 验收门禁
- [ ] 审核 P95 ≤ 1.5s
- [ ] 模拟违规 → Block/Flag 正确
- [ ] LLM 失败不阻塞用户

---

## Phase 5 — 插件生态

### 5.1 Hot Reload
- [ ] admin 上传 wasm → 沙箱校验 → 替换 → PluginEngine reload
- [ ] 失败回滚到上一版本
- [ ] **验证 Hot Reload 时的内存回收**：旧 `wasmi::Module` / `Instance` / `Memory` 必须完全 Drop，防止服务器内存缓慢泄漏（用 `valgrind` 或长跑监测 RSS 验证）
- [ ] 测试：连续 1000 次 reload 后 RSS 涨幅 ≤ 50MB

### 5.2 示例插件（3 个核心）
- [ ] `examples/plugin-theme-purple`：自定义主题 demo
- [ ] `examples/plugin-auth-feishu`：飞书登录
- [ ] `examples/plugin-moderation-haiku`：Claude Haiku 轻量审核

### 5.3 文档
- [ ] `docs/PLUGIN_DEV.md`：从零开发插件（30 分钟上手）
- [ ] `docs/PLUGIN_ABI.md`：ABI 规范 + 版本兼容性

### 5.4 验收门禁
- [ ] 自测 30 分钟内做出新主题
- [ ] admin 上传 wasm 不需重启
- [ ] ABI 不兼容被拒绝并提示升级

### 5.5 插件市场（开源后启用，优先级低）
- [ ] `assets/plugins/registry.json` 已审核插件清单
- [ ] `/plugins` 前端浏览页
- [ ] 提交审核流程文档

---

## Phase 6 — 内容板块扩展

> 每模块遵循 `lib.rs + <name>.rs(UI) + server.rs + text.rs` + 单测 ≥ 12。

### 6.1 ~ 6.5 新模块
- [ ] `modules/embedded`：Rust 嵌入式（no_std / Embassy / RTIC / 平台）
- [ ] `modules/ai`：Rust AI（candle / burn / llm 推理 / tokenizers）
- [ ] `modules/web3`：Web3（alloy / solana / anchor / substrate）
- [ ] `modules/wasm`：WASM 专题（wasm-bindgen / wasi / 组件模型）
- [ ] `modules/cli`：CLI 工具（clap / ratatui / indicatif）

### 6.6 Cases 联动
- [ ] 按 module 自动归类
- [ ] 每模块至少 3 个真实案例

### 6.7 验收门禁
- [ ] 5 模块 `cargo test -p` 通过
- [ ] ModuleEngine 一键开关
- [ ] sitemap / feed 包含新模块内容

---

## Phase 7 — 可部署上线

### 7.1 数据库 Migration
- [ ] `crates/migration`（sea-orm-migration），替代 `init.sql`
- [ ] 启动时自动迁移

### 7.2 Auth 进一步加固
- [ ] PKCE 持久化：加密 cookie 替代进程内 HashMap
- [ ] state CSRF 短 TTL（5 分钟）

### 7.3 搜索持久化
- [ ] `MmapDirectory` 替代 `RAMDirectory`
- [ ] 增量索引

### 7.4 部署
- [ ] `Dockerfile`（多阶段 alpine）
- [ ] `docker-compose.yml`：app + postgres + ollama
- [ ] `.github/workflows/ci.yml`：fmt + clippy + test + build

### 7.5 日志
- [ ] `tracing` + `tracing-subscriber`
- [ ] 删除全部 `println!` 调试输出，替换为结构化日志

### 7.6 文档
- [ ] `docs/DEPLOY_GUIDE.md`
- [ ] `docs/OPERATIONS.md`

### 7.7 验收门禁
- [ ] CI 全绿
- [ ] `docker compose up` 一键启动 + 自动迁移

---

## 跨阶段持续任务

- [ ] **代码规范**：消除所有非测试代码中的 `unwrap` / `expect`
- [ ] **Rust target dir**：构建产物路径 `/Users/hal/.target`
- [ ] **debug 习惯**：新增 server fn 打印请求/响应/DB 查询便于联调
- [ ] **每个模块完成后**：更新本 Todos.md + 写 `docs/<MODULE>_SPEC.md`
- [ ] **测试 + 编译通过**才允许 commit；commit 附 `Co-Authored-By: Oz <oz-agent@warp.dev>`

---

## 进度跟踪

| Phase | 状态 | 关键能力解锁 |
|---|---|---|
| 0 | ✅ 完成 | 基线（7 模块 + 6 插件 + MDX 稳定） |
| 1A | ⏳ 待开始 | 安全加固 + DB 池 + 插件缓存 + Dioxus 原生化 + 安全补遗 |
| 1B | ⏳ 待开始 | App crate 拆分（758行→≤200行）+ 统一错误类型 |
| 1C | ⏳ 待开始 | 3 核心引擎 + WASM ABI 重构（Extism/wit-bindgen）+ 5 占位 |
| 2 | ⏳ 待开始 | MDX 组件开放注册 + SEO 到位 |
| 3 | ⏳ 待开始 | 站点形态配置化 |
| 4 | ⏳ 待开始 | LLM/VLM 审核 + XSS 防护 |
| 5 | ⏳ 待开始 | 插件生态（Hot Reload + 内存回收验证 + 示例） |
| 6 | ⏳ 待开始 | 5 新内容板块 |
| 7 | ⏳ 待开始 | Docker + CI + 可部署 |

## v2.1 变更记录（2026-05-09）

### 新增任务
- **Phase 1A.4**：高优先级安全补遗（OAuth state CSRF / DB 事务 / 上传校验 / token 加密 / dead code 清理）
- **Phase 1A.5**：Dioxus 渲染原生化（去 `eval(...)` JS 注入）
- **Phase 1B.5**：统一错误类型 `AppError`
- **Phase 1C.2**：WASM 通信重构（Extism 或 wit-bindgen）+ 移除 `unsafe extern "C"` + 输出大小限制
- **Phase 4.2**：用户内容 XSS 防护 + `dangerous_inner_html` 审计
- **Phase 5.1**：Hot Reload 内存回收验证（防止 Module/Instance/Memory 泄漏）

### 设计权衡
- **WASM ABI 重构放在 Phase 1C 而非 1A**：1A 先用最小改动（缓存）止血，1C 做深层 ABI 替换；这样 1A 可立刻交付不阻塞其他工作
- **Dioxus 原生化放在 1A 而非延后**：成本低（约 30 行 RSX 改写），收益是恢复 desktop/mobile 跨平台承诺，应尽早还
- **WASM 内存上限默认 8MB**：覆盖 Theme CSS（数 KB）/ i18n 翻译（数 KB）/ Auth profile（数 KB）/ Moderation response（最大 ~MB），充足且足够防御
