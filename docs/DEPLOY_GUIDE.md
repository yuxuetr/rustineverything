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

**功能分层**：镜像默认是完整站点。要部署其他档位（见 [FEATURE_TIERS.md](FEATURE_TIERS.md)），构建时传
`--build-arg APP_FEATURES="..."`，例如只要博客 / 文档 / 播客、不要数据库：

```bash
docker build --build-arg APP_FEATURES="--no-default-features" -t rie-app:basic .
```

basic 档只需要 `BASE_URL`，不连库、不要求 `JWT_SECRET`，`/healthz` 返回 `"db":"disabled"`（正常）；搜索索引在启动时增量刷新。
带 `accounts` 的档位与完整站点一样需要 Postgres 和 `JWT_SECRET`。

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

最少必须填的三项（缺任何一项 compose 拒绝启动）：

```dotenv
JWT_SECRET=<32+ 字符随机串；用 openssl rand -hex 32 生成>
BASE_URL=https://example.com         # 生产；本地可用 http://127.0.0.1:8080
POSTGRES_PASSWORD=<强随机密码；openssl rand -hex 24>
```

可选项：私有内容仓库（§3.5）、OAuth 凭据、`RUST_LOG`。完整变量见 [`.env.example`](../.env.example)。

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

### 3.5 私有内容仓库（文章与课程）

框架仓库只带示例内容（`assets/posts/welcome`、`assets/courses/rust-basics`），其余文章与课程被 `.gitignore` 挡住。站点自己的内容放在单独的私有仓库，结构为 `posts/<slug>/index.mdx` 与 `courses/<course>/...`。

服务器上把两个仓库并排克隆，框架的 `.env` 指向内容仓库：

```bash
git clone https://github.com/<owner>/rustineverything.app.git
git clone git@github.com:<owner>/<content-repo>.git   # 私有仓库：服务器上配一把只读 deploy key
cd rustineverything.app
echo 'CONTENT_HOST_DIR=../<content-repo>' >> .env
docker compose up -d --build
```

compose 把 `CONTENT_HOST_DIR` 只读挂到容器的 `/app/content`（app 设 `CONTENT_DIR=/app/content`）；不设时挂框架自带的 `./assets`，即示例内容。

| 要做的事 | 命令 | 是否重启 |
| --- | --- | --- |
| 发布 / 修改文章或课程 | `git -C ../<content-repo> pull` | 不用。页面立即生效，搜索索引 10 分钟内增量刷新（急用可在后台点「重建索引」） |
| 升级框架 | `git pull && docker compose up -d --build` | 重建 app 镜像，内容不受影响 |

本地写作：框架 `.env` 设 `CONTENT_DIR=<内容仓库绝对路径>`，`dx serve` 后刷新页面即可看到新文章。

### 3.6 生产：用发布的镜像

服务器上不编译（app 镜像要几 GB 内存构建）。镜像由 `.github/workflows/release.yml` 在 CI 构建，
`docker-compose.prod.yml` 叠加在基础文件上：app 用 ghcr 镜像（accounts 档）、postgres 不开端口、
加 `rie-gateway`（host 网络监听 80 / 443）、各服务设内存上限。

**发布镜像**：打 tag 推上去，CI 构建 linux/amd64 的 `ghcr.io/<owner>/rie-app` 与 `rie-gateway`，
标签为 tag 名、短 sha 和 `latest`。第一次发布后在 GitHub 的 Packages 页把两个包设为 Public，服务器才能免登录拉取。

```bash
git tag v0.1.1 && git push origin v0.1.1
```

**服务器布局**（`/data/rie`）：

```text
/data/rie/
  rustineverything/          框架仓库：只用到 compose 文件和 scripts/
  rustineverything-content/  内容仓库（只读 deploy key）
  certs/                     fullchain.pem、privkey.pem（600）
  cert.env                   DYNADOT_API_KEY=...（600），cert_refresh.sh 读
  backups/                   backup.sh 写
```

**`.env`**：在服务器上生成，密钥不经过别的机器：

```bash
cd /data/rie/rustineverything
umask 077
cat > .env <<ENV
COMPOSE_FILE=docker-compose.yml:docker-compose.prod.yml
RIE_TAG=v0.1.1
BASE_URL=https://example.com
CONTENT_HOST_DIR=/data/rie/rustineverything-content
POSTGRES_PASSWORD=$(openssl rand -hex 24)
JWT_SECRET=$(openssl rand -hex 32)
DATA_ENCRYPTION_KEY=$(openssl rand -hex 32)
ENV
```

