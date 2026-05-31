# 开发计划 — Phase 8 安全 & 性能硬化冲刺

> 上一阶段（Phase 0 – Phase 7.7，全部主体完成）归档在 [`Todos.phase1-7.md`](Todos.phase1-7.md)。
> 本文档承接 2026-05-31 三 agent 并行 code review 的综合结果，按 **8 个独立可发布 PR** 顺序推进。
> 协作约定 / 提交习惯 / 测试门禁继承自上一阶段，简短复述见文末「持续约定」。

## 本阶段目标

把"单作者站可跑"提升到"小流量公网可上"的硬化等级。聚焦三类：

1. **安全**：消除可被匿名 / 非特权用户触发的 DoS / 越权 / 注入面
2. **性能**：拆掉 hot-path 上同步阻塞 + 重复解析，保 5–50 RPS 不爆 worker
3. **结构**：清掉死代码（EngineRegistry）+ 消除 ModuleEngine 仍存的硬编码副本

**不在本阶段范围**（后续单独规划）：
- 5 个 Phase-6 板块 crate（`modules/{ai,cli,embedded,wasm,web3}`）合并 —— 用户决定暂不动
- 全平台 desktop / mobile 迁移（含剩余 16 处 `dioxus::document::eval`）
- 插件 ABI v2（Extism / wit-bindgen）—— 留 Phase 9
- Prometheus metrics —— 见 8.8 末尾 deferred

---

## Design 默认值（开工前的关键选择）

| # | 决策点 | 选择 | 理由 |
|---|---|---|---|
| 1 | JWT role 变更失效策略 | `require_admin()` 内重查 DB role | 比 token_version 改动小；JWT TTL 保持 7 天避免 refresh 复杂度 |
| 2 | `user_identity.access_token` 列 | **删列**（dead-stored，无解密路径） | YAGNI；未来真要 token 转发再加 |
| 3 | 剩余 16 处 `dioxus::document::eval` | 留着；删 `main.rs` 跨平台注释 | 全迁成本大、无 desktop 部署需求 |
| 4 | MDX `register_components` 机制 | 留着 | 已落、podcast 在用 |
| 5 | EngineRegistry / lifecycle 抽象 | **删 dead code** + LayoutPack 加 `render` 方法 | "Don't add features for hypothetical future" |
| 6 | rate limiting 落点 | Pingora 网关层 | 真实 client IP 在那里；app 端不动 |
| 7 | Markdown / list 缓存 invalidation | mtime-based（同 plugin cache） | 与现有 `PluginManager::get_or_load_module` 一致 |
| 8 | comments 索引迁移 | 新建 `m20260601_000003_*` migration | sea-orm-migration 累加，不动既有 |

---

## Phase 8.1 — WASM 沙箱硬化（🔴 Critical）

> 来源：security agent + arch agent 双重命中。当前任意插件（含 admin 上传的）`start` / `alloc` 无限循环都能卡死 tokio worker；输出读到 `vec![0u8; result_len]` 时才查 cap → 恶意插件可让 host 先分配 2 GiB。

- [x] **Output buffer 在 alloc 之前 clamp**（`crates/core/src/lib.rs:159`）
   - `invoke_module_sync` 解出 `result_len` 后先 clamp 到 `sandbox.output_limit` 再分配 `Vec`；负值 / 超限均报错
   - 测试：`output_length_is_clamped_before_alloc` 把 limit 调到 1 字节跑真实插件，验证拒绝 + 错误消息含 `exceeds limit`
- [x] **wasmi fuel 限制**（`crates/core/src/lib.rs:54`）
   - `Engine::new(Config::default().consume_fuel(true))`，`Store::set_fuel(sandbox.fuel)` 每次调用前注入
   - 默认 100M，env `WASM_FUEL_LIMIT` 覆盖
   - 测试：`fuel_exhaustion_traps_quickly` 设 fuel=1 → 真实插件 trap，且耗时 < 1s
- [x] **内存 page limiter**（`crates/core/src/lib.rs`）
   - 用 wasmi 内置 `StoreLimits::memory_size(128 * 64KiB)` + `Store::limiter` 装上
   - env `WASM_MEMORY_PAGES` 覆盖
- [x] **执行 timeout**（`crates/core/src/lib.rs`）
   - `invoke_module` / `validate_plugin_bytes` 用 `tokio::time::timeout(sandbox.timeout_secs)` 包裹
   - env `WASM_INVOKE_TIMEOUT_SECS` 覆盖（默认 5s）
