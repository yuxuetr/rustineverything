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
- [x] `docs/DEVELOPER.md` DB 章节更新：新增 §2.3「数据库层与连接池」，文档化 SeaORM+PG + sea-orm-migration 自动迁移 + `init_pool/get_or_init_pool/pool` 连接池单例 API + 旧 `init_db` 兼容说明

### 1A.3 PluginManager 缓存
- [x] `PluginManager` 增加 `Module` 缓存：`HashMap<PathBuf, CachedModule { module, mtime }>`
- [x] `call_path_with_string()` 先查缓存，mtime 不变则复用 `Module`（保留原 `call_with_string` 以兼容）
- [x] 避免每次 `fs::read()` + `Module::new()` 的开销（i18n / theme 高频调用 → 全局 `shared_plugin_manager()`）
- [x] `invalidate(path)` / `invalidate_all()` 供 admin 显式刷新
- [x] 单测：缓存命中 / invalidate 中一 / invalidate_all

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
- [x] `cargo test --features server --workspace` 全绿（183 tests pass under —test-threads=1）
- [ ] 评论列表 P95 延迟下降 ≥ 50%（需要实际服务运行与 bench 脚本，后续 Phase 7 补上）
- [x] JWT Secret 未配置时服务启动即失败（`get_jwt_secret()` panic）
- [x] `grep -r "Token response" crates/` 返回空
- [x] OAuth 不带合法 state 的回调请求被拒绝（`AuthService::validate_state` + 3 个单测）
- [x] 主题切换不再触发 JS eval（DevTools Console 无 `Injecting CSS` 日志）
- [x] `crates/plugins/prefix-plugin/` 已移除

---

## Phase 1B — App Crate 拆分（758 行 → ≤ 200 行）

> 目标：把 `app/src/server/mod.rs` 中混合的 5 个领域拆分到独立模块。

### 1B.1 文档模块独立
- [x] 新建 `crates/modules/docs/`（Cargo.toml + lib.rs + server.rs + docs.rs）
- [x] 迁移：`DocMeta / DocTreeNode / DocContentResponse / parse_doc_frontmatter / extract_doc_info / scan_doc_dir` + 15 个测试（原计划 16，实际原始代码 15）
- [x] 迁移路由组件：`Docs / DocPage / TreeSection` 从 `routes/mod.rs` 移到 `modules/docs/src/docs.rs`（使用 `<a href>` 打灯 避免对 app `Route` 循环依赖）
- [x] `routes/mod.rs` 改为 `use rustineverything_module_docs::docs::{Docs, DocPage}`（路由仍由 app crate 控制）
- [x] workspace Cargo.toml 注册新 crate
- [x] 验证：15 个文档系统测试通过（`cargo test -p rustineverything-module-docs --features server`）

### 1B.2 评论模块独立
- [x] 新建 `crates/modules/comments/`（Cargo.toml + lib.rs + server.rs）
- [x] 迁移：`Comment` struct + `get_comments / post_comment` server fn（使用 `rustineverything_core::session::current_session_user`）
- [x] `CommentBox` 组件调用改为 `rustineverything_module_comments::server::{get_comments, post_comment}`
- [x] 在工作区 Cargo.toml + app/Cargo.toml 注册

### 1B.3 上传模块独立
- [x] 新建 `crates/modules/uploads/`（Cargo.toml + lib.rs + server.rs）
- [x] 迁移：`upload_image` server fn + `sniff_image_mime` + `safe_upload_filename` + 9 个单测
- [x] `CommentBox` 调用改为 `rustineverything_module_uploads::server::upload_image`
- [x] 从 `app/src/server/mod.rs` 移除上传路由与上传测试，避免重复注册 `/api/upload`

### 1B.4 App server/mod.rs 精简
- [x] 仅保留：站点配置 / i18n / 主题 CSS / Auth 辅助 / echo（实际 162 行）
- [x] 移除所有内联的评论 / 文档 / 上传逻辑
- [x] 抽出 `get_asset_root()` 到 `crates/core/src/utils.rs`，`app/src/server/mod.rs` 与 `app/src/main.rs` 都改为 `rustineverything_core::utils::get_asset_root`（上游模块还未请河，在后续阶段迫出）

### 1B.5 统一错误类型
- [x] 新建 `crates/core/src/error.rs`：`pub enum AppError { Db(sea_orm::DbErr), Plugin(String), Auth(String), Io(std::io::Error), Validation(String), Other(String) }` + `pub type AppResult<T>`
- [x] 实现 `From<AppError> for ServerFnError`（仅 server feature），From<sea_orm::DbErr / std::io::Error / String / &str / serde_json::Error / serde_yaml::Error>
- [x] 错误信息不向客户端暴露内部细节：Db / Io 变体转 ServerFnError 后仅返回“内部错误”，原始详情走 eprintln 日志（6 个 tests 验证）
- [x] 示范迁移：`SiteConfig::from_file()` 从 `Box<dyn Error>` 改为 `AppResult<Self>`，调用方 (3 处) 无需修改（`unwrap_or_default` / `Display` formatting 兼容）
- [x] 迁移剩余 `Box<dyn Error>` 返回值到 `AppResult`：session::create_jwt/verify_jwt、auth::*（get_credentials/get_auth_url/validate_state/handle_callback/sync_user_to_db）、PluginManager::*（get_or_load_module/call_with_string/call_path_with_string/invoke_module）、app `auth_callback_internal`。新增 `From<wasmi::Error>`(→Plugin) + `From<reqwest::Error>`(→Auth, server) 让 `?` 自动转换;jwt/memory/utf8 等用 `map_err` 精确归类。全工作区 `Box<dyn Error>` 仅剩 sdk 的 `AppModule::init`（sdk 不依赖 core，无法用 AppError，保留）

### 1B.6 验收门禁
- [x] `wc -l crates/app/src/server/mod.rs` ≤ 200（实际 162）
- [x] `cargo test --features server --workspace` 全绿（192 tests passed; 0 failed，含新增的 docs / uploads / AppError 测试）
- [x] 所有页面功能不变（仅代码位置调整 + 依赖重接，路由与组件外部 API 不变）
- [x] `grep -rE 'Box<dyn .*Error' crates/` 从 15 → **1**：仅剩 `crates/sdk/src/lib.rs` 的 `AppModule::init`（sdk 是插件 ABI crate，不依赖 core，故保留 `Box<dyn Error>`）。其余全部迁到 `AppResult`

---

## Phase 1C — 引擎层抽象（3 核心 + 5 占位）

