# Deploy Guide

> 适用阶段：Phase 7.1 + 7.4 + 7.5 + 7.6（v2.1 Todos.md）。
> 本文是站点的「从零部署」runbook：从克隆仓库到第一次 `docker compose up`，
> 再到生产环境上 https。Day-2 操作（日志 / 备份 / 回滚 / 常见故障）见
> [OPERATIONS.md](OPERATIONS.md)。

## 1. 部署方式概览

| 方式 | 适用场景 | 路径 |
| --- | --- | --- |
| `docker compose` (推荐) | 一键起整站（app + postgres），生产小规模 / staging / 自测 | §3 |
| 单 Docker 镜像 | 已有 postgres / 数据库托管，仅跑 app | §4 |
| 裸机 `dx serve` | 本地开发（含热重载） | [README.md](../README.md) |

镜像 runtime 基于 `debian:trixie-slim`（glibc）+ 非 root 用户 `app`，绑定 `0.0.0.0:8080`。

## 2. 前置条件

- Docker 24+（含 buildx 与 compose v2）
- ≥ 1 CPU / 1 GB RAM / 5 GB 磁盘（app + postgres；内容审核走托管 LLM API，无需本地 GPU/模型）
- 一组 OAuth 凭据（GitHub / Google / Discord / Twitter）— **可选**，不配置时登录页自动隐藏对应入口
- HTTPS 反向代理（线上）：Caddy / nginx / Traefik 任意，详见 §6

## 3. docker-compose 一键部署

### 3.1 准备 .env

```bash
git clone https://github.com/<owner>/rustineverything.app.git
cd rustineverything.app
cp .env.example .env
$EDITOR .env
```

最少必须填的两项：

```dotenv
JWT_SECRET=<32+ 字符随机串；用 openssl rand -hex 32 生成>
BASE_URL=https://example.com         # 生产；本地可用 http://127.0.0.1:8080
```

可选项：postgres 密码、OAuth 凭据、`RUST_LOG`。完整变量见 [`.env.example`](../.env.example)。

### 3.2 启动

```bash
docker compose up -d        # 后台拉镜像 + 启动
docker compose logs -f app  # 跟随 app 启动日志
```

预期日志（顺序）：
1. `startup: DB pool initialized`
2. `startup: schema migrations applied` ← sea-orm-migration 自动跑
3. `[Server] Listening on 0.0.0.0:8080`

如果看到 `JWT_SECRET 未配置` 或 `BASE_URL 未配置`，回 §3.1 检查 `.env`。

### 3.3 首次烟测

```bash
curl -fsS http://127.0.0.1:8080/sitemap.xml | head -5
curl -fsS http://127.0.0.1:8080/feed.xml    | head -5
curl -fsS http://127.0.0.1:8080/robots.txt
```

三个端点都应返回 200。打开浏览器访问 `BASE_URL`，主导航 + 主题切换应该工作。

### 3.4 创建第一个 admin

新注册的用户 `role` 默认为 `member`，需要手动升级：

```bash
docker compose exec postgres psql -U $POSTGRES_USER -d $POSTGRES_DB \
  -c "UPDATE users SET role='admin' WHERE nickname='<your-nickname>';"
```

或使用项目脚本（如 postgres 在容器内）：

```bash
./scripts/promote_admin.sh <your-nickname>
```

`role=admin` 的用户登录后顶部菜单会出现「管理后台」入口。

## 4. 单 Docker 镜像（不带 compose）

适用于已有托管 postgres / RDS / Supabase 的场景。

```bash
docker build -t rustineverything:latest .

docker run -d --name rie-app \
  -p 8080:8080 \
  -e DATABASE_URL='postgres://<user>:<pass>@<host>:5432/<db>' \
  -e JWT_SECRET='<32+ 字符随机>' \
  -e BASE_URL='https://example.com' \
  -e GITHUB_CLIENT_ID='...' \
  -e GITHUB_CLIENT_SECRET='...' \
  -v rie-uploads:/app/assets/uploads \
  rustineverything:latest
```

挂卷 `/app/assets/uploads` 让用户图片在容器重启 / 滚动升级时不丢。

## 5. OAuth 凭据申请

| Provider | 申请页 | redirect_uri |
| --- | --- | --- |
| GitHub | `https://github.com/settings/developers` → New OAuth App | `<BASE_URL>/api/auth/callback/github` |
| Google | `https://console.cloud.google.com/apis/credentials` → OAuth 2.0 Client | `<BASE_URL>/api/auth/callback/google` |
| Discord | `https://discord.com/developers/applications` → New Application → OAuth2 | `<BASE_URL>/api/auth/callback/discord` |
| Twitter | `https://developer.x.com/en/portal` → User authentication settings | `<BASE_URL>/api/auth/callback/twitter` |

详细 scope / 注意事项见 [`AUTH_GUIDE.md`](AUTH_GUIDE.md)。

> ⚠️ `BASE_URL` 必须与每个 OAuth App 注册时填的回调 host 一致；任何不匹配
> 都会被 provider 拒绝。生产环境务必使用 https。

## 6. HTTPS / 反向代理

app 自己不终止 TLS。生产部署把 8080 端口放在反向代理后面。