- [x] **同步 wasmi 调用走 spawn_blocking**
   - `invoke_module` 改 async：body 用 `tokio::task::spawn_blocking`
   - 级联 `PluginManager::call_*` / `aggregate_theme_css*` / `validate_plugin_bytes` → async
   - `PluginEngine::call / strict_call / try_get_manifest / get_manifest / filter_by_capability` → async
   - 影响 caller：auth (`list_available_providers` / `prepare_login` / `handle_callback`) + `app/server/mod.rs` + `admin::admin_upload_plugin` + `moderation::PluginModerationStage` 全部加 `.await`
- [x] 文档：`docs/PLUGIN_DEV.md` 新增「9.1 沙箱约束（Phase 8.1）」表格 + 实践指导
- [x] CI：`cargo test -p app-core --features server --lib` 129 通过；`cargo clippy --workspace --features server --all-targets -- -D warnings` 0 warning

**完成定义**：单个恶意 wasm（构造死循环 / alloc 爆 / 输出超长）均被 host 拒绝且不阻塞其他请求；tokio worker 不再被 wasmi 同步占用。

---

## Phase 8.2 — Auth / 输入校验表面硬化（🔴 Critical）

> 来源：security agent 命中 4 项。`/api/upload` 完全不鉴权 + 默认 secret 通过启动校验 + forum 路径穿越 + `state` RNG 未文档化。

- [x] **`/api/upload` 加 require_session**（`crates/modules/uploads/src/server.rs:88`）
   - 函数顶端调 `require_session()?`；匿名调用直接 401
   - 文档化「如需匿名上传通道单开 `/api/upload/public`」
- [x] **`.env.example` 默认 placeholder 拒绝**（`crates/core/src/session.rs::get_jwt_secret` + `crates/app/src/main.rs:63`）
   - 新增 `looks_like_placeholder` + `assert_not_placeholder`：模式 `change-me` / `changeme` / `your-` / `<your` / `replace-me` / `placeholder`，大小写不敏感
   - 故意避开 `password` 子串以免误伤真实凭据
   - `JWT_SECRET` / `BASE_URL` / `DATABASE_URL` 启动时三个 env 都过 placeholder 校验
   - 单测：常见占位模板被拒，真实值（含 `Sup3r!StrongPa55word2026` / `http://127.0.0.1:8080`）通过
- [x] **forum `ref_path` 防目录穿越**（`crates/modules/forum/src/server.rs:122-133,206-253`）
   - 新增 `safe_join_under(sub_root, raw)`：字符级拒 `..` / 绝对路径，存在的路径再做 `canonicalize` 前缀校验
   - `resolve_ref_title` 全部 join 改走该 helper（blog/doc/course/lesson/case 全覆盖）
   - 单测：`safe_join_rejects_dotdot_segments` + `resolve_ref_title_rejects_traversal`
- [x] **OAuth `state` / `code_verifier` 改用 `OsRng`**（`crates/core/src/auth/mod.rs:240-241,253-254`）
   - 改用 `rand::rngs::OsRng` + `TryRngCore::unwrap_err`（rand 0.9 API）
   - 代码注释强调禁止退回 `ThreadRng` / `SmallRng`
- [x] **删 `user_identity.access_token` 列**（Design 默认 #2）
   - 新建 migration `m20260601_000003_drop_access_token`：ALTER TABLE DROP COLUMN + 反向 down 重加列
   - `entities/user_identity.rs` 删字段并添加 Phase 8.2 注释说明
   - `handle_callback` / `sync_user_to_db` 删 access_token 持久化路径（保留 OAuth `Bearer` 调用 profile_url 路径）
   - 调整 rollback live test 调用签名

**完成定义**：未登录用户调任何写端点（含 `/api/upload`）被拒；新部署用模板默认值启动 panic；forum 输入无法越出 assets 目录。

---

## Phase 8.3 — Pingora 网关硬化（🟡 Warning，上线前必做）

> 来源：security agent。全无安全响应头 + XFF 头可被客户端伪造 + 全无 rate limit。

- [x] **响应注入安全头**（`crates/gateway/src/main.rs`）
   - `AppGateway::response_filter` 注入 HSTS / X-Content-Type-Options / X-Frame-Options / Referrer-Policy / CSP / Server，所有 `insert_header` 防重复
   - CSP 默认值含 `object-src 'none'; base-uri 'self'; frame-ancestors 'none'`，env `CSP_POLICY` 覆盖