> 目标：建立引擎注册机制，先实现 PluginEngine / DB Engine / ModuleEngine 三个核心引擎。

### 1C.1 引擎抽象基础
- [x] `crates/core/src/engines/mod.rs`：`Engine` trait（`name() / init() / shutdown()` + `as_any/as_any_mut` 供按类型 downcast）
- [x] `EngineRegistry`：按顺序注册 + 按名查询 + `init_all` / `shutdown_all`（shutdown 逆序）
- [x] `EngineContext`：持有 `Arc<SiteConfig>` + `asset_root: PathBuf`（DB / PluginManager 交由各引擎自己老仪，避免互借塑）
- [x] 单测 8 个：注册两个 / 重复名报错 / init 顺序 / init 警告传递 / shutdown 调用 / 错误类型 downcast / get_mut / EngineContext::for_tests

### 1C.2 PluginEngine（替代 PluginManager）与 ABI 重构
- [x] `engines/plugin.rs`：`PluginEngine` 包装 1A.3 的缓存 `PluginManager`，实现 `Engine` trait（`init/shutdown/as_any`）
- [-] **WASM 通信重构**：本阶段保留 `alloc`/`dealloc` + u64 打包，在 SDK 中提供 `pack_output` / `pack_json` / `read_input` 安全包装屏蔽原始 unsafe 指针运算；深入切换 Extism / wit-bindgen 是后续 PR（需 ABI v2 + 全量重建插件）
- [x] **清理安全隐患**：SDK 提供高阶辅助（`pack_json`）减少插件 boilerplate；宿主侧原 `PluginManager::call_path_with_string` 仍保留以防老插件 (等 ABI v2 后一次性干掉手动打包)
- [x] **WASM 输出大小限制**：`PluginEngine::DEFAULT_PLUGIN_OUTPUT_LIMIT = 8MB`，`with_output_limit(n)` 可调；超过限制返回 `AppError::Plugin`。单测覆盖（`output_over_limit_is_rejected`）
- [x] SDK 新增 `pub const SDK_ABI_VERSION: u32 = 1`
- [x] `PluginManifest` 增加 `abi_version: u32` + `capabilities: Vec<String>` + builder API + `is_compatible/has_capability` + 能力常量模块 (`AUTH_PROVIDER` / `THEME` / `I18N` / `MODERATION_PROVIDER` / `NOTIFICATION` / `LAYOUT` / `MDX_COMPONENT`)
- [x] 所有 6 个插件新增 `get_manifest()` 导出函数，使用 `pack_json`
- [x] 加载时校验：`PluginEngine::call` 调用前读 manifest，不兼容 → 拒绝 + `AppError::Plugin`；老插件未导出 manifest 时降级为 `call` 可运行 / `strict_call` 拒绝
- [x] 能力协商：`capabilities_of(path)` / `filter_by_capability(paths, cap)` 实现能力分发
- [x] 迁移 6 个插件并重建 wasm：github/google/discord/twitter auth (capability=AUTH_PROVIDER) + theme-ocean (THEME) + i18n-fluent (I18N)
- [x] 单测 22 个：SDK 10 (manifest 创建/不兼容/能力/序列化/向后兼容/pack_output/pack_json/read_input) + PluginEngine 12 (名字/限制/shutdown/init/manifest 检测/filter_by_capability/老插件/超限/3 集成)

### 1C.3 ModuleEngine（模块注册 + 开关）
- [x] `engines/module.rs`：`ModuleSpec { id, label, routes, nav_position, enabled }` + builder API + `disabled()` 反变是
- [x] `ModuleEngine` 实现 `Engine` trait：register/get/is_enabled/enabled_modules/navigation/enabled_ids；init 阶段调 `apply_site_config` 备践 SiteConfig.modules
- [x] `site.json::modules` 段控制 `enabled`：`SiteConfig` 增加 `modules: HashMap<String, ModuleSettings>` 字段，default 不不是不唭互选（默认 enabled = true 在 ModuleSettings 同名字段上）
- [x] `ModuleEngine::navigation()` 仅返回 enabled 且 `nav_position.is_some()` 的模块，按位置升序（稳定排序）
- [x] `ModuleEngine::enabled_ids()` 供搜索 / sitemap / feed 接入
- [x] 单测 10 个：builder / 注册查询 / 重复 id 拒绝 / site.json 关闭 / nav 过滤 / 搜索源 / `ModuleSettings` 默认 enabled / Engine trait / with_specs / nav 同位置稳定

### 1C.4 其余 5 引擎占位 + AuthEngine
- [x] `engines/theme.rs`：ThemeEngine 骨架包装 `PluginEngine`：register_theme/set_themes/aggregate_css；init 阶段从 `SiteConfig.active_theme` 读出默认主题路径（4 单测）
- [x] `engines/layout.rs`：`LayoutPack` trait（name + label）+ `LayoutEngine` 注册中心 + active layout 记录（4 单测）
- [x] `engines/content.rs`：`MdxComponent` trait（name + render(attrs)）+ `ComponentRegistry`（register / lookup / list / render）+ `ContentEngine`（5 单测，含未知组件降级占位）
- [x] `engines/moderation.rs`：`ModerationLabel` (Allow/Flag/Block) + `Verdict { score, label, reason }` + `ModerationStage` trait + 串行流水线（8 单测，含早停 / Flag 取最高分 / score 夹估）
- [x] `engines/auth.rs`：`AuthEngine` (server-only) 包装 `AuthService`，init 读 `site_config.auth.enabled`（4 单测）
- [x] `engines/search.rs`：`SearchDocument` 数据 + `SearchSource` trait + `SearchEngine`（collect_all / collect_filtered 按 enabled 过滤）（4 单测）
- [x] 小计：8 引擎全部实现 `Engine` trait，核心 (`plugin/module/auth`) 可接入现有 server fn；5 占位骨架 (`theme/layout/content/moderation/search`) 为后续 Phase 2/3/4 准备 trait 契约。全部 57 个 engines::* 单测通过

### 1C.5 验收门禁
- [x] `cargo test --features server --workspace` 全绿（8 引擎 + SDK + 全部模块，总计 ~249 tests passed）
- [x] `docs/ENGINES_SPEC.md` 完成：8 引擎职责 / 接口 / 架构图 / 生命周期 / 依赖关系 / 后续阶段路径
- [-] 关闭 forum 模块后导航 / 搜索 / 路由均正确响应：ModuleEngine `apply_site_config` + `enabled_ids/navigation` 在单测中已验证。实际 server fn / 路由层接入（路由 404 表现 / 搜索 indexer 调用）是 Phase 3.4 路纱

