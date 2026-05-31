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
- HTTPS 反向代理（线上）：**Pingora**（推荐 — Rust 原生）/ Caddy / nginx / Traefik 任选，详见 §6

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

> **选型原则**：本项目全栈 Rust，反代默认推荐 [**Pingora**](https://github.com/cloudflare/pingora)（Cloudflare 开源，纯 Rust，多线程异步，承载 Cloudflare 边缘）。同栈技术降低运维认知负担、便于内部贡献者上手。Caddy / nginx / Traefik 仍可用，作为不愿引入 Rust 工具链时的备选（§6.2–§6.4）。

### 6.1 Pingora（推荐 — Rust 原生）

仓库已自带可用的 gateway 实现：**`crates/gateway/`**（独立 workspace，自带
`[workspace]` 标签 + `exclude` 进父 workspace，避免 openssl/native deps 拖累
主构建）。直接 `cd crates/gateway && cargo build --release` 即可拿到
`target/release/rie-gateway` 二进制；下面的 Cargo.toml + main.rs 是同一份代码
的展开说明，自行从零搭起也照此即可。

#### Cargo.toml

```toml
[package]
name = "rie-gateway"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
pingora = { version = "0.8", features = ["lb"] }
pingora-proxy = "0.8"
pingora-core = "0.8"
pingora-http = "0.8"
log = "0.4"
env_logger = "0.11"
```

#### src/main.rs

```rust
use async_trait::async_trait;
use pingora::prelude::*;
use pingora_core::server::Server;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{http_proxy_service, ProxyHttp, Session};

/// 反代上游 = 本机 app 容器；HTTP/1.1 + keep-alive。
const UPSTREAM_ADDR: &str = "127.0.0.1:8080";
const UPSTREAM_SNI: &str = "";   // 上游是 plain HTTP, 不需要 SNI

struct AppGateway;

#[async_trait]
impl ProxyHttp for AppGateway {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  /// 选定上游：恒为 app 容器（单 upstream，不做 LB）。
  async fn upstream_peer(
    &self,
    _session: &mut Session,
    _ctx: &mut (),
  ) -> Result<Box<HttpPeer>> {
    // tls=false → 与上游走明文（同主机回环）；SNI 留空
    Ok(Box::new(HttpPeer::new(UPSTREAM_ADDR, false, UPSTREAM_SNI.into())))
  }

  /// 上游请求 header 改写：补 X-Forwarded-* 以便 app cookie 决定 `Secure` 标志、日志取真实 IP。
  async fn upstream_request_filter(
    &self,
    session: &mut Session,
    req: &mut RequestHeader,
    _ctx: &mut (),
  ) -> Result<()> {
    req.insert_header("X-Forwarded-Proto", "https").ok();
    if let Some(addr) = session.client_addr() {
      let ip = addr.to_string();
      req.insert_header("X-Real-IP", ip.clone()).ok();
      // append 而不是 insert：保留上游可能已有的 chain
      req.append_header("X-Forwarded-For", ip).ok();
    }
    Ok(())
  }
}

/// 80 端口专用：所有请求 301 到 https://同一 host/同一 path。
struct HttpToHttps;

#[async_trait]
impl ProxyHttp for HttpToHttps {
  type CTX = ();
  fn new_ctx(&self) -> Self::CTX {}

  async fn request_filter(&self, session: &mut Session, _ctx: &mut ()) -> Result<bool> {
    let req = session.req_header();
    let host = req
      .headers
      .get("host")
      .and_then(|v| v.to_str().ok())
      .unwrap_or("");
    let path = req.uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let location = format!("https://{}{}", host, path);

    let mut resp = ResponseHeader::build(301, None).unwrap();
    resp.append_header("Location", location).ok();
    resp.append_header("Content-Length", "0").ok();
    // end_of_stream = true：301 无 body，回写头即结束
    session.write_response_header_ref(&resp, true).await.ok();
    Ok(true) // short-circuit, 不再去上游
  }

  // upstream_peer 永远不会被调用，但 trait 要求实现
  async fn upstream_peer(
    &self,
    _session: &mut Session,
    _ctx: &mut (),
  ) -> Result<Box<HttpPeer>> {
    Err(pingora::Error::new_str("unreachable: redirect短路 already returned"))
  }
}

fn main() {
  env_logger::init();
  let mut server = Server::new(None).unwrap();
  server.bootstrap();

  // 443: TLS 终止 + 反代到 app
  let mut proxy = http_proxy_service(&server.configuration, AppGateway);
  let cert = std::env::var("TLS_CERT_PATH")
    .expect("TLS_CERT_PATH 未配置：指向 fullchain.pem");
  let key = std::env::var("TLS_KEY_PATH")
    .expect("TLS_KEY_PATH 未配置：指向 privkey.pem");
  let mut tls =
    pingora_core::listeners::tls::TlsSettings::intermediate(&cert, &key).unwrap();
  tls.enable_h2();
  proxy.add_tls_with_settings("0.0.0.0:443", None, tls);
  server.add_service(proxy);

  // 80: HTTP→HTTPS 301
  let mut redirect = http_proxy_service(&server.configuration, HttpToHttps);
  redirect.add_tcp("0.0.0.0:80");
  server.add_service(redirect);

  server.run_forever();
}
```

#### TLS 证书来源

Pingora **不自带 ACME**，证书需外部提供。两种推荐路径：

| 方式 | 推荐场景 | 命令 |
| --- | --- | --- |
| `certbot --standalone` | 单机部署；先停 Pingora、申请、再启动 | `certbot certonly --standalone -d example.com` |
| [`lego`](https://github.com/go-acme/lego) | 容器化部署；可独立 sidecar 跑续期 | `lego --email you@example.com --domains example.com --http run` |
| Rust 原生：[`instant-acme`](https://crates.io/crates/instant-acme) | 想纯 Rust 栈 | 需自己写续期脚本，~50 行 |

证书续期后给 Pingora 进程发 `SIGHUP` 触发零停机热重载（Pingora 内置）：

```bash
systemctl reload rie-gateway   # 或：kill -HUP $(pidof rie-gateway)
```

#### 运行

```bash
sudo -E TLS_CERT_PATH=/etc/letsencrypt/live/example.com/fullchain.pem \
        TLS_KEY_PATH=/etc/letsencrypt/live/example.com/privkey.pem \
        ./target/release/rie-gateway
```

绑定 443/80 需要 root（或在 systemd unit 加 `AmbientCapabilities=CAP_NET_BIND_SERVICE`，避免长期 root）。推荐打成 systemd unit + `User=rie-gateway` 的非 root 用户跑。

#### 上传体积

app 已在 server fn 内强制 5 MB 限制（Phase 1A.4）。Pingora 默认对 body 大小无硬上限，依赖上游兜底；若需要在边缘提前丢弃超大请求，可在 `request_filter` 内读 `Content-Length` 头 + 拒绝。

#### 与 docker-compose 的关系

`docker-compose.yml` 仍只跑 `app` + `postgres`，并把 8080 绑到 `127.0.0.1:8080`（不暴露公网）。`rie-gateway` 作为**宿主机**进程（或独立容器）监听 443/80 → 转发到 127.0.0.1:8080。这样：
- 升级 app：`docker compose pull app && docker compose up -d app`，Pingora 不受影响。
- 升级网关：`cargo build --release && systemctl reload rie-gateway`，app 不受影响。

### 6.2 Caddy（最简备选，零配置 ACME）

```Caddyfile
example.com {
  encode zstd gzip
  reverse_proxy 127.0.0.1:8080
}
```

Caddy 自动申请 + 续期 Let's Encrypt 证书。无 Rust 工具链时最省心。

### 6.3 nginx

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

### 6.4 Traefik 标签（compose 集成）

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
- **搜索索引目录需挂卷**：Phase 7.3 起索引落盘到 `SEARCH_INDEX_DIR`（默认 `data/search-index`）。docker-compose 已为该路径配卷；裸机部署需自行确保该目录持久化，否则重启会触发一次全量重建（仍可正常对外服务，只是冷启动慢）。
- **moderation provider**：当前 ModerationEngine 为骨架，未接入实际 LLM。Phase 4.3+ 落地。

## 12. 参考

- [CI workflow](.github/workflows/ci.yml) — 跟生产构建路径几乎对齐
- [OPERATIONS.md](OPERATIONS.md) — 日常运维 / 故障排查
- [AUTH_GUIDE.md](AUTH_GUIDE.md) — OAuth 详细配置
- [ENGINES_SPEC.md](ENGINES_SPEC.md) — 引擎层架构