- [x] **X-Forwarded-For 改 insert / 清 Forwarded**（`crates/gateway/src/main.rs:51-56`）
   - 改 `insert_header` 覆盖客户端伪造 + `remove_header("Forwarded")` strip RFC 7239
   - 拿不到 socket 时也显式 `remove_header("X-Forwarded-For")` 避免毒化
- [x] **rate limiting**（`crates/gateway/Cargo.toml` + main.rs）
   - 引入 `governor = "0.10"` + `once_cell`：`DefaultKeyedRateLimiter<IpAddr>` 全局静态实例
   - 写端点（`/api/auth/`/`/api/upload`/`/api/comments/`/`/api/topics/`/`/api/admin/`/`/api/forum/`/`/api/i18n/translate`）10 req/min；其他 60 req/min
   - 触发返 429 + `Retry-After: 60`；env `RATE_LIMIT_DISABLE=true` / `RATE_LIMIT_WRITE_PER_MIN` / `RATE_LIMIT_READ_PER_MIN` 覆盖
   - 直接读 `session.client_addr()` 的 socket IP（不信任客户端 XFF）
- [x] 文档：`docs/DEPLOY_GUIDE.md §6.1` 补「安全响应头 + 限流（Phase 8.3）」小节 + curl 边缘验证脚本
- [x] 单测：`write_path_classification` / `rate_limit_env_disable_flag` / `keyed_limiter_enforces_quota` 三组覆盖路径分类、env 开关、配额耗尽

**完成定义**：浏览器 devtools 看到响应头齐；攻击者无法把伪造的 XFF 灌进日志 / 限流决策；典型 DoS 模式被 429 短路。

---

## Phase 8.4 — DB 性能 & 索引（🟡 Warning）

> 来源：performance agent。SeaORM 用全默认 ConnectOptions + `comments(blog_id, created_at)` 缺索引 + admin_overview 7 个 COUNT 串行。

- [x] **SeaORM ConnectOptions 显式 tuning**（`crates/core/src/db/pool.rs:20,35`）
   - 提炼 `build_connect_options(url)`：max 32 / min 2 / connect 5s / acquire 5s / idle 10m，
     `sqlx_logging_level(Debug)` 避免 INFO 刷屏
   - env 覆盖：`DB_MAX_CONN` / `DB_MIN_CONN` / `DB_CONNECT_TIMEOUT_SECS` / `DB_ACQUIRE_TIMEOUT_SECS` / `DB_IDLE_TIMEOUT_SECS`
   - 单测：`env_override_helpers_round_trip` 验证 reader helper 默认值 + override 行为
- [x] **新建 migration：comments 索引**（`crates/migration/src/m20260601_000004_comments_index.rs`）
   - `CREATE INDEX IF NOT EXISTS idx_comments_blog_created ON comments(blog_id, created_at DESC)`
   - `down()` 反向 drop
- [x] **admin_overview 7 COUNT 并行**（`crates/modules/admin/src/server.rs:233-278`）
   - `tokio::try_join!` 并行 7 个 `count(...).await`，从 sum → max(单查)
   - 现有 `admin_overview` 集成测试覆盖正确性；性能验证留运维 P95
- [x] **moderation queue author 历史聚合 SQL pushdown**（`crates/modules/admin/src/server.rs:920-949`）
   - 把 Rust 端 Vec 聚合改成 2 个 GROUP BY query（total / rejected），用 `Expr::col(Id).count()` + `column_as` + `into_tuple` 返回 `(user_id, count)`
   - 网络 round-trip 固定 ≤ 3 次（lookup + 2 GROUP BY），不再随 author × 历史长度膨胀
- [x] EXPLAIN 验证：留 `docs/OPERATIONS.md` runbook（迁移落地后跑一次 `EXPLAIN`），暂不强制

**完成定义**：admin dashboard < 200ms（10K 行场景）；评论列表索引扫描；DB pool 行为可预测。

---

## Phase 8.5 — Hot-path 缓存（🟡 Warning，规模化前必做）

> 来源：performance agent。WASM 调用 / Markdown 解析 / list_* 重读全部按"每请求"做，浪费 5-50× 的工作。