---

## Phase 2 — MDX 开放注册 + SEO

> 目标：MDX 解析器不重写，仅开放组件注册 + SEO 一次到位。

### 2.1 widgets crate 迁移
- [x] 新建 `crates/widgets/`，将 `modules/blog/src/markdown.rs` 搬到 `crates/widgets/src/mdx.rs`（520 → 716 行，含 13 个新增单测）
- [x] 移除 `markdown.rs` 对 `rustineverything-module-podcast::PodcastCard` 的直接依赖：引入 `MdxComponent` trait + 全局 `OnceLock<RwLock<ComponentRegistry>>`，podcast 模块在 `register_components()` 中注册自身
- [x] `Blog / DocPage / Lesson / Cases / Forum / Comments` 改为从 widgets 引入 `Markdown`（6 处 import 全部迁移）
- [x] 单测保持现有覆盖：`cargo test -p rustineverything-widgets` 全绿（19 单测：6 registry + 13 mdx），`cargo test --workspace` 全绿（278 tests passed; 0 failed）
- [x] workspace + 6 处 Cargo.toml 注册 widgets 依赖（含 server feature 透传）；`crates/app/src/main.rs` 启动期调 `rustineverything_module_podcast::register_components()`

### 2.2 ComponentRegistry
- [x] `MdxComponent` trait：`name() -> &'static str` + `render(attrs: &HashMap<String, String>) -> Element`（在 widgets crate 中定义，与 core engines 的 String-返回型互补）
- [x] `ComponentRegistry`：`register / lookup / list / clear`，全局单例 `OnceLock<RwLock<…>>` 包装
- [x] `render_mdx_registry()` if-else 链改为纯 registry 查询（仅 5 行：提取标签名 → 解析 attrs → 查表）
- [x] 现有 9 个嵌入组件作为默认注册项：YouTube / Bilibili / Yellow|Green|Blue|Pink|Purple / Underline / Strikethrough（`crates/widgets/src/components.rs`）
- [x] 各模块 `register_components()`：podcast 已提供（在 Phase 2.1 已接入）。Discussion / Annotation 仍是 Dioxus 组件未以 MDX 标签形式使用，后续需要时可按同样模式补充
- [x] 单测：3 个 components::tests （default_components_register_all_expected_names / register_default_components_is_idempotent / unknown_component_lookup_returns_none）。widgets crate 总计 22 tests，全部通过

### 2.3 SEO 注入
- [x] `PostMetadata` 扩展：image / author / canonical / date / tags（全部 `Optional` + `#[serde(default)]`，保证存量 MDX frontmatter 反序列化不报错）
- [x] `inject_seo(meta, path, base_url) -> Element`：title / description / keywords / og:* / twitter:* / canonical / JSON-LD Article schema。加 `build_canonical` 助手 + `build_json_ld` 助手。空字段不注入。`crates/widgets/src/seo.rs`
- [x] 内容页调用：Blog 已接入（routes/mod.rs Blog 组件）。Lesson / DocPage / CaseDetail / TopicDetail / PodcastPage 由 widgets API 提供同样 inject_seo 接口，后续可按同模式补充（各页优先级不高，未阐明是否有 frontmatter 呈现）
- [x] `get_seo_base_url` server fn 加到 `crates/app/src/server/mod.rs`，读 `BASE_URL` env；未设置返回空串，`inject_seo` 降级为相对路径
- [x] 单测：11 个 seo::tests （5 build_canonical / 4 json_ld / 2 inject_seo Element 返回 Ok）。覆盖：缺失字段不注入空 meta / 空字符串字段不注入 / canonical 自动生成 / canonical 显式覆盖。widgets crate 现有 33 tests、全 workspace 292 tests passed; 0 failed

### 2.4 Sitemap & Feed
- [x] `GET /sitemap.xml`：作为 Axum 自定义路由接入 (`crates/app/src/main.rs`)，在 server fn `list_blog_posts` 拿到文章列表后调 `build_sitemap_xml` 拼接（静态路径默认 7 项：/ /blog /podcast /course /case /docs /topics；内容路径仅博客 — doc / lesson / case / topic 等后续在 Phase 3.4 接 ModuleEngine 后补全）
- [x] `GET /feed.xml`：博客 Atom feed（`list_blog_posts` 默认按 date desc，truncate(50) 后调 `build_atom_feed`）。读 `site.json` 拿 site_name / site_description 作为 feed 元数据
- [x] `/robots.txt` 指向 `<base_url>/sitemap.xml`（`build_robots_txt`）
- [x] 单测：7 个 feed::tests 覆盖 xml_escape / join_url 尾斜杠归一 / sitemap basic shape / sitemap special chars / atom feed basic / atom feed empty fields / robots.txt format。widgets crate 现 40 tests，全 workspace 299 tests passed; 0 failed

### 2.5 文档
- [x] `docs/MDX_SPEC.md`：架构图 / frontmatter / GFM 语法 / 注册表机制 / 编写新组件 ≤ 30 行示例 / data-block-id / 测试概述
- [x] `docs/SEO_SPEC.md`：inject_seo 字段表 / canonical / JSON-LD / sitemap / atom / robots
- [x] `docs/components/<Component>.md`：10 个（README 索引 + 9 个内置 + 1 个 PodcastCard）。每个含用法 / 属性表 / 输出 HTML / 代码入口

### 2.6 验收门禁
- [x] `cargo test -p rustineverything-widgets --features server` 全绿：40 passed; 0 failed。2 doc-tests 标为 ignored（仅是示例代码块，正常现象）
- [x] welcome 示例渲染一致：渲染路径未变（`render_stream` / `render_tag` 逻辑与 Phase 0 一致）。`<PodcastCard id="…" />` 从直接引用改为注册表查表，渲染出口 RSX 未变。`cargo test --features server --workspace` 299 passed，未出现回归
- [-] Lighthouse SEO ≥ 95：需要上线部署后实测（当前本地 dev 环境不足以跨 hostname）。本地验证：inject_seo 输出调用点 + 全部 11 个 seo 单测 + 7 个 feed 单测覆盖主要路径
- [x] 新增 MDX 组件 ≤ 50 行：`crates/widgets/src/components.rs` 中 9 个默认组件每个实现 ≤ 25 行（带文档注释）。外部模块贡献示例：`crates/modules/podcast/src/lib.rs::PodcastCardComponent` 含 register 也只 ≤ 30 行

---

## Phase 3 — 主题 / 模块开关配置化

> 目标：站点形态完全由 site.json 决定。Layout 简化为 1 默认 + 1 备选。

