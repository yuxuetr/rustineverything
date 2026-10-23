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

日志按 `docker-compose.yml` 的 `x-logging` 轮转：每个容器最多 5 个 10MB 文件，更早的丢弃。要长期留存（如 `pay_audit` 审计日志），
另行转发到日志平台。

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
| `search: built initial index from disk + DB` / `search: loaded existing index` | info | 首次建索引 / 复用卷里的索引 |
| `search: index refreshed upserts=N deletes=M` | info | 启动时及每 10 分钟的增量刷新 |
| `search: index refresh failed` | warn | 增量刷新失败（下一轮重试） |
| `theme: unknown cookie theme, ignoring` | warn | 访客 cookie 不是内置主题 id（旧值或被改写），已回退默认 |
| `theme: unknown site.json theme, using default` | warn | site.json 的 `theme` 配错，已回退 `ocean` |
| `[AppError::Db] ...` | error | 数据库错误已转 `ServerFnError`，详情留服务端 |
| `comment: post_comment failed` | error | 用户提交失败，可能审核拒绝 |

> 设计原则：错误对外只暴露 `client_message`，详细信息走 tracing
> （`crates/core/src/error.rs` 的 `From<AppError> for ServerFnError`）。

## 2. 备份

`scripts/backup.sh` 每次写两份文件到 `BACKUP_DIR`（默认 `/data/rie/backups`），各保留最新 14 份（`KEEP`）：

| 文件 | 内容 |
| --- | --- |
| `db-<时间>.dump` | `pg_dump` 自定义格式，写完用 `pg_restore -l` 读一遍确认可用 |
| `uploads-<时间>.tar.gz` | app 容器的 `/app/assets/uploads`（`docker cp` 打包，顶层目录是 `uploads/`） |

任一步失败就以 1 退出，不留半成品，也不清理旧备份；按份数而不是按天数清理，连续失败几天旧备份也不会被删光。

```cron
MAILTO=you@example.com
30 3 * * * /data/rie/rustineverything/scripts/backup.sh >> /data/rie/backups/backup.log 2>&1
```

备份只在本机等于没备份：服务器整机出事（被入侵、磁盘坏）时它们一起没了，定期复制到另一台机器或对象存储。

### 2.1 恢复演练

```bash
scripts/restore_drill.sh                       # 最新一份
scripts/restore_drill.sh /data/rie/backups/db-20261023-033000.dump
```

把 dump 导入临时库 `rie_restore_drill`，逐表比对行数，再比对上传包与线上的文件数，最后删掉临时库，不碰线上库。
备份之后线上又有写入时会对不上，挑没人写的时段跑，或者先跑一次 `backup.sh` 再演练。
脚本本身的测试：`scripts/test_backup.sh`（需要 docker）。

### 2.2 恢复

先停 app（`docker compose stop app`），恢复后再启动：

```bash
# 数据库：变量在容器里展开（单引号）；宿主 shell / cron 不会加载 .env
docker exec -i rie-postgres \
  sh -c 'pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists --no-owner' \
  < /data/rie/backups/db-<时间>.dump

# 上传：卷名带 compose 项目名前缀（docker volume ls 可查）；写成 app-uploads 会新建一个空卷
docker run --rm -v rustineverything_app-uploads:/data -v /data/rie/backups:/in:ro alpine \
  tar xzf /in/uploads-<时间>.tar.gz -C /data --strip-components=1
```

### 2.3 site.json

跟随 git 仓库 / 镜像版本走，**不**需要独立备份。任何 `assets/`
变更必须经 git → CI → 重新 build 镜像，才能上线。例外：admin 在
`/admin/moderation/settings` 保存的审核配置直接写回运行中容器的 `site.json`，
容器重建后丢失，需要同步提交到仓库。

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
docker compose logs app | grep "search: index"

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

期待看到 `search: loaded existing index` 或 `search: built initial index from disk + DB`，之后每 10 分钟一条
`search: index refreshed`。一直是 `index refresh failed` 时看后面的 `error=`。

强制刷新索引：admin 会话下调用 `POST /api/search/reindex`（默认增量，`mode=full` 全量重建）。

### 5.5 主题不切换

```bash
docker compose logs app | grep -i theme
```

期待看到 `frontend: fetched theme CSS len=N`。主题是内置的（`crates/core/src/engines/theme.rs` 的 `THEMES`）：
- `site.json::theme` / `site_theme` cookie 不在 `THEMES` 中时回退到默认主题
- ThemePicker 下拉来自 `list_available_themes`，列的就是 `THEMES`

### 5.6 内容审核 LLM 不可用

审核走托管 LLM API（`OPENAI_LLM_*` / `ANTHROPIC_LLM_*`，可指向 OpenAI / DeepSeek /
… 或自托管 ollama 的 `/v1`）。若该 API 超时 / 限流 / 掉线，或回复读不出结论，
按 `site.json::moderation.on_llm_failure` 处理：默认 `review` 照常发布并送 `/admin/moderation`
人工复核，`reject` 拒绝提交（详 [`MODERATION_SPEC.md`](MODERATION_SPEC.md) §3.3）。

排查：
```bash
docker compose logs app | grep "LLM review failed"   # cause = 原因，failures = 累计失败次数
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

迁移只在启动时执行 `up`；运行时镜像只有服务端二进制，没有 `sea-orm-cli`，`migration` crate 也没有命令行入口。
所以不支持单条 `down`：新版本含迁移且需要回滚时，按 6.3 用升级前的备份恢复数据库，再切回旧代码。
升级前先跑一次 `scripts/backup.sh`。

### 6.3 完整回滚（含数据）

```bash
# 1. 停服
docker compose down

# 2. 只起数据库，用升级前的备份按 2.2 覆盖恢复
docker compose up -d postgres
docker exec -i rie-postgres \
  sh -c 'pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists --no-owner' \
  < /data/rie/backups/db-<升级前的时间>.dump

# 3. 切回旧代码
git checkout <previous-sha>
docker compose build app
docker compose up -d
```

## 7. 性能调优

### 7.1 postgres 连接池

`crates/core/src/db/pool.rs::init_pool`：默认最多 32、最少 2 个连接，连接 / 获取超时 5s，空闲 600s。
用环境变量调：`DB_MAX_CONN`、`DB_MIN_CONN`、`DB_CONNECT_TIMEOUT_SECS`、`DB_ACQUIRE_TIMEOUT_SECS`、`DB_IDLE_TIMEOUT_SECS`。

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

## 8. 安全运维

| 任务 | 频率 |
| --- | --- |
| `JWT_SECRET` 轮换 | 季度（旋转时所有用户被踢下线） |
| postgres 密码轮换 | 季度 |
| `docker compose pull` upstream 镜像 | 月度（postgres 安全补丁） |
| 备份恢复演练（`scripts/restore_drill.sh`） | 季度 |
| OAuth 凭据轮换 | 按 provider 推荐 |
| 审查 `seaql_migrations` 与 git 是否一致 | 每次发布 |

## 9. 参考

- [DEPLOY_GUIDE.md](DEPLOY_GUIDE.md) — 从零部署
- [docs/](.) — 各模块 SPEC
- [Todos.md](../Todos.md) — Roadmap + 各阶段验收门禁
- [.github/workflows/ci.yml](../.github/workflows/ci.yml) — CI 配置（与构建路径同构）