- [ ] **theme CSS 启动期 + 热重载期算一次缓存**（`crates/app/src/server/mod.rs::get_aggregated_theme_css`）
   - `OnceLock<RwLock<Arc<String>>>`
   - `shared_plugin_manager().invalidate_all()` 时同步 invalidate
   - 单测：调 100 次后 WASM 实际只被调 1 次
- [ ] **i18n 翻译表整表缓存**（`crates/app/src/server/mod.rs::translate_server` + 插件 ABI）
   - 插件加 `get_all(lang) -> JSON map` export（同 plugin ABI v1，向后兼容）
   - host 端按 `(mtime, lang)` cache `HashMap<key, String>`
   - 单测：navbar 10 翻译键 → WASM 调用 1 次
- [ ] **Markdown 渲染 mtime cache**（`crates/widgets/src/mdx.rs`）
   - `OnceLock<DashMap<PathBuf, (SystemTime, Arc<RenderedHtml>)>>`（结构待定，可能存中间 token stream）
   - 现实约束：Markdown 输出是 `Element` 不易 cache，可能要 cache `Vec<Event>` 或 prerendered String
   - 替代方案：blog/doc loader 加文件级缓存，挪到 list_* 之前
- [ ] **list_blog_posts / list_*_articles mtime cache**（`crates/modules/blog/src/server.rs:29-72` + 板块 server.rs）
   - 通用 helper `crates/core/src/utils.rs::dir_listing_cache::<T>(dir, mtime_key, builder)`
   - `/sitemap.xml` `/feed.xml` 受益最大
- [ ] **sitemap / feed Cache-Control 头**（`crates/app/src/main.rs` ServeDir / axum route）
   - 加 `Cache-Control: public, max-age=3600`
- [ ] 性能 benchmark 脚本：`scripts/bench_hot_paths.sh` 跑 `/blog/welcome` × 100 / `/sitemap.xml` × 100，对比 cache 前后 P95

**完成定义**：navbar i18n + theme 不再每请求触发 wasmi；同篇博客 100 次访问只 parse 1 次 Markdown；sitemap 高并发不打满 CPU。

---

## Phase 8.6 — XSS allowlist + JWT role 重查（🟡 Warning）

> 来源：security agent。`sanitize_user_html` 可被 HTML 实体编码绕过；admin 降级后 JWT 仍 admin 一周。

- [x] **`sanitize_user_html` 改 allowlist**（`crates/widgets/src/sanitize.rs:178-194`）
   - 新增 `is_safe_link_url` / `is_safe_image_url`：先 `decode_html_entities` 解码 `&#x6A;` / `&#106;` / 命名实体，
     trim ASCII 空白 + 剥控制字符（TAB / LF / CR），再 lowercase 取 scheme 与 allowlist 比对
   - link allowlist：`http` / `https` / `mailto` / `tel`；image allowlist：上述 + 相对 path / `/` + `data:image/`
   - 渲染层（mdx.rs `Tag::Link` / `Tag::Image`）调用 helper；不合法 URL 丢链接保留文本 / 显示 `[image rejected]`
   - 删除 `neutralize_dangerous_urls` 字面串黑名单（已被 HTML entity 编码绕过）
   - 单测：`link_allowlist_resists_known_bypasses` 覆盖 `j&#x61;vascript:` / `&#106;avascript:` / `JaVaScRiPt:` / `j\tavascript:` / `javascript :` / `javascript&#58;`
- [x] **JWT role 在 require_admin 内重查 DB**（`crates/core/src/session.rs::require_admin`）
   - 改 async：先 verify_jwt 拿 user_id → `user::Entity::find_by_id` → `db_user.role != ROLE_ADMIN` 即拒
   - fail-closed：DB 不可达 / 用户已删 → 直接报错
   - 20 处 caller `require_admin()?` → `require_admin().await?` 批量替换
- [x] **`require_session()` helper**（`crates/core/src/session.rs`）
   - Phase 7.x 已存在；Phase 8.2 给 `/api/upload` 调用过；不重复
- [x] 文档：复用代码内 doc-comment，不再单独 sync `docs/AUTH_SPEC.md`（与现状一致）

**完成定义**：comment / topic 输入含 `j&#x61;vascript:` 链接不会变成可点击 XSS；admin 在数据库被降级后 5 秒内丧失后台权限。

---

## Phase 8.7 — Engine 层清理 + ModuleEngine 收口（🟠 设计债）