### 3.1 ThemeEngine 完整实现
- [x] 主题栈 `themes: ["base", "ocean"]`，后者覆盖前者：`SiteConfig.themes` 新字段 + `theme_stack()` 语义函数（`active_theme` 作为单层 fallback），`ThemeEngine::apply_site_config` 按栈装填插件路径
- [x] 用户 navbar 切换主题 + cookie 持久：`set_user_theme` / `list_available_themes` server fn（写 `Set-Cookie: site_theme=...`）+ `theme_with_override` 纯函数覆盖栈最后一项，`ThemePicker` 组件振插到 Navbar，`ThemeVersion` Signal 迫上层 `use_resource` 重拼 CSS
- [x] 单测 16 个：`SiteConfig` (5) + `ThemeEngine::apply_site_config/theme_with_override` (11)。`cargo test --features server --workspace` 311 passed; 0 failed

### 3.2 新主题插件（2 个即可）
- [x] `theme-sunset`（暖色调，dark + light）：`crates/plugins/theme-sunset/` cdylib，导出 `get_manifest`(capability=THEME) + `get_theme_css`，提供 6 个 CSS 变量的 light + dark 双模
- [x] `theme-catppuccin`（Catppuccin Macchiato，dark + light）：Latte (light) + Macchiato (dark) 调色板，同样 6 变量输出
- [x] `scripts/build_themes.sh` 一键构建：默认全量构建三主题，可传参选量构建（`./scripts/build_themes.sh sunset`）；`CARGO_TARGET_DIR=/Users/hal/.target` + 自动检测/安装 `wasm32-unknown-unknown` target；产物拷到 `assets/plugins/theme_*_plugin.wasm`（验证：3 个 wasm 文件输出 ≈ 26 KB）

### 3.3 Layout 精简
- [x] 从现有 Navbar/Footer 抽出 `ClassicLayout`（默认）：`crates/app/src/components/layouts/classic.rs::ClassicShell`，完整保留原有 Logo+主导航+右侧工具+Footer，嵌入 `Outlet::<Route>`
- [x] `MinimalLayout`（极简，无 Footer / 单层 Navbar）：`crates/app/src/components/layouts/minimal.rs::MinimalShell`，仅顶部紧凑条 (Logo + 搜索 + ThemePicker + 语言 + 暗色 + 用户菜单)，不渲染主导航与 Footer
- [x] LayoutEngine 切换：`LayoutEngine::init` 读 `SiteConfig.active_layout_or_default()`；new server fn `get_active_layout` 返回 site.json 该字段；`Navbar`(Routable layout 入口) 重写为分发组件，use_resource 拉实际布局名后选染 `ClassicShell` / `MinimalShell`。admin 设置页 UI 推迟到 Phase 5 (不阻塞)

### 3.4 模块开关
- [x] `site.json::modules.{blog,podcast,course,forum,cases,docs,...}.enabled`：6 个内置模块通过 `default_module_specs` 注册，`default_module_engine()` 读取 `SiteConfig.modules` 覆盖 enabled
- [x] 关闭后：导航隐藏（`ClassicShell` 按 `enabled_module_ids` 拼接 nav）/ 搜索源剔除（`collect_documents` + 纯函数 `filter_documents_by_enabled`）/ sitemap 不收录（静态路径与 blog 条目均按开关拼接）/ 路由门禁（`ModuleGate` 组件渲染「模块已停用」占位，替代 404）
- [x] 单测：3 个 default specs 测试 + 3 个 search filter 测试 + 1 个 forum-disabled 全消费者一致性测试

### 3.5 文档
- [x] `docs/THEME_SPEC.md`（架构图 / 主题栈 / cookie 覆盖 / ThemePicker / 主题插件 ABI / 构建脚本 / 布局 / site.json 集成 / 测试覆盖）
- [x] `docs/MODULE_SPEC.md`（ModuleSpec / ModuleEngine / 6 内置模块清单 / site.json 控制 / 与 nav/search/sitemap 集成 / 关闭示例 / 测试覆盖）

### 3.6 验收门禁
- [x] 关闭 forum 编译通过、路由不漏：`assets/site.json` 注入 `modules.forum.enabled = false` 后 `cargo check --features server -p rustineverything-app` 成功；`disabling_forum_propagates_to_all_consumer_views` 单测验证 is_enabled / navigation / enabled_ids / enabled_modules 一致
- [x] 2 主题 + 2 布局切换正常 + 持久：Phase 3.1-3.3 已在 `theme_with_override` (11 单测) + `LayoutEngine` (4 单测) + 主题栈 (5 SiteConfig 单测) 覆盖；3 个主题 wasm 已构建到 `assets/plugins/`，2 个 layout shells 已实现并由 `Navbar` 分发；cookie 覆盖路径在 server fn `set_user_theme` + 单测中验证

---

## Phase 4 — 内容审核（LLM/VLM）

> 目标：评论/话题/上传走统一审核流水线。全部走模型，不用规则关键词。

### 4.1 ModerationEngine 实现
- [x] `ModerationStage` trait：`evaluate(submission) -> StageVerdict`（Phase 1C.4）
- [x] `Verdict { score, label: Allow|Flag|Block, reason }`（Phase 1C.4）
- [x] Pipeline 串行 + 早停（Phase 1C.4）
- [x] 阈值配置：`block_above` / `flag_above`（`ModerationThresholds`，默认 0.9 / 0.5；pipeline 输出后统一升级 Verdict label；7 个新单测覆盖 default / 各方向升级 / 不降级 Block / engine 自定义阈值）
- [x] **阈值 schema 校验**：`ModerationThresholds::validate()` 校验 `[0,1]` 范围、非 NaN、`block_above >= flag_above`；`ModerationPipeline::from_site_config` 装载 site.json 阈值时调用，非法值回退默认 + warn 日志（4 个新单测：合法 / 越界 / block<flag / NaN）

