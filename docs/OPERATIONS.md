# Operations Runbook

> 适用阶段：Phase 7.6（v2.1 Todos.md）。
> 部署完后的日常运维手册：日志 / 备份 / 监控 / 故障排查 / 回滚。
> 初次部署见 [DEPLOY_GUIDE.md](DEPLOY_GUIDE.md)。

## 1. 日志

### 1.1 实时跟随

```bash
docker compose logs -f app           # app 服务
docker compose logs -f postgres      # 数据库
docker compose logs -f               # 全部
```

### 1.2 过滤级别

App 用 [`tracing`](https://docs.rs/tracing) + [`tracing-subscriber`](https://docs.rs/tracing-subscriber)
（Phase 7.5）。通过 `RUST_LOG` 控制粒度：

```dotenv
# 调试单个模块
RUST_LOG=info,app_core::auth=debug

# 静音吵闹的 sqlx 心跳
RUST_LOG=info,sqlx=warn

# 全 debug（仅短时排错）
RUST_LOG=debug
```

改完 `.env` 后 `docker compose up -d app` 重启即可生效。

### 1.3 关键日志事件

| 事件 | 级别 | 含义 |
| --- | --- | --- |
| `startup: DB pool initialized` | info | postgres 连接 OK |
| `startup: schema migrations applied` | info | sea-orm-migration 成功 |
| `startup: migration failed` | error | 迁移失败 — 检查 schema 漂移 |
| `auth: token exchange success provider=X` | info | OAuth 登录闭环 |
| `auth: PKCE code_verifier matched` | debug | PKCE 校验通过 |
| `auth: site.json::auth.enabled=false` | warn | 登录被禁用（预期还是误配？） |
| `search: index rebuilt documents=N` | info | tantivy 重建完成 |
| `theme: skipping plugin ...` | warn | 主题插件加载失败，不阻塞 |
| `[AppError::Db] ...` | error | 数据库错误已转 `ServerFnError`，详情留服务端 |
| `comment: post_comment failed` | error | 用户提交失败，可能审核拒绝 |

> 设计原则：错误对外只暴露 `client_message`，详细信息走 tracing
> （`crates/core/src/error.rs` 的 `From<AppError> for ServerFnError`）。

## 2. 备份

### 2.1 数据库

```bash
# 完整导出（约 2-50 MB，含 schema + 数据）
docker compose exec postgres \
  pg_dump -U $POSTGRES_USER -F c $POSTGRES_DB \
  > backup-$(date +%F-%H%M).dump

# 还原
cat backup-2026-05-27-1430.dump | \
  docker compose exec -T postgres \
  pg_restore -U $POSTGRES_USER -d $POSTGRES_DB --clean --if-exists
```

建议 cron：

```cron
0 3 * * * cd /srv/rustineverything && docker compose exec -T postgres \
  pg_dump -U $POSTGRES_USER -F c $POSTGRES_DB \
  > /backups/rie-$(date +\%F).dump
```

### 2.2 用户上传

`app-uploads` 卷只在 docker 节点本地。异地备份：

```bash
docker run --rm -v app-uploads:/data -v $(pwd):/out \
  alpine tar czf /out/uploads-$(date +%F).tgz -C /data .
```

恢复：

```bash
docker run --rm -v app-uploads:/data -v $(pwd):/in \
  alpine sh -c 'cd /data && tar xzf /in/uploads-2026-05-27.tgz'
```

### 2.3 site.json + plugins/

这两个跟随 git 仓库 / 镜像版本走，**不**需要独立备份。任何 `assets/`
变更必须经 git → CI → 重新 build 镜像，才能上线。

### 2.4 插件热更新（Hot Reload, Phase 5.1）

admin 可在 `/admin/plugins` 直接上传 `.wasm`，无需重启进程：

- **校验**：上传字节先在临时 wasmi Store 上编译 + 实例化，校验 `memory` /
  `alloc` / `dealloc` 导出，并读 `get_manifest` 比对 ABI 版本。不兼容 / 非法
  wasm 直接拒绝，文件不落盘。
- **原子替换 + 回滚**：旧文件先复制为 `<name>.bak`，新字节写 `<name>.tmp` 后
  `rename` 原子替换；任一 IO 步骤失败自动从 `.bak` 恢复。
- **生效**：替换后失效 `PluginManager` 缓存（主题 / i18n / auth 下次调用按
  mtime 重新加载）；审核类插件额外触发 `reload_pipeline()` 重建审核流水线。
- 「重新载入」按钮 = `admin_reload_plugins`：清空全部插件缓存 + 重建审核流水线
  （改完 `site.json::moderation` 阈值 / 插件列表后点一下即可生效）。

> ⚠️ **持久化警告**：hot reload 写入的是**运行中容器**的 `assets/plugins/`。
> 容器重建（`docker compose up --force-recreate` / 滚动发布）会回到镜像内的
> 版本。要永久生效，仍需把 `.wasm` 提交进 git → CI → 重 build 镜像（见 2.3）。
> 若希望热更新持久，可把 `assets/plugins/` 挂为命名卷。

**内存回收监测**：每次 reload 旧 `wasmi::Module` 句柄从缓存 HashMap 移除即
Drop（单测 `test_reload_evicts_old_module_cache_stays_bounded` 验证缓存恒为
单条不累积）。生产环境若担心长跑泄漏，连续上传同一插件后观察 RSS：

```bash
# 反复 reload 时跟随容器内存（应趋于平稳，不持续爬升）
watch -n 5 'docker compose exec app sh -c "cat /proc/1/status | grep VmRSS"'
```

## 3. 数据库迁移

### 3.1 自动应用

每次 `docker compose up -d app` 启动时，`crates/migration` 会调
`Migrator::up(&db, None)`，幂等。已应用的迁移记录在 `seaql_migrations` 表。

```bash
# 查询当前已应用迁移
docker compose exec postgres psql -U $POSTGRES_USER -d $POSTGRES_DB \
  -c "SELECT * FROM seaql_migrations ORDER BY version;"
```

### 3.2 手动控制

通过 sea-orm-cli（容器内或本地，需 `DATABASE_URL`）：

```bash
# 看下一步要跑的迁移
sea-orm-cli migrate status

# 回滚最近一条
sea-orm-cli migrate down -n 1

# 跑到指定版本
sea-orm-cli migrate up -n 1
```

### 3.3 编写新迁移

```bash
# 模板（手工 + 参考 m20260527_000001_initial_schema.rs）
crates/migration/src/m<YYYYMMDD>_<NNNNNN>_<slug>.rs
```

骨架：

```rust
use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;
impl MigrationName for Migration {
  fn name(&self) -> &str { "m20260612_000002_add_moderation_log" }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> { /* ... */ Ok(()) }
  async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> { /* ... */ Ok(()) }
}
```

记得在 `lib.rs::Migrator::migrations()` 里追加 `Box::new(...)`。
再加一行单测进 `tests::migrations_have_expected_names`。

## 4. 监控指标

当前未集成 Prometheus / Grafana（Phase 7 后续）。可临时通过：

```bash
# 容器实时资源
docker stats --no-stream

# postgres 连接数
docker compose exec postgres psql -U $POSTGRES_USER -d $POSTGRES_DB \
  -c "SELECT count(*) FROM pg_stat_activity WHERE datname='$POSTGRES_DB';"

# tantivy 索引规模（启动日志）
docker compose logs app | grep "index rebuilt"

# 评论 / 话题量
docker compose exec postgres psql -U $POSTGRES_USER -d $POSTGRES_DB \
  -c "SELECT
       (SELECT count(*) FROM comments) AS comments,
       (SELECT count(*) FROM topics)   AS topics,
       (SELECT count(*) FROM users)    AS users;"
```

## 5. 故障排查

### 5.1 app 启动失败 `panic: JWT_SECRET 未配置`

`.env` 缺少 `JWT_SECRET`。修复后 `docker compose up -d app`。

### 5.2 OAuth 回调 `Error: 401 unauthorized`

可能原因：
1. **`BASE_URL` 与 OAuth App 注册的 redirect_uri 不一致** — 最常见
2. **state 校验失败**：用户多个标签页同时点登录，state 在另一个 tab 被消费。让用户重新点一次登录入口
3. **PKCE 过期**：5 分钟 TTL；进程重启会全部失效

排查：

```bash
docker compose logs app | grep -i "auth"
```

应该看到 `auth: token exchange success` 或 `auth: failed to ...`。

### 5.3 评论 / 话题 500 错误

```bash
docker compose logs app | grep -E "AppError::Db|post_comment"
```

常见：
- postgres 容器挂了：`docker compose ps` 看 `postgres` 是否 `healthy`
- 迁移漂移（旧 schema + 新代码）：拿 `seaql_migrations` 表对照 `crates/migration` 中的列表

### 5.4 搜索没结果

```bash
docker compose logs app | grep "search:"
```

期待看到 `index rebuilt with N documents`。N=0 通常意味着 `assets/posts/`
里没文章，或文章 frontmatter 解析失败。

强制刷新索引：admin 登录后访问 `/admin/plugins`（同时也会触发搜索 reload）。

### 5.5 主题不切换

```bash
docker compose logs app | grep -i theme
```

期待看到 `frontend: fetched theme CSS len=N`。如果 N=0 / `failed to fetch theme`：
- `assets/plugins/<theme>.wasm` 是否存在
- ThemePicker 下拉用 `list_available_themes` server fn 扫 manifest；检查 wasm `get_manifest` capability 是否含 `theme`

### 5.6 内容审核 LLM 不可用

审核走托管 LLM API（`OPENAI_LLM_*` / `ANTHROPIC_LLM_*`，可指向 OpenAI / DeepSeek /
… 或自托管 ollama 的 `/v1`）。若该 API 超时 / 限流 / 掉线，pipeline **fail-open**：
当前 stage 记 warning 并放行，不阻塞用户提交（详 [`MODERATION_SPEC.md`](MODERATION_SPEC.md)）。

排查：
```bash
docker compose logs app | grep -i moderation   # 看 fail-open / 调用失败日志
# 直接探活所配置的 LLM 端点（示例）：
curl -sS "$OPENAI_LLM_BASE_URL/v1/models" -H "Authorization: Bearer $OPENAI_LLM_API_KEY" | head
```
默认 `site.json::moderation.enabled=false` 时审核关闭，本节不适用。

## 6. Rollback

### 6.1 应用回滚（代码）

```bash
git log --oneline -10
git checkout <previous-sha>
docker compose build app
docker compose up -d app
```

> ⚠️ 如果新版本含 schema 迁移，先评估是否需要回滚迁移：

### 6.2 schema 回滚

```bash
# 回退一条迁移（执行最近一条的 down() 函数）
docker compose run --rm app \
  sh -c 'cd /workspace && sea-orm-cli migrate down -n 1'
```

如果迁移有数据破坏性变更（drop column / change type），先恢复备份后再回滚代码。

### 6.3 完整回滚（含数据）

```bash
# 1. 停服
docker compose down

# 2. 恢复 postgres 卷
docker run --rm -v rustineverything_postgres-data:/var/lib/postgresql/data \
  -v $(pwd):/backup alpine sh -c \
  'rm -rf /var/lib/postgresql/data/* && tar xzf /backup/postgres-2026-05-27.tgz -C /var/lib/postgresql/data'

# 3. 切回旧代码
git checkout <previous-sha>
docker compose build app
docker compose up -d
```

## 7. 性能调优

### 7.1 postgres 连接池

`crates/core/src/db/pool.rs::init_pool` 用 SeaORM 默认配置。要调
（默认 max 100, min 1）需要修改源码（计划中：通过 env 变量配置）。

排查连接耗尽：

```sql
SELECT count(*), state FROM pg_stat_activity GROUP BY state;
```

### 7.2 镜像层缓存

Dockerfile builder 阶段把「依赖编译」与「源码编译」分层。修改源码不变 deps
时 build 仅 ~30s；改 `Cargo.toml` 则触发依赖重编（5-10 分钟）。

CI 用 `Swatinem/rust-cache@v2` 节省 PR 编译时间。本地用 `sccache` 进一步加速：

```bash
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### 7.3 wasm 插件冷启动

每次 server fn 调用都会 `wasmi::Module::new`。Phase 1A.3 已加 mtime 缓存
（`crates/core/src/lib.rs::PluginManager::cache`），同一 wasm 文件后续调
用 ~µs 级。在 admin `/admin/plugins` 刷新会 invalidate 全部缓存。

## 8. 安全运维

| 任务 | 频率 |
| --- | --- |
| `JWT_SECRET` 轮换 | 季度（旋转时所有用户被踢下线） |
| postgres 密码轮换 | 季度 |
| `docker compose pull` upstream 镜像 | 月度（postgres 安全补丁） |
| 备份恢复演练 | 季度（确认 dump 可用） |
| OAuth 凭据轮换 | 按 provider 推荐 |
| 审查 `seaql_migrations` 与 git 是否一致 | 每次发布 |

## 9. 参考

- [DEPLOY_GUIDE.md](DEPLOY_GUIDE.md) — 从零部署
- [docs/](.) — 各模块 SPEC
- [Todos.md](../Todos.md) — Roadmap + 各阶段验收门禁
- [.github/workflows/ci.yml](../.github/workflows/ci.yml) — CI 配置（与构建路径同构）