### 6.1 Caddy（最简）

```Caddyfile
example.com {
  encode zstd gzip
  reverse_proxy 127.0.0.1:8080
}
```

Caddy 自动申请 + 续期 Let's Encrypt 证书。

### 6.2 nginx

```nginx
server {
  listen 443 ssl http2;
  server_name example.com;

  ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

  location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;

    # 上传大图：默认 1MB 太小（Phase 1A.4 上限 5MB）
    client_max_body_size 5M;
  }
}
```

### 6.3 Traefik 标签（compose 集成）

如果用 Traefik，在 `docker-compose.yml::app.labels` 加：

```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.routers.rie.rule=Host(`example.com`)"
  - "traefik.http.routers.rie.tls.certresolver=letsencrypt"
  - "traefik.http.services.rie.loadbalancer.server.port=8080"
```

## 7. site.json 配置

`assets/site.json` 控制站点形态（站点名 / 主题 / 模块开关 / 布局）。
完整字段见 [`THEME_SPEC.md`](THEME_SPEC.md) + [`MODULE_SPEC.md`](MODULE_SPEC.md)。

部署时常见调整：

```jsonc
{
  "site_name": "你的站点名",
  "site_description": "副标题",
  "themes": ["theme_sunset_plugin.wasm"],   // 主题栈
  "active_layout": "classic",                // 或 "minimal"
  "modules": {
    "forum": { "enabled": true },
    "podcast": { "enabled": false }          // 关闭某模块
  },
  "auth": {
    "enabled": true,
    "providers": [
      { "id": "github", "plugin": "github_auth_plugin.wasm" }
    ]
  }
}
```

修改后只需重启 app 容器，无需重新 build 镜像：

```bash
docker compose restart app
```

## 8. 内容资产

`assets/` 包含运行时数据：

| 子目录 | 用途 | 是否随镜像 |
| --- | --- | --- |
| `posts/` | 博客 MDX | ✅ 镜像内 |
| `docs/` | 文档 MDX | ✅ |
| `courses/` | 课程多媒体 | ✅ |
| `podcasts/` | 播客音频元数据 | ✅ |
| `cases/` | 案例展示 | ✅ |
| `plugins/` | WASM 插件 | ✅ |
| `audio/` | 大体积音频 | ✅（10MB 以下；超过被 build.rs 跳过） |
| `uploads/` | **用户上传** | ❌ 走持久卷 |

新内容上线流程：
1. 在本地把 MDX 放到 `assets/posts/<slug>.md`
2. `git commit` + push
3. 在服务器上 `git pull && docker compose build app && docker compose up -d app`

## 9. 升级流程

```bash
# 0. 备份数据库（强烈建议；详见 OPERATIONS.md §备份）
docker compose exec postgres pg_dump -U $POSTGRES_USER $POSTGRES_DB > backup-$(date +%F).sql

# 1. 拉新代码
git pull

# 2. 重新构建镜像（增量；deps 不变时 builder 阶段大半走缓存）
docker compose build app

# 3. 滚动重启（postgres 容器不动；仅 app）
docker compose up -d app

# 4. 跟日志确认 migrations 成功
docker compose logs -f app | grep "schema migrations"
```

回滚见 [OPERATIONS.md](OPERATIONS.md#rollback)。

## 10. 安全 checklist（上线前）

| 项目 | 检查 |
| --- | --- |
| `JWT_SECRET` | 32+ 字符随机串，**不与任何 git 历史共享** |
| `BASE_URL` | https；与 OAuth callback 一致 |
| postgres 密码 | 强密码；不暴露 5432 端口到公网（compose 默认仅 127.0.0.1） |
| Cookie Secure | `BASE_URL` 以 `https://` 起首时自动启用（main.rs cookie_is_secure） |
| 反向代理 | TLS 终止 + `X-Forwarded-Proto: https` |
| `RUST_LOG=info` | 而非 `debug`，避免敏感请求参数进日志 |
| Admin 升级 | 创建第一个 admin 后**移除** `users` 表的开放写权限脚本 |
| 容器更新 | 定期 `docker compose pull` 后台镜像（postgres） |

## 11. 已知限制

- **uploads/ 不集中备份**：当前用户图片只在 `app-uploads` 卷里；若需异地备份，挂 NFS / S3-FUSE 或自行 `docker cp` 定时拷出。
- **搜索索引 RAMDirectory**：每次重启重建全量索引。Phase 7.3 将切到 MmapDirectory 增量持久化。
- **PKCE store 进程内**：app 容器重启会丢失正在进行的 OAuth state，未完成登录的用户需要重新点登录。Phase 7.2 计划改为加密 cookie 替代。
- **moderation provider**：当前 ModerationEngine 为骨架，未接入实际 LLM。Phase 4.3+ 落地。

## 12. 参考

- [CI workflow](.github/workflows/ci.yml) — 跟生产构建路径几乎对齐
- [OPERATIONS.md](OPERATIONS.md) — 日常运维 / 故障排查
- [AUTH_GUIDE.md](AUTH_GUIDE.md) — OAuth 详细配置
- [ENGINES_SPEC.md](ENGINES_SPEC.md) — 引擎层架构
