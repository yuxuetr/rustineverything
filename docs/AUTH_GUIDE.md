# 第三方授权登录接入指南

本站支持 GitHub、Google、Discord、X（Twitter）四种 OAuth 登录。四个 provider 内置在宿主代码里（`crates/core/src/auth/provider.rs`）；`site.json` 决定启用哪些、按什么顺序显示，环境变量提供凭据。

---

## 1. 系统架构

```
用户浏览器                     服务端
    │                           │
    │ 点击登录按钮               │
    │──────────────────────────>│ site.json 启用了该 provider？
    │                           │ 读取 {PROVIDER}_CLIENT_ID
    │ 302 重定向到第三方授权页    │ 授权 URL = Provider::spec().auth_url（固定常量）
    │<──────────────────────────│
    │                           │
    │ 授权回调 ?code=xxx        │
    │──────────────────────────>│ 校验 state / PKCE cookie
    │                           │ Token 交换 → spec().token_url（固定常量）
    │                           │ 获取 Profile → spec().profile_url（固定常量）
    │                           │ Provider::map_profile → 缺用户 ID 即登录失败
    │                           │ 写入数据库
    │ 登录成功                   │
    │<──────────────────────────│
```

**安全要点**（SEC-04 / SEC-05）：

- 授权、token、profile 三个端点都是代码里的 https 常量，`client_secret` 只会发往对应 provider 的 token 端点，配置文件无法改动它们。
- 账号匹配用的 provider 用户 ID 只从 profile 响应里取；取不到（错误响应、字段缺失、`0` / 空串）就拒绝登录，不会回退到占位 ID。
- profile 请求按 HTTP 状态码拒绝错误响应。

---

## 2. 快速接入已有 Provider

如果你只需要启用系统已内置的 Provider（GitHub、Google、Discord、Twitter/X），只需两步：

### 第一步：在第三方平台创建 OAuth App

以 GitHub 为例：