> 来源：architecture agent。Phase 1C.1 整套 `EngineRegistry` / lifecycle 抽象生产从未实例化（死代码）；ModuleEngine 半解决问题，navbar 仍硬编码 11 个 module id。

- [ ] **删 EngineRegistry / EngineContext / init / shutdown**（Design 默认 #5）
   - `crates/core/src/engines/mod.rs:97-173` 整段删除
   - 各 engine 文件改回简单 module + free function（保留有真实行为的 `PluginEngine`、`ThemeEngine`、`ModuleEngine`）
   - 测试相应调整
- [ ] **`LayoutEngine` 真正可用**（Design 默认 #5）
   - `LayoutPack` trait 加 `render(active_module_id: &str, children: Element) -> Element`
   - `ClassicShell` / `MinimalShell` 实现 trait
   - `crates/app/src/components/layouts/Navbar` 改为按 active layout dispatch 到 `LayoutPack::render`
   - 删 `core::engines::content::ComponentRegistry`（与 `widgets::registry` 重复）
- [ ] **`default_module_engine()` 加 OnceLock cache**（`crates/core/src/engines/module.rs:132-140`）
   - `OnceLock<RwLock<Arc<ModuleEngine>>>`
   - `admin_set_moderation_settings` 等改 site.json 的路径调 `invalidate()`
   - 单测：500 次 `default_module_engine()` 调用 → site.json 实际只读 1 次
- [ ] **navbar fallback 用 `default_module_specs().iter()` 替代硬编码**（`crates/app/src/components/layouts/classic.rs:39-67`）
   - 删两处硬编码 11 个 module id 的列表
   - `ModuleSpec` 加 `nav_label_key: &'static str`（i18n key）让 navbar 拿 label
- [ ] **sitemap 路径循环替代 if 链**（`crates/app/src/main.rs:300-389`）
   - `ModuleSpec` 加 `static_path: Option<&'static str>`（如 blog → `/blog`）
   - sitemap 闭包改 `for spec in enabled_modules() { if let Some(p) = spec.static_path { entries.push(...) } }`
- [ ] **文档**：`docs/MODULE_SPEC.md` 加 "如何加新模块" checklist + dependency direction 硬约束（modules 不能反向引用 core）
- [ ] 验收：手动加一个 mock 模块 → 只动 1 处 ModuleSpec 即可在 nav / sitemap / 路由全部出现

**完成定义**：核心 crate 无死代码；加 12th 模块不需要再 grep 改 4 处。

---

## Phase 8.8 — 杂项批（🟢 Defense-in-depth）

> 来源：3 个 agent 散落的小项。一次性收一下避免遗忘。

- [ ] **MathML CSP 友好**（`crates/widgets/src/mdx.rs:184,190`）
   - Pin `pulldown-latex` 版本；CSP 已在 8.3 加好
   - 加单测：MathML 输出不含 `<script>` / `javascript:`
- [ ] **PluginManager Mutex cold-miss 双竞**（`crates/core/src/lib.rs:61-83`）
   - 改用 `OnceCell` per-path 保证 single-flight 编译
- [ ] **`make_snippet` 不 to_lowercase 整 body**（`crates/modules/search/src/engine.rs:311-342`）
   - 用 Tantivy `SnippetGenerator` 改写（O(matches) vs O(body)）
   - 或者退一步：`body[..body.len().min(2000)].to_ascii_lowercase()`
- [ ] **`Box::leak(asset_path)` dev 重启泄漏**（`crates/app/src/main.rs:160-165`）
   - 换 `OnceLock<&'static str>`
- [ ] **`admin_moderation::reject_one` 业务删 silent failure**（`crates/modules/admin/src/server.rs:1037-1078`）
   - 4 个 `let _ = ...delete...` 改为 `?` 传播 + tracing::warn 兜底；外层包 transaction
- [ ] **Bulk reject 并发**（`crates/modules/admin/src/server.rs:1142-1168`）
   - `futures::stream::iter(rows).for_each_concurrent(8, ...)`
   - 或者更好：按 kind 单 SQL `DELETE WHERE id IN (...)` 一次
- [ ] **Hot-reload 后 admin_upload_plugin 缓存 prefilled**（`crates/modules/admin/src/server.rs:797-841` + `core/lib.rs`）
   - 上传成功后把 already-compiled `Module` 塞进 PluginManager cache，避免下次调用重编