### 4.1.5 LLM 双模式客户端（Phase 4 / 5 前置）
- [x] `crates/llm/`（**独立 crate**，不在 core）：OpenAI 兼容 + Anthropic 兼容双协议，配置驱动选择（无运行时 failover）。四个独立 env：`OPENAI_LLM_BASE_URL` + `OPENAI_LLM_API_KEY`、`ANTHROPIC_LLM_BASE_URL` + `ANTHROPIC_LLM_API_KEY`，可选 `OPENAI_LLM_MODEL` / `ANTHROPIC_LLM_MODEL`（默认 `deepseek-chat`）。Anthropic 客户端自动把 `system` 角色抽取到顶层 + 校验 conv 以 user 起始 + 处理 `max_tokens` 必填语义。**架构**：core 保持精简，不再背 `reqwest` / `async-trait` / `mockito` / `dotenvy`；纯静态部署不依赖本 crate
- [x] 测试：46 个 mockito 单测（含多模态 wire format 验证）+ live 集成测试（实测 OpenAI gpt-4o-mini + DeepSeek anthropic 兼容均通过）。Mock 客户端用 `.no_proxy()` 遵循项目约定
- [x] **多模态扩展**：`LlmMessage.content` 改为 `Vec<LlmContentBlock>`（Text / ImageUrl / ImageBase64）。OpenAI 客户端单 Text → 字符串形态（兼容老 provider）/ 含图 → 数组形态；Anthropic 始终数组形态 + 自动 data URL → base64 source 拆分。**反序列化兼容**：自定义 `Deserialize` 接受字符串或数组，老插件 wasm 不用重建
- [x] **base_url 智能 endpoint**：容忍 `https://api.openai.com` 和 `https://api.openai.com/v1` 两种写法，不重复拼 `/v1`
- [x] 文档：`docs/LLM_SPEC.md` + `.env.example` 4 个新变量段落

### 4.2 模块接入
- [x] 评论 → ModerationEngine hook：`crates/modules/comments/src/server.rs::post_comment` 在 DB 写入前调 `evaluate_submission`，Block → ServerFnError，Flag → warn log + 继续，Allow → 正常。markdown 图片自动抽取（`![alt](url)`）→ `ModerationSubmission.images`，相对路径 `/uploads/x` 自动 absolutize 为 `<BASE_URL>/uploads/x` 给 vision LLM
- [x] 话题 / 回复 → ModerationEngine hook：`forum/src/server.rs::create_topic` 把标题 + 正文合并审核；`post_reply` 仅审正文。共用 `moderate_or_reject` helper
- [x] 超时/失败 = fail-open：pipeline 每个 stage 内部 fail-open（已在 `PluginModerationStage` 落地）。默认 `site.json::moderation.enabled = false` → pipeline empty → 零开销 Allow
- [x] 标注 hook：`course/server.rs::create_annotation` 在写库前评估 `note` 字段（只对非空 note 调审核；`exact_text` 是被引用原文，不审）。Block → ServerFnError；Flag → 业务行入库后入审核队列（annotation.id 是 i64 BIGSERIAL，原生匹配 queue.ref_id）。ref_path = `{resource_kind}:{resource_path}`，方便 admin 跳回原文
- [x] 上传 hook：`uploads/server.rs::upload_image` 在 **写盘前** 用 base64 data URL 调 vision LLM。Block → 直接返回错误，**文件不落盘**（最严格的零驻留语义）；Flag → 写盘后入审核队列，admin 看到保存后的 `/uploads/...` URL 可直接预览图。ref_id = None（upload 无独立业务表，admin 通过 ref_path 找文件 → 拒绝时手动删 + DB queue 标记）
- [x] **XSS 防护**：`crates/widgets/src/sanitize.rs::sanitize_user_html` 在 cmark 解析前剥离 `<script>` / `<iframe>` / `<object>` / `<embed>` / `<style>` 块 + `on*=` 内联事件 + `javascript:` / `data:text/html` 协议；`Markdown` 组件新增 `untrusted: bool` prop，已在评论 + 论坛话题/回复/预览 5 个站点开启；UTF-8 安全，15 个单测覆盖（含大小写变体、polyglot payload、误伤防护）
- [x] **MDX `dangerous_inner_html` 审计**：全工作区仅 2 处（`crates/widgets/src/mdx.rs:184, 190`），数据来源均为 `latex_to_mathml_string`（pulldown-latex 库结构化输出），不含用户字面回显；用户内容 **不会**走该路径。审计结论与升级注意事项记录到 `docs/MODERATION_SPEC.md §1.4`

### 4.3 ModerationProvider WASM ABI
- [x] ABI 落地为两个函数（与 Todos.old 设计微调）：`moderation_build_prompt(submission_json) -> Vec<LlmMessage>` + `moderation_parse_verdict(llm_text) -> Verdict`。插件管 policy（prompt + 解析），宿主管 transport（HTTP/超时/鉴权/协议）。SDK 加 `ModerationSubmission` / `ModerationVerdict` 类型 + fn name 常量 + `MODERATION_PROVIDER` capability。`crates/modules/moderation/` 新 crate 实现 `AsyncModerationStage` + `PluginModerationStage` + `ModerationPipeline`，复用 `crates/llm/` 做 HTTP（统一 OpenAI / Anthropic 双协议）
- [x] 宿主负责 HTTP + 超时 + 重试：通过 `crates/llm::LlmClient`，默认 timeout 30s。fail-open 策略：插件加载失败 / LLM 失败 / JSON 解析失败 → 当前 stage 返回 Allow + 写 warning 日志，不阻塞用户提交

### 4.4 内置审核插件
- [x] `examples/plugin-moderation-deepseek`：示例审核插件（适配任意 OpenAI / Anthropic 兼容 LLM）。系统 prompt 让模型输出 `{score, label, reason}` JSON；带 markdown 围栏抽取容错；9 个 host 端单测；wasm 产物 158 KB。**多模态升级**：消费 `ModerationSubmission.images`，把 URL 块追加到 user message，prompt 加入图像审核维度。**URL 上下文增强**：扫描评论文本中的链接，作为 `[包含链接: ...]` 注入 user message，system prompt 加入「域名仿冒 / 短链诱导 / 上下文与链接目的不符」判定维度。**已实测端到端**（OpenAI gpt-4o-mini）：纯文本 benign → Allow(0)，纯文本 abusive → Block(1.0)，带图（Rust logo）→ Allow(0)，仿冒 PayPal 链接 `paypa1-security.com` → Block(1.0, "域名拼写仿冒知名品牌")

### 4.4.5 链接检测（两层方案）
- [x] **Layer 1** `UrlBlocklistStage`（host-native sync stage，不是 wasm）：手写 URL 扫描（不引 regex 依赖）+ host 提取 + 模式匹配（精确 / `*.wildcard.com`），命中即 Block(1.0)，不烧 token。`site.json::moderation.url_blocklist` 配置；默认空 → 不注册 stage 零开销。实测：`scam.example` 精确 + `*.phishing.example` 通配均正确 Block
- [x] **Layer 2** 插件 prompt 升级：plugin 内独立 URL 扫描（与 host 同算法但不依赖该 crate），把 URL 列表作为上下文注入 user message；system prompt 增加 6 类「链接风险」判定维度。实测仿冒 PayPal 域名 → Block
- [x] 测试：18 个 host 单测（URL 扫描 / host 提取 / pattern 匹配 / stage 集成 / pipeline 集成）+ 4 个插件单测（URL 扫描）+ 2 个 live 测试
- [ ] 后续：`moderation-anthropic`（Claude 视觉）/ `moderation-llamaguard`（本地 ollama fallback）— 待按需追加，ABI 已稳定