1. 前往 [GitHub Developer Settings](https://github.com/settings/developers) → New OAuth App
2. **Homepage URL**：`http://localhost:8080`（生产环境替换为实际域名）
3. **Authorization callback URL**：`http://localhost:8080/api/auth/callback/github`
4. 记录 Client ID 和 Client Secret

> **回调 URL 格式**：`{BASE_URL}/api/auth/callback/{provider_id}`
>
> 其中 `provider_id` 对应 `site.json` 中的 `id` 字段。

### 第二步：配置环境变量和 site.json

在项目根目录的 `.env` 文件中添加凭据（环境变量命名约定：`{PROVIDER_ID 大写}_CLIENT_ID`）：

```env
# GitHub
GITHUB_CLIENT_ID=Ov23lihdNKNIezcqGMab
GITHUB_CLIENT_SECRET=your_secret_here

# Google
GOOGLE_CLIENT_ID=your_google_client_id
GOOGLE_CLIENT_SECRET=your_google_client_secret

# Discord
DISCORD_CLIENT_ID=your_discord_client_id
DISCORD_CLIENT_SECRET=your_discord_client_secret

# Twitter/X
TWITTER_CLIENT_ID=your_twitter_client_id
TWITTER_CLIENT_SECRET=your_twitter_client_secret

# 基础配置
BASE_URL=http://localhost:8080
DATABASE_URL=postgresql://postgres:password@localhost:5432/rustineverything
```

在 `assets/site.json` 的 `auth.providers` 中按显示顺序列出要启用的 provider id：

```json
{
  "auth": {
    "enabled": true,
    "providers": ["github", "google", "discord", "twitter"]
  }
}
```

列出且配置了凭据的 provider 会显示在登录弹窗中；缺凭据的跳过并记 warn 日志，未知 id 同样忽略并记日志。没列出的 provider 即使配了凭据，登录与回调路由也会拒绝。

---

## 3. 各平台接入参考

### GitHub
- **OAuth 文档**：https://docs.github.com/en/apps/oauth-apps
- **回调 URL**：`{BASE_URL}/api/auth/callback/github`
- **Scopes**：`read:user user:email`
- **Profile API 字段**：`id`(int), `login`, `avatar_url`, `email`

### Google
- **OAuth 文档**：https://developers.google.com/identity/protocols/oauth2
- **Google Cloud Console**：https://console.cloud.google.com/apis/credentials
- **回调 URL**：`{BASE_URL}/api/auth/callback/google`
- **Scopes**：`openid email profile`
- **Profile API 字段**：`id`, `name`, `picture`, `email`

### Discord
- **OAuth 文档**：https://discord.com/developers/docs/topics/oauth2
- **Developer Portal**：https://discord.com/developers/applications
- **回调 URL**：`{BASE_URL}/api/auth/callback/discord`
- **Scopes**：`identify email`
- **Profile API 字段**：`id`, `username`, `global_name`, `avatar`(hash), `email`

### Twitter/X
- **OAuth 文档**：https://developer.x.com/en/docs/authentication/oauth-2-0
- **Developer Portal**：https://developer.x.com/en/portal/dashboard
- **回调 URL**：`{BASE_URL}/api/auth/callback/twitter`
- **Scopes**：`users.read tweet.read`
- **Profile API**：v2 `/users/me`，字段在 `data.{id, name, username, profile_image_url}` 下
- **PKCE**：X 要求 PKCE（S256），token 交换用 HTTP Basic 传客户端凭据；两者都已内置

---

## 4. 新增登录方式

在 `crates/core/src/auth/provider.rs` 中：

1. `Provider` 枚举加一个分支，`ALL` 数组同步加上；
2. 写一个 `ProviderSpec` 常量（三个 https 端点、scopes、是否 PKCE、token 交换方式、按钮展示信息）；
3. 补全 `id()` / `spec()` / `map_profile()` 的 `match`——漏掉任何一处都编译不过；
4. 在测试 `maps_each_provider_profile` 与 `profiles_without_a_usable_uid_are_rejected` 中加上该 provider 的真实 profile 样例与缺 ID 样例。

然后在 site.json 的 `auth.providers` 里加上 id，在 `.env` 配 `{ID 大写}_CLIENT_ID` / `_CLIENT_SECRET`，并在第三方平台登记回调 URL `{BASE_URL}/api/auth/callback/{id}`。

`map_profile` 的约定：用户 ID 必须是 provider 给出的稳定、非空 uid，取不到就返回 `None`；昵称缺失时回退为「{显示名} 用户」；头像 URL 只在 profile 给出时填写（注意 CSP `img-src` 只放行四个已知头像 CDN，新 provider 的头像主机要加进 `crates/app/src/server/security.rs`）。

---

## 5. 运行时流程

1. 前端调用 `get_auth_providers()`：服务端按 site.json 的顺序取启用的内置 provider，过滤掉缺凭据的，返回展示信息。
2. 用户点击按钮 → `GET /api/auth/login/{provider}`：provider 必须在 site.json 中启用；生成 state（及 X 的 PKCE verifier），加密写入 `oauth_pkce` cookie，302 到授权页。
3. 第三方回调 `/api/auth/callback/{provider}?code=…&state=…`：校验 cookie 的 provider、state、TTL 与 PKCE verifier → token 交换 → 取 profile（非 2xx 拒绝）→ `map_profile` → 按 `(provider, uid)` 查找或创建账号 → 签发会话。

---

## 6. 已内置的 Provider

| Provider | id / 环境变量前缀 | 协议 | 备注 |
|----------|------------------|------|------|
| GitHub | `github` / `GITHUB_` | OAuth 2.0 | 用户 ID 是数字 |
| Google | `google` / `GOOGLE_` | OAuth 2.0 + OpenID | 需 Google Cloud 项目 |
| Discord | `discord` / `DISCORD_` | OAuth 2.0 | 头像由 CDN URL 构造 |
| X (Twitter) | `twitter` / `TWITTER_` | OAuth 2.0 + PKCE | token 交换用 HTTP Basic |
