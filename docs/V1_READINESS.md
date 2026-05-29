# v1 上线就绪清单（v1 Readiness Checklist）

> 截至 2026-05-29。汇总当前能力、验收状态与上线前待办。
> 图例：✅ 已完成并测试 · 🟡 已实现但需真实环境验证 · ⏳ 未做（v1 可选）

## 1. 核心平台

- ✅ Dioxus 0.7 全栈（SSR + hydration）+ Axum 自定义路由
- ✅ SeaORM + PostgreSQL，启动期 `sea-orm-migration` 自动迁移（8 张表：initial_schema 7 + moderation_queue 1）
- ✅ 全局 DB 连接池单例（`init_pool` / `get_or_init_pool`）
- ✅ 8 引擎架构（plugin/module/auth/theme/layout/content/moderation/search）
- ✅ 测试：`cargo test --features server --workspace -- --test-threads=1` → **558 passed / 0 failed / 18 ignored**（ignored 为 live-LLM + live-DB 集成测试，分别需 API key / `DATABASE_URL`）

## 2. 安全（Phase 1A）

- ✅ `JWT_SECRET` / `BASE_URL` 缺失时启动即 panic（无不安全 fallback）
- ✅ OAuth `state` CSRF 校验（5 分钟 TTL）+ PKCE store TTL
- ✅ 生产 Cookie `Secure` 标志（按 `BASE_URL` 是否 https）
- ✅ 图片上传 MIME 嗅探白名单 + 5MB 上限 + 安全文件名
- ✅ `user_identities.access_token` AES-GCM 加密存表
- ✅ 用户 Markdown XSS 防护（`sanitize_user_html`，评论/论坛 5 处开启）
- ✅ 用户创建事务化（user + identity 同事务）+ 回滚回归测试（`#[ignore]` live-DB）

## 3. 内容板块（Phase 6）

- ✅ 11 个内置模块，`site.json::modules.<id>.enabled` 一键开关（nav/路由 gate/sitemap/feed 一致）
- ✅ 5 个新内容板块：embedded / ai / web3 / wasm / cli（各独立 crate，≥15 单测，含真实长文）
- ✅ 板块文章接入 Tantivy 全文搜索（`collect_boards()` 索引 `assets/topics/<board>/`，kind=板块 id，受 `site.json` 模块开关 gate）
- ✅ 每板块 ≥3 真实案例（cases 联动）：15 个真实 Rust 项目 case（embedded/ai/web3/cli 各 3 + wasm 标签 ≥3），按 `category`/`tag` 归类，`/cases` 可按板块筛选

## 4. 内容审核（Phase 4）

- ✅ 统一 ModerationPipeline，默认 `enabled=false` → 零开销 Allow
- ✅ 5 条提交路径全部接入（评论/话题/回复/标注/上传）
- ✅ 两层链接检测（host 黑名单 sync stage + 插件 prompt）
- ✅ 阈值 schema 校验（范围/NaN/block≥flag，装载时校验回退）
- ✅ Admin 复核页：Tab 过滤 + 单条/批量 approve/reject + 作者历史违规徽章
- ✅ 多模态视觉审核（已对 gpt-4o-mini 实测）
- 🟡 审核 P95 ≤ 1.5s — 取决于所选 LLM provider，需真实部署压测

## 5. 插件生态（Phase 5）

- ✅ wasmi 插件运行时 + ABI 版本协商 + 输出大小上限（8MB）
- ✅ Hot reload：admin 上传 wasm（沙箱校验 + ABI 校验 + 备份 + 原子替换 + 回滚）→ 失效缓存 / 重建审核流水线，无需重启
- ✅ 内存回收：invalidate 即 Drop 旧 Module（单测验证缓存恒为 1）
- ✅ `/plugins` 公开浏览页：扫 `assets/plugins/*.wasm` 读 manifest 展示（已浏览器实测 9 插件，0 console error）
- ⏳ 插件市场 `registry.json` 已审核清单 + 提交流程文档（开源后做）

## 6. SEO / 内容分发

- ✅ `inject_seo`（og/twitter/canonical/JSON-LD）+ `/sitemap.xml` + `/feed.xml` + `/robots.txt`
- ✅ sitemap/feed 按模块开关收录（含 5 板块文章；feed 全站日期降序取 50）
- 🟡 Lighthouse SEO ≥ 95 — 需上线后跨 hostname 实测

## 7. 部署 / CI / 运维（Phase 7）

- ✅ 多阶段 Debian (trixie/glibc) Dockerfile + `docker-compose.yml`（app + postgres；审核走托管 LLM API，无 ollama/GPU 依赖）
- ✅ CI：fmt + clippy **强校验**（clippy `-D warnings` 零告警）+ test + build + wasm 插件构建
- ✅ tracing 日志（`RUST_LOG`），全工作区无 `println!` 调试输出
- ✅ runbook：`DEPLOY_GUIDE.md`（部署）+ `OPERATIONS.md`（day-2 运维，含 hot reload §2.4）
- ✅ `docker compose up` 一键起 + 自动迁移 — 2026-05-29 干净环境实跑通过（Debian trixie 镜像；postgres healthy → app 启动 → 8 表迁移干净应用 → `curl :8080` 200）
- ⏳ 7.2 PKCE 持久化（加密 cookie）/ 7.3 搜索 `MmapDirectory` 持久化 — v1 可选

## 8. 上线前必做（go-live）

1. 准备 `.env`：`JWT_SECRET`（强随机）、`BASE_URL`（https 公网域名）、`DATABASE_URL`、OAuth 凭据。
2. 在干净环境跑一次 `docker compose up -d` 验证迁移 + 烟测端点（见 DEPLOY_GUIDE）。
3. 首个管理员：`scripts/promote_admin.sh <昵称>` 后重新登录。
4. 如启用审核：配置 `OPENAI_LLM_*` / `ANTHROPIC_LLM_*` + `site.json::moderation`，压测 P95。
5. 配 HTTPS 反代（Caddy/nginx/Traefik，见 DEPLOY_GUIDE）。
6. 备份策略：postgres `pg_dump` + `assets/uploads/`（见 OPERATIONS §2）。

## 9. 已知限制（v1 可接受）

- 审核阈值在线图形编辑器未做（改 site.json + 「重新载入」生效）。
- cases 案例的 star 数统一为 0（不抓取实时 GitHub 数据，避免展示陈旧/虚假计数）。
- 评论硬删除，无软删除 / 操作审计。
- hot reload 写入运行中容器的 `assets/plugins/`，容器重建会回到镜像版本（除非挂卷）。