`COMPOSE_FILE` 让之后的 `docker compose` 命令自动叠加 prod 文件，不用每次写 `-f`。

**证书**：网关不自带 ACME（§6.1）。本站用域名注册商 Dynadot 签发的 Let's Encrypt 证书：Dynadot 在到期前约一个月自动续期，
`scripts/cert_refresh.sh` 每天从永久下载链接取回，检查通过（PEM、域名、与现有私钥配对、证书链、到期日更晚）才替换并重启网关，
任一项不过就不动现有证书。首次把证书和私钥放进 `certs/`。用 certbot 等其他来源时，续期后执行 `docker restart rie-gateway` 即可。

**启动**：

```bash
docker compose pull
docker compose up -d
docker compose logs -f app | grep "schema migrations"
```

**cron**（`crontab -e`）：

```cron
MAILTO=you@example.com
0 4 * * * /data/rie/rustineverything/scripts/cert_refresh.sh >> /data/rie/certs/refresh.log 2>&1
30 3 * * * /data/rie/rustineverything/scripts/backup.sh >> /data/rie/backups/backup.log 2>&1
```

另给 `.github/workflows/cert-check.yml` 加 schedule，每天从外网查线上证书剩余天数（`scripts/check_cert_expiry.sh`），少于 20 天就红。

| 要做的事 | 命令 |
| --- | --- |
| 更新文章 / 课程 | `git -C /data/rie/rustineverything-content pull`（不重启） |
| 升级框架 | `scripts/backup.sh`；`git pull`（compose 文件与脚本）；`.env` 里改 `RIE_TAG`；`docker compose pull && docker compose up -d` |
| 回滚框架 | `.env` 里把 `RIE_TAG` 改回旧版本后 `docker compose up -d`；新版本带迁移时先按 OPERATIONS.md §2.2 恢复升级前的备份 |

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
  -v /srv/<content-repo>:/app/content:ro -e CONTENT_DIR=/app/content \
  rustineverything:latest