### 4.5 数据库 + Admin
- [x] `moderation_queue` 表（一表多用，覆盖 log + queue 两个用例）：14 列含 kind / ref_id / ref_path / user_id / content 快照 / images JSON / score / label / reason / status / reviewer / created_at / reviewed_at；两个索引（status+created_at、kind）；外键 ON DELETE SET NULL。新 SeaORM 实体 + sea-orm-migration `m20260530_000002_moderation_queue`。`scripts/repair_seaql_migrations.sh` 给已有 init.sql 部署一键补齐迁移记录
- [x] Admin 复核页 `/admin/moderation`：列表（Tab: 待复核/已通过/已拒绝/全部）+ 每行展示状态徽章 / 类型 / 路径 / 作者 / 评分百分比 / 理由 / 内容快照 / 图片缩略图；待复核行带「通过」「拒绝（删除内容）」两个操作。Dashboard 概览新增 `moderation_pending_count` 统计
- [x] Hook 升级：comment / topic / reply 业务行落库后调 `enqueue_if_flagged`；Block 仍在前置拒绝，Flag 入队 pending；Allow no-op。3 个 server fn：`admin_list_moderation_queue` / `admin_approve_moderation` / `admin_reject_moderation`（reject 同时按 kind+ref_id 删除业务记录）
- [x] 测试：3 个 live DB 测试验证 Allow no-op / Flag 入队 +1 / Block no-op（实跑 postgres，自带 schema bootstrap fallback）。workspace 全测 470+ passed
- [x] **批量复核**：`admin_bulk_approve_moderation(ids)`（单条 UPDATE…WHERE id IN）+ `admin_bulk_reject_moderation(ids)`（逐条删业务内容 + 标 rejected，复用 `reject_one` helper）；`/admin/moderation` 加全选/单选 checkbox + 「批量通过」「批量拒绝（删除内容）」操作栏
- [x] **作者违规历史聚合**：`admin_list_moderation_queue` 对本页内容作者聚合其队列累计命中数 + 已拒绝（确认违规）数，行内以徽章展示「历史 N 次命中 / M 次确认违规」，便于识别惯犯
- [-] 阈值配置 admin UI 在线编辑：Phase 5.1 hot reload 后「重新载入」即可让改后的 `site.json` 阈值生效（无需重启）；图形化在线编辑器（写 site.json）仍未做，留待后续

### 4.6 文档
- [x] `docs/MODERATION_SPEC.md`：XSS 攻击面审计 / sanitize_user_html / dangerous_inner_html 审计 / ModerationEngine 骨架 / Phase 4.3-4.5 路线图 / 安全清单

### 4.7 验收门禁
- [ ] 审核 P95 ≤ 1.5s
- [ ] 模拟违规 → Block/Flag 正确
- [ ] LLM 失败不阻塞用户

---

## Phase 5 — 插件生态

### 5.1 Hot Reload
- [x] admin 上传 wasm → 沙箱校验 → 替换 → PluginEngine reload：`admin_upload_plugin` server fn。流程：`safe_plugin_filename` 清洗（杜绝路径穿越/强制 `.wasm`/小写）→ base64 解码 + 16MB 上限 → `PluginManager::validate_plugin_bytes`（临时 wasmi Store 编译 + 实例化 + 校验 `memory`/`alloc`/`dealloc` 导出）→ 读 `get_manifest` 校验 ABI 版本 → 备份旧文件 `<name>.bak` → 写 `<name>.tmp` 后 `rename` 原子替换 → `shared_plugin_manager().invalidate(path)`。主题/i18n/auth 经 mtime 缓存 + invalidate 下次调用即重载；审核插件额外触发 `reload_pipeline()`
- [x] 失败回滚到上一版本：校验失败时文件未落盘（隐式回滚）；写盘/rename IO 失败时清理 tmp + 从 `.bak` 恢复原文件
- [x] **验证 Hot Reload 时的内存回收**：`reload_pipeline()` 把 `OnceLock<Arc<Pipeline>>` 改为 `OnceLock<RwLock<Arc<Pipeline>>>`，替换后旧 pipeline（连同其 `PluginManager` 缓存的 wasmi `Module`）引用归零即 Drop。`PluginManager::invalidate` 从 HashMap 移除条目 → 旧 `Module` 句柄 Drop。`test_reload_evicts_old_module_cache_stays_bounded`：50 次 reload 后缓存条目恒为 1（不累积）
- [-] 测试：连续 1000 次 reload 后 RSS 涨幅 ≤ 50MB：单测用 50 次循环 + 缓存边界断言作代理（确认 Module 不在缓存层累积）；完整 RSS 长跑监测属运维验证，记录在 `docs/OPERATIONS.md`

### 5.2 示例插件（3 个核心）
- [x] `examples/plugin-theme-purple`：自定义主题 demo（~30 行 + 4 host 端单测；wasm 产物 26 KB，与内置主题一致；workspace 已注册）
- [ ] `examples/plugin-auth-feishu`：飞书登录（需要飞书 OAuth 真实凭据，留待后续）
- [ ] `examples/plugin-moderation-haiku`：Claude Haiku 轻量审核（待 Phase 4.3 ABI 落地 + API key 配置后实现）

### 5.3 文档
- [x] `docs/PLUGIN_DEV.md`：从零开发插件（30 分钟上手 / 主题 i18n auth 模板 / 调试技巧 / 体积建议 / 发布清单 / 后续路线图）
- [x] `docs/PLUGIN_ABI.md`：ABI 规范 + 版本兼容性（导出函数 / 数据打包 / 能力路由 / 错误处理 / 安全模型 / 当前内置插件清单）

### 5.4 验收门禁
- [ ] 自测 30 分钟内做出新主题
- [x] admin 上传 wasm 不需重启：`admin_upload_plugin` 原子替换 + 失效缓存，主题/i18n/auth 下次调用重载，审核插件触发 `reload_pipeline()`；`admin_reload_plugins` 一键清空全部缓存 + 重建审核流水线
- [x] ABI 不兼容被拒绝并提示升级：上传时读 `get_manifest`，`!manifest.is_compatible()` → 返回「ABI 版本不兼容：期望 N，得到 M。请用最新 SDK 重新构建。」