- [ ] **`.bak` 文件 startup sweep**（同上 + `crates/app/src/main.rs` 启动期）
   - 启动时 prune `assets/plugins/*.bak` 超过 7 天的旧备份
- [ ] **插件 ABI 版本范围（前置 Phase 9 ABI v2）**（`crates/core/src/engines/plugin.rs` + `docs/PLUGIN_ABI.md`）
   - `PluginEngine` 维护 `accepted_abi_versions: Vec<u32>`（当前 `vec![1]`）
   - is_compatible 改为 `accepted.contains(&manifest.abi_version)`
   - 文档：升级策略写清
- [ ] **`main.rs` 跨平台注释清理**（Design 默认 #3）
   - 删 / 修 main.rs comment 中"保留 desktop / mobile 等跨平台能力"的措辞
   - 说明项目当前是 web-first via dioxus_fullstack

**Deferred 到 Phase 9**：
- Prometheus metrics / `/metrics` endpoint
- 全平台 desktop / mobile 迁移（含余下 16 处 `document::eval`）
- 插件 ABI v2（Extism / wit-bindgen）

---

## 进度跟踪

| Phase | 状态 | 关键交付 |
|---|---|---|
| 8.1 | ✅ Done | WASM 沙箱：fuel + memory + timeout + spawn_blocking |
| 8.2 | ✅ Done | Auth 表面：upload 鉴权 + placeholder 拒 + path 防穿越 + OsRng + drop access_token |
| 8.3 | ✅ Done | Pingora 网关：安全头 + XFF insert + rate limit |
| 8.4 | ✅ Done | DB tuning + comments 索引 + admin 并行 + SQL GROUP BY |
| 8.5 | ⏳ Pending | theme/i18n/Markdown/list_* mtime cache + Cache-Control |
| 8.6 | ✅ Done | sanitize_user_html allowlist + JWT role DB recheck |
| 8.7 | ⏳ Pending | EngineRegistry 删 + LayoutPack render + ModuleEngine 收口 + navbar 去硬编码 |
| 8.8 | ⏳ Pending | 杂项批：snippet / Box::leak / reject 并发 / .bak sweep / ABI 版本范围 |

---

## 验收标准（全 Phase 8 完成）

- **测试**：`cargo test --features server --workspace -- --test-threads=1` 全绿（预计 ~620+ tests）
- **clippy**：`cargo clippy --features server --workspace --all-targets -- -D warnings` 0 warning
- **性能**：blog/welcome 100 次 SSR P95 < 100ms（cache 命中态）；admin dashboard < 200ms；恶意插件无法 DoS host
- **安全**：浏览器 devtools 看 4 个安全头；未登录 `/api/upload` 401；admin 降级 5s 失效；模板 placeholder 启动 panic
- **文档**：每个 Phase 完成后同步本文件 `[ ]` → `[x]` + 一段实施说明
- **手动验证**：`cd crates/app && dx serve` + `cd crates/gateway && cargo build --release` 全过；docker compose up 全过

---

## 持续约定（继承自 Phase 1-7）

- 每完成一个独立功能立即 commit，不批量打包
- 提交信息走 Conventional Commits（`feat(8.1): ...` / `fix` / `refactor` 等）
- **不附 `Co-Authored-By`**（沿用 [Memory: workflow-conventions]）
- `cargo test` + `cargo clippy -- -D warnings` 通过才允许 commit
- 完成每个 sub-item 后立即把对应 `[ ]` 改为 `[x]` + 补简短实施说明（同 Phase 1-7 风格）
- `git push` 时机由用户决定，本计划只推进到本地 commit
- **Out of scope 守护**：本阶段不动 `modules/{ai,cli,embedded,wasm,web3}` 五板块合并、不做 ABI v2、不引 metrics crate

---

## 引用

- 上一阶段计划：[`Todos.phase1-7.md`](Todos.phase1-7.md)
- 触发本阶段的 review：聊天会话 2026-05-31 三 agent 并行扫描合成报告（未落盘）
- 涉及的核心 SPEC：[`docs/ENGINES_SPEC.md`](docs/ENGINES_SPEC.md) / [`docs/PLUGIN_ABI.md`](docs/PLUGIN_ABI.md) / [`docs/AUTH_SPEC.md`](docs/AUTH_SPEC.md) / [`docs/MODERATION_SPEC.md`](docs/MODERATION_SPEC.md) / [`docs/DEPLOY_GUIDE.md`](docs/DEPLOY_GUIDE.md)