```

挂卷 `/app/assets/uploads` 让用户图片在容器重启 / 滚动升级时不丢。内容仓库的挂载见 §3.5；不挂则显示镜像里的示例内容。

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
`target/release/rie-gateway` 二进制；下面的 Cargo.toml + main.rs 摘录了反代
+ 301 跳转的核心骨架，完整版（含 Phase 8.3 安全头 + per-IP 限流）请直接看
[`crates/gateway/src/main.rs`](../crates/gateway/src/main.rs)。

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
| 域名注册商签发（如 Dynadot） | 注册商托管 DNS、自动续期 | `scripts/cert_refresh.sh` 每天取回（§3.6） |

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

#### 安全响应头 + 限流（Phase 8.3）

`crates/gateway/src/main.rs` 在每个响应上处理安全头；浏览器 devtools 可见：

| Header | gateway 行为 | 值 |
| --- | --- | --- |
| `Strict-Transport-Security` | 总是设置（TLS 在 gateway 终止） | `max-age=31536000; includeSubDomains` |
| `X-Content-Type-Options` | 应用未下发时补 | `nosniff` |
| `X-Frame-Options` | 应用未下发时补 | `DENY` |
| `Referrer-Policy` | 应用未下发时补 | `strict-origin-when-cross-origin` |
| `Content-Security-Policy` | **不碰**，原样转发应用的值 | 见 `crates/app/src/server/security.rs`；在 **app** 进程设 `CSP_POLICY` 覆盖 |
| `Server` | 总是设置 | `rie-gateway` |

CSP 只有应用这一份定义（SEC-10）：gateway 若另设一份，会覆盖掉应用 CSP 里 hydration 必需的
`'wasm-unsafe-eval'` 与内联启动脚本许可，页面整体失去交互。

gateway 不限流，限流只在 app 里做（`crates/app/src/server/rate_limit.rs`：只限 `/api/*`，按 gateway 写入的 `X-Forwarded-For` 分桶，环境变量见 `.env.example` 的 `RATE_LIMIT_*`）。gateway 原先也按 IP 限流，但把静态资源算在内：一次冷加载文档页约 57 个请求，读配额 60 / 分钟刷新一次就 429，`/api/auth/me` 还算进 10 / 分钟的写配额。

`X-Forwarded-For` 改用 `insert_header` 覆盖客户端伪造，同时 strip 掉 RFC 7239 `Forwarded` 头；下游 app 看到的 XFF 一定是 Pingora 取到的真实 TCP 对端 IP。

边缘验证：

```bash
# 安全头检查（任意路径）
curl -kI https://your-domain.com/ | grep -iE 'strict-transport|x-frame|x-content-type|content-security-policy|referrer-policy'
```

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

## 7. 在线支付（可选）

默认镜像带在线支付（`payments` feature）。一个网关都没配时：购买弹窗提示「该地区暂无可用的支付方式」，打赏卡片不显示；
运营仍可在 `/admin/entitlements` 手动开通课程与会员。网关各自独立，配哪个启用哪个。

### 7.1 变量与回调地址

变量清单与注释见 [`.env.example`](../.env.example) 支付段，细节见 [PAYMENT_SPEC.md](PAYMENT_SPEC.md) §8 / §15.3。
三个网关共用 `PAY_NOTIFY_BASE`（公网 https 根地址，通常与 `BASE_URL` 相同）。

| 网关 | 变量 | 回调地址（填到网关后台） |
| --- | --- | --- |
| 支付宝 | `ALIPAY_APP_ID` `ALIPAY_APP_PRIVATE_KEY` `ALIPAY_PUBLIC_KEY`（可选 `ALIPAY_GATEWAY`） | 下单时自动带上 `{PAY_NOTIFY_BASE}/api/pay/alipay/notify`，无需后台配置 |
| 微信支付 v3 | `WECHAT_MCHID` `WECHAT_APP_ID` `WECHAT_API_V3_KEY` `WECHAT_MCH_PRIVATE_KEY` `WECHAT_MCH_SERIAL_NO` `WECHAT_PLATFORM_PUBLIC_KEY` | 下单时自动带上 `{PAY_NOTIFY_BASE}/api/pay/wechat/notify` |
| Stripe | `STRIPE_SECRET_KEY` `STRIPE_WEBHOOK_SECRET` | 在 Stripe 后台添加 webhook：`{PAY_NOTIFY_BASE}/api/pay/stripe/notify`（见 7.2） |

启动日志逐个网关报告状态，部署后先看这几行：

```bash
docker compose logs app | grep 'payment gateway'
# payment gateway enabled gateway="stripe"
# payment gateway partly configured; disabled gateway="alipay" missing="ALIPAY_PUBLIC_KEY"
```

`partly configured` 列出缺的变量名；`value is malformed` 表示都设了但某个值格式不对（密钥不是 PEM / base64、
`WECHAT_API_V3_KEY` 不是 32 字节）。日志只有变量名，不含值。

### 7.2 Stripe 上线

1. 后台切到正式模式，`STRIPE_SECRET_KEY` 填 `sk_live_…`。测试密钥只留在本地；贴到过聊天或工单里的测试密钥在后台 roll 掉。
2. Developers → Webhooks 添加端点 `https://<域名>/api/pay/stripe/notify`，订阅
   `checkout.session.completed`、`checkout.session.async_payment_succeeded`、`charge.refunded`、`charge.dispute.created`（后两个用于在 Stripe 后台退款或买家拒付时撤销课程权限）；把该端点的 signing secret（`whsec_…`）填进 `STRIPE_WEBHOOK_SECRET`。正式与测试模式的 secret 不同。
3. 要在 Stripe 收银台出现支付宝 / 微信支付：在 Settings → Payment methods 里开通。代码不指定支付方式，收银台显示后台已开通的全部方式。Stripe 账户须在中国大陆以外注册。

### 7.3 上线检查

- 反向代理放行 `/api/pay/*` 的 POST，不改写请求体：回调的签名覆盖请求内容，改过就验不过、不发货。
- 回调走 app 的敏感限流桶（每 IP 5 rps、突发 15，见 `crates/app/src/server/rate_limit.rs`）。偶尔被限流也不会丢单：网关会重试，对账任务（默认每 5 分钟）会向网关查单并补发货。
- 小额实付一单 → 我的订单显示已支付、课程解锁 → 管理员在 `/admin/entitlements` 退款 → 订单变为已退款。Stripe 后台的 webhook 投递记录应全是 200。

## 8. site.json 配置

`assets/site.json` 控制站点形态（站点名 / 主题 / 模块开关 / 布局）。
完整字段见 [`THEME_SPEC.md`](THEME_SPEC.md) + [`MODULE_SPEC.md`](MODULE_SPEC.md)。

部署时常见调整：

```jsonc
{
  "site_name": "你的站点名",
  "site_description": "副标题",
  "theme": "sunset",                         // 内置主题 id：ocean / sunset / catppuccin
  "active_layout": "classic",                // 或 "minimal"
  "modules": {
    "forum": { "enabled": true },
    "podcast": { "enabled": false }          // 关闭某模块
  },
  "auth": {
    "enabled": true,
    "providers": ["github"]                  // 内置 provider id，按显示顺序
  }
}
```

修改后只需重启 app 容器，无需重新 build 镜像：

```bash
docker compose restart app
```

## 9. 内容资产

`assets/` 包含运行时数据：

| 子目录 | 用途 | 是否随镜像 |
| --- | --- | --- |
| `posts/` | 博客 MDX | ✅ 镜像内 |
| `docs/` | 文档 MDX | ✅ |
| `courses/` | 课程多媒体 | ✅ |
| `podcasts/` | 播客音频元数据 | ✅ |
| `cases/` | 案例展示 | ✅ |
| `audio/` | 大体积音频 | ✅（10MB 以下；超过被 build.rs 跳过） |
| `uploads/` | **用户上传** | ❌ 走持久卷 |

新内容上线流程：
1. 在本地把 MDX 放到 `assets/posts/<slug>.md`
2. `git commit` + push
3. 在服务器上 `git pull && docker compose build app && docker compose up -d app`

## 10. 升级流程

用 §3.6 发布的镜像部署时，升级见 §3.6 末尾的表格。下面是在服务器上自己构建镜像的流程。

```bash
# 0. 备份数据库与上传文件（OPERATIONS.md §2）
scripts/backup.sh

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

## 11. 安全 checklist（上线前）

| 项目 | 检查 |
| --- | --- |
| `JWT_SECRET` | 32+ 字符随机串，**不与任何 git 历史共享** |
| `BASE_URL` | https；与 OAuth callback 一致 |
| postgres 密码 | 强密码；不暴露 5432 端口到公网（compose 默认仅 127.0.0.1） |
| Cookie Secure | `BASE_URL` 以 `https://` 起首时自动启用（main.rs cookie_is_secure） |
| 反向代理 | TLS 终止 + `X-Forwarded-Proto: https` |
| app 端口 | compose 默认只绑 `127.0.0.1:8080`（`APP_BIND`）；对外只开网关 / 反代。限流按 `X-Forwarded-For` 区分客户端，直连 app 的请求可伪造该头绕过限流 |
| `RUST_LOG=info` | 而非 `debug`，避免敏感请求参数进日志 |
| 在线支付 | 启动日志里要用的网关都是 `payment gateway enabled`；Stripe 用 `sk_live_` 且 webhook 已建（§7） |
| Admin 升级 | 创建第一个 admin 后**移除** `users` 表的开放写权限脚本 |
| 容器更新 | 定期 `docker compose pull` 后台镜像（postgres） |

## 12. 已知限制

- **备份只在本机**：`scripts/backup.sh` 把数据库和上传文件写到同一台服务器的 `/data/rie/backups`（OPERATIONS.md §2），异地复制需自行安排。
- **搜索索引目录需挂卷**：Phase 7.3 起索引落盘到 `SEARCH_INDEX_DIR`（默认 `data/search-index`）。docker-compose 已为该路径配卷；裸机部署需自行确保该目录持久化，否则重启会触发一次全量重建（仍可正常对外服务，只是冷启动慢）。
- **卷的属主**：镜像里预先建好 `/app/assets/uploads`、`/app/data/search-index` 并归 `app`（uid 999），新建的命名卷沿用这个属主。用旧镜像建出的卷归 root，容器里上传和搜索索引都会报 `Permission denied`，在宿主上执行一次 `chown 999:999 /var/lib/docker/volumes/rustineverything_app-{uploads,search-index}/_data` 即可。
- **moderation provider**：当前 ModerationEngine 为骨架，未接入实际 LLM。Phase 4.3+ 落地。

## 13. 参考

- [CI workflow](.github/workflows/ci.yml) — 跟生产构建路径几乎对齐
- [OPERATIONS.md](OPERATIONS.md) — 日常运维 / 故障排查
- [AUTH_GUIDE.md](AUTH_GUIDE.md) — OAuth 详细配置
- [ENGINES_SPEC.md](ENGINES_SPEC.md) — 引擎层架构