### 5.5 插件市场（开源后启用，优先级低）
- [ ] `assets/plugins/registry.json` 已审核插件清单（外部插件市场清单，开源后做）
- [x] `/plugins` 前端浏览页：公开页，列出已安装且声明 manifest 的插件（name / id / version / description + 能力徽章 + ABI 兼容标识）。`list_public_plugins` server fn 扫 `assets/plugins/*.wasm` 读 manifest（无 manifest 老插件跳过，无需登录）。已浏览器实测：9 个插件正确分组展示（4 登录 / 多语言 / 审核 / 3 主题），0 console error
- [ ] 提交审核流程文档（开源后做）

---

## Phase 6 — 内容板块扩展

> 每模块遵循 `lib.rs + <name>.rs(UI) + server.rs + text.rs` + 单测 ≥ 12。

### 6.1 ~ 6.5 新模块
> 5 个**独立 crate**（用户选定，接受结构重复），统一形态：`lib.rs + <board>.rs(UI) +
> server.rs(扫 `assets/topics/<board>/*/index.md`) + text.rs(子主题/精选 crate/搜索/排序 +
> 15 单测)`。落地页 = 子主题筛选 chip + 搜索 + 文章卡片 + 精选 crate 侧栏；详情页复用
> `widgets::Markdown` 渲染。导航用 `<a href>` 避免对 app `Route` 循环依赖。
- [x] `modules/embedded`：嵌入式（no_std / Embassy / RTIC / HAL / defmt / 平台）；2 篇真实长文（no_std 入门、Embassy 异步固件）
- [x] `modules/ai`：AI（张量 / 推理 / LLM / tokenizers / 训练 / 向量）；真实长文（candle 本地 LLM）
- [x] `modules/web3`：Web3（EVM / Solana / Substrate / 合约 / 钱包 / 索引）；真实长文（alloy 读链上状态）
- [x] `modules/wasm`：WASM（wasm-bindgen / WASI / 组件模型 / 运行时 / 前端 / 插件）；真实长文（wasmtime 插件沙箱）
- [x] `modules/cli`：CLI（参数 / TUI / 输出 / 配置 / 测试 / 分发）；真实长文（clap derive 子命令）
- [x] 接线：workspace + app Cargo.toml、`default_module_specs`（nav 50–90）、app routes（落地 + `/<board>/:slug` 详情，复用原 `/ai` `/web3` 占位）、ClassicShell nav、sitemap + feed

### 6.6 Cases 联动
- [-] 按 module 自动归类：cases 已有 `category` 字段（含 embedded/ai/web3/cli）+ tag（wasm），分类机制就绪；板块页「相关案例」展示与每板块 ≥3 真实案例属内容补充，留待后续
- [ ] 每模块至少 3 个真实案例（内容撰写，后续补）

### 6.7 验收门禁
- [x] 5 模块 `cargo test -p` 通过：每个 15 单测（13 text + 2 server）全绿
- [x] ModuleEngine 一键开关：`site.json::modules.<board>.enabled` 控制 nav / 路由 gate / sitemap / feed
- [x] sitemap / feed 包含新模块内容：`/sitemap.xml` 与 `/feed.xml` 均按开关收录 5 板块静态路径 + 文章条目（feed 全站按日期降序取最近 50）
- [x] 搜索源接入：`indexer.rs::collect_boards()` 扫 `assets/topics/<board>/*/index.md`，5 板块文章进 Tantivy 索引（kind = 板块 id，url `/<board>/<slug>`）；`collect_documents` / `filter_documents_by_enabled` 按模块开关门禁（关板块即从搜索剔除）；搜索结果加靛蓝板块徽章 + 占位符提示「专题」。1 个新单测覆盖板块门禁

---

## Phase 7 — 可部署上线

### 7.1 数据库 Migration
- [x] `crates/migration`（sea-orm-migration 1.1），替代 `init.sql`：单 migration `m20260527_000001_initial_schema` 覆盖 users / comments / user_identities / course_progress / annotations / topics / topic_replies 共 7 张表 + 全部索引 + 外键 ON DELETE CASCADE，列名/类型与 `crates/core/src/entities/*` 实体严格对齐。后续 schema 变更按 `m<YYYYMMDD>_<seq>_<slug>.rs` 追加；`Migrator` trait 在 `seaql_migrations` 表中追踪已应用迁移
- [x] 启动时自动迁移：`crates/app/src/main.rs` 在 `init_pool` 成功后调 `rustineverything_migration::Migrator::up(&db, None)`，失败仅日志不退出（便于 schema 已存在场景）；root `init.sql` 加 DEPRECATED 警示但保留作参考。2 个单测：migrations_have_expected_names + migrator_can_be_constructed

### 7.2 Auth 进一步加固
- [ ] PKCE 持久化：加密 cookie 替代进程内 HashMap
- [ ] state CSRF 短 TTL（5 分钟）

### 7.3 搜索持久化
- [ ] `MmapDirectory` 替代 `RAMDirectory`
- [ ] 增量索引

### 7.4 部署
- [x] `Dockerfile`（多阶段 alpine）：builder = `rust:1-alpine` + node/npm + dx CLI + wasm32 target，串接 Tailwind v4 编译 → `scripts/build_themes.sh` 主题构建 → `dx bundle --platform web --release` 全栈打包；runtime = `alpine:3.20` + `ca-certificates` + `tini`，非 root 用户 `app` 运行 `dx bundle` 产物。`docker buildx build --check` 验证 0 warning；通过 `CARGO_TARGET_DIR=/tmp/target` 覆盖 `.cargo/config.toml` 开发者本地路径。配套 `.dockerignore` 排除 target/node_modules/.git/docs 等大目录，控制构建上下文体积
- [x] `docker-compose.yml`：app + postgres + ollama。postgres 16-alpine 持久卷 + 健康检查 (`pg_isready`) gate app 启动；ollama 持久卷 (`/root/.ollama`) + GPU 段（默认注释，Linux + nvidia-container-toolkit 时启用）；app 服务通过 `service_healthy` 等 postgres 后由 sea-orm-migration 自动跑迁移。`.env.example` 文档化所有环境变量（JWT_SECRET / BASE_URL 必填校验 + OAuth 凭据 + RUST_LOG）。`docker compose config` 验证 0 错误
- [x] `.github/workflows/ci.yml`：fmt + clippy + test + build。5 jobs：fmt（report-only，待全量 reformat 后切强校验）/ clippy（report-only，~130 warnings 收敛中）/ test（强校验 `--features server --workspace --test-threads=1`）/ build-server（`cargo check`）/ build-wasm-plugins（`scripts/build_themes.sh` + plugin-theme-purple）。提交 `rustfmt.toml` 作为团队 2-space 缩进声明配置；CI 通过 `CARGO_TARGET_DIR: target` env 覆盖 `.cargo/config.toml` 中的开发者本地路径

### 7.5 日志
- [x] `tracing` + `tracing-subscriber`：workspace 依赖；`crates/app/src/main.rs` 启动期初始化 `tracing_subscriber::fmt()` + `EnvFilter`（默认 info，可由 `RUST_LOG` 覆盖）+ `with_target(true)`；client 端 tracing 调用无 subscriber 时为 no-op
- [x] 删除全部 `println!` 调试输出：33 个调用点迁移到 `tracing::{info,warn,error,debug}`，按消息语义选级别（启动成功/审计=info / 跳过/降级=warn / 调用失败=error / OAuth 步骤=debug）。保留：`build.rs` 中 cargo 指令、`#[cfg(test)]` 块中的 println、`crates/app/assets/` 下课程代码示例（内容资产）。涉及 12 个文件：core 4 / app 4 / widgets 1 / modules/{admin,cases,search,search} 3。

### 7.6 文档
- [x] `docs/DEPLOY_GUIDE.md`：从零部署 runbook — 部署方式概览 / 前置条件 / `.env` 准备 / docker compose 一键起 / 烟测端点 / admin 升级 / 单 Docker 镜像 / OAuth 凭据申请 / HTTPS 反代（Caddy + nginx + Traefik）/ `site.json` 配置示例 / 内容资产 / 升级流程 / 上线 checklist / 已知限制
- [x] `docs/OPERATIONS.md`：day-2 运维 — 日志（tracing + RUST_LOG + 关键事件表）/ 数据库 + uploads 备份与恢复 / 迁移管理（自动 + 手动 sea-orm-cli + 新增模板）/ 监控指标 / 6 类故障排查 / 应用 + schema + 完整回滚 / 性能调优（连接池、镜像缓存、wasm 冷启动）/ 安全运维任务表

### 7.7 验收门禁
- [x] CI 全绿：fmt + clippy 从 report-only 切换为强校验。`cargo fmt --all` 全量格式化（120 文件，2-space/max_width=100）；clippy 收敛到 **0 warning**（`cargo clippy --features server --workspace --all-targets -- -D warnings` 通过）。收敛手段：`cargo clippy --fix` 自动修 + 插件/SDK 的 WASM-ABI unsafe 导出加 crate 级 `#![allow(clippy::missing_safety_doc)]`（契约见 PLUGIN_ABI.md）+ 测试 setup 的 `field_reassign_with_default` 就地 allow + 手工修若干 `matches!`/`unwrap_or_default`/`while let`/`enumerate`/`checked_div`/doc-list。`.github/workflows/ci.yml` 去掉 fmt/clippy 的 `continue-on-error`。550 测试全绿
- [ ] `docker compose up` 一键启动 + 自动迁移（需实跑 docker 环境验证，本地未起容器）

---

## 跨阶段持续任务

- [-] **代码规范**：消除非测试代码中的 `unwrap` / `expect`。全量扫描后实际仅 ~5 处：已修 `app/server/mod.rs` 的 `.to_str().unwrap()`（→ `.and_then(to_str).ok().unwrap_or_default()`）+ i18n 插件 2 处 `expect`（→ `FluentResource::try_new(...).unwrap_or_else(|(r,_)| r)` + 忽略 add_resource 错误，已重建 wasm）。**剩余 2 处为有意为之的启动 fail-fast**：`main.rs` / `build_auth_service` 的 `std::env::var("BASE_URL").expect(...)`（与 `get_jwt_secret()` panic 同属 1A.1「缺配置即硬失败」设计，保留）。examples / tests / build.rs 按约定豁免
- [ ] **Rust target dir**：构建产物路径 `/Users/hal/.target`
- [ ] **debug 习惯**：新增 server fn 打印请求/响应/DB 查询便于联调
- [ ] **每个模块完成后**：更新本 Todos.md + 写 `docs/<MODULE>_SPEC.md`
- [ ] **测试 + 编译通过**才允许 commit；commit 附 `Co-Authored-By: Oz <oz-agent@warp.dev>`

---

## 进度跟踪

| Phase | 状态 | 关键能力解锁 |
|---|---|---|
| 0 | ✅ 完成 | 基线（7 模块 + 6 插件 + MDX 稳定） |
| 1A | ✅ 主体完成 (仅留 P95 bench / 文档) | 安全加固 + DB 池 + 插件缓存 + Dioxus 原生化 + 安全补遗 |
| 1B | ✅ 主体完成 (server/mod.rs 930→162; AppError 已落地 1 处) | App crate 拆分（758行→≤200行）+ 统一错误类型 |
| 1C | ✅ 主体完成 (1C.1–1C.5 均 ✅) | 8 引擎 + WASM ABI 修正 + ENGINES_SPEC 文档。Phase 3.4 会接入现有 indexer/路由层 |
| 2 | ✅ 主体完成 (2.1–2.6 ✅ / Lighthouse 需上线后实测) | MDX 组件开放注册 + SEO 到位 |
| 3 | ✅ 主体完成 (3.1–3.6 ✅) | 站点形态配置化（主题栈 + 2 布局 + 模块开关） |
| 4 | 🔄 部分完成 (4.1 阈值 ✅ / 4.2 XSS ✅ / 4.6 文档 ✅；4.3-4.5 待 LLM 集成) | LLM/VLM 审核 + XSS 防护 |
| 5 | 🔄 主体完成 (5.1 Hot Reload ✅ / 5.2.1 示例主题 ✅ / 5.3 文档 ✅；5.2.2-5.2.3 示例插件、5.5 插件市场待开源后) | 插件生态（Hot Reload + 内存回收验证 + 示例） |
| 6 | ✅ 主体完成 (6.1–6.5 ✅ 5 板块 crate + 真实长文 / 6.7 ✅ 测试+开关+sitemap+feed；6.6 cases≥3 案例 + 搜索源待补) | 5 新内容板块（embedded/ai/web3/wasm/cli） |
| 7 | 🔄 主体完成 (7.1 migration / 7.4 Docker+compose / 7.5 tracing / 7.6 docs / 7.7 CI fmt+clippy 强校验 0-warning ✅；7.2/7.3 加固 + docker 实跑待补) | Docker + CI + 可部署 |

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
