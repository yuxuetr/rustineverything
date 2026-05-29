# 第三方授权登录接入指南

本文档介绍 Rust in Everything 平台的插件化 OAuth 授权登录系统。该系统基于 WASM 插件架构，支持用户通过 `site.json` 配置文件灵活控制启用哪些登录方式，并允许开发者编写自定义 Auth 插件来接入任意第三方平台。

---

## 1. 系统架构

```
用户浏览器                     服务端                           WASM 插件
    │                           │                               │
    │ 点击登录按钮               │                               │
    │──────────────────────────>│ 读取 site.json                │
    │                           │ 加载 {provider}_auth_plugin   │
    │                           │──────────────────────────────>│ get_provider_config()
    │                           │<──────────────────────────────│ → auth_url, scopes...
    │                           │ 读取 {PROVIDER}_CLIENT_ID     │
    │ 302 重定向到第三方授权页    │<─────────────────────────────│
    │<──────────────────────────│                               │
    │                           │                               │
    │ 授权回调 ?code=xxx        │                               │
    │──────────────────────────>│ Token 交换 (宿主完成)         │
    │                           │ 获取用户 Profile (宿主完成)   │
    │                           │──────────────────────────────>│ map_profile(raw_json)
    │                           │<──────────────────────────────│ → StandardUser
    │                           │ 写入数据库                    │
    │ 登录成功                   │<─────────────────────────────│
    │<──────────────────────────│                               │
```

**核心原则**：插件只负责"数据描述"和"字段映射"，所有涉及网络请求、密钥管理、数据库操作均由宿主完成。

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

在 `assets/site.json` 的 `auth.providers` 中启用需要的 Provider：

```json
{
  "auth": {
    "enabled": true,
    "providers": [
      { "id": "github", "plugin": "github_auth_plugin.wasm" },
      { "id": "google", "plugin": "google_auth_plugin.wasm" },
      { "id": "discord", "plugin": "discord_auth_plugin.wasm" },
      { "id": "twitter", "plugin": "twitter_auth_plugin.wasm" }
    ]
  }
}
```

系统运行时会自动检查：**WASM 插件存在 + 环境变量已配置** → 在登录模态框中显示该 Provider 按钮。未配置凭据的 Provider 会被静默跳过。

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
- **注意**：Twitter OAuth 2.0 使用 PKCE 流程，当前宿主端尚未生成 `code_challenge`，接入前需补充此逻辑

---

## 4. 开发自定义 Auth 插件

如果需要接入系统未内置的平台（如微信、飞书、GitLab 等），可以开发自定义 Auth 插件。

### 4.1 创建插件 Crate

```bash
mkdir -p crates/plugins/my-auth/src
```

`crates/plugins/my-auth/Cargo.toml`：

```toml
[package]
name = "my-auth-plugin"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
sdk = { path = "../../sdk" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 4.2 实现三个必需导出函数

每个 Auth 插件必须导出以下三个函数：

#### `get_provider_config` — 返回 OAuth 端点配置

```rust
use sdk::{alloc, dealloc, AuthProviderConfig, AuthProviderDisplay, StandardUser};
use std::slice;
use serde_json::Value;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_provider_config(_ptr: *mut u8, _len: usize) -> u64 {
    let config = AuthProviderConfig {
        auth_url: "https://example.com/oauth/authorize".to_string(),
        token_url: "https://example.com/oauth/token".to_string(),
        profile_url: "https://api.example.com/user".to_string(),
        scopes: vec!["read:user".to_string(), "email".to_string()],
    };

    let result_str = serde_json::to_string(&config).unwrap_or_default();
    unsafe { pack_result(result_str) }
}
```

**SDK 类型 `AuthProviderConfig`**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `auth_url` | `String` | 用户授权页面 URL |
| `token_url` | `String` | Token 交换 API URL |
| `profile_url` | `String` | 获取用户信息 API URL |
| `scopes` | `Vec<String>` | 需要申请的权限列表 |

#### `get_display_info` — 返回前端展示信息

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_display_info(_ptr: *mut u8, _len: usize) -> u64 {
    let display = AuthProviderDisplay {
        provider_id: "myplatform".to_string(),
        display_name: "My Platform".to_string(),
        icon_svg: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10...".to_string(), // SVG path d
        brand_color: "#FF6600".to_string(), // 按钮背景色
    };

    let result_str = serde_json::to_string(&display).unwrap_or_default();
    unsafe { pack_result(result_str) }
}
```

**SDK 类型 `AuthProviderDisplay`**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `provider_id` | `String` | 插件标识，需与 site.json 中的 `id` 一致 |
| `display_name` | `String` | 登录按钮上显示的名称 |
| `icon_svg` | `String` | SVG `<path d="...">` 的 d 属性值 |
| `brand_color` | `String` | 按钮品牌色 hex（亮色背景会自动使用深色文字） |

#### `map_profile` — 将第三方原始 Profile 映射为标准用户

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn map_profile(ptr: *mut u8, len: usize) -> u64 {
    let input_bytes = unsafe { slice::from_raw_parts(ptr, len) };
    let raw: Value = serde_json::from_slice(input_bytes).unwrap_or_default();

    let user = StandardUser {
        external_id: raw["uid"].as_str().unwrap_or("0").to_string(),
        nickname: raw["display_name"].as_str().unwrap_or("User").to_string(),
        avatar_url: raw["avatar"].as_str().map(|s| s.to_string()),
        email: raw["email"].as_str().map(|s| s.to_string()),
        provider: "myplatform".to_string(),
        raw_data: raw.to_string(),
    };

    let result_str = serde_json::to_string(&user).unwrap_or_default();
    unsafe { pack_result(result_str) }
}
```

**SDK 类型 `StandardUser`**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `external_id` | `String` | 平台唯一用户 ID |
| `nickname` | `String` | 用户昵称 |
| `avatar_url` | `Option<String>` | 头像 URL |
| `email` | `Option<String>` | 邮箱 |
| `provider` | `String` | 平台标识 |
| `raw_data` | `String` | 原始 Profile JSON 备份 |

#### 辅助函数 `pack_result` 和 `plugin_unused_fix`

```rust
/// 将字符串打包为 (ptr << 32 | len) 格式返回给宿主
unsafe fn pack_result(s: String) -> u64 {
    let bytes = s.into_bytes();
    let len = bytes.len();
    let ptr = unsafe { alloc(len) };
    let dst = unsafe { slice::from_raw_parts_mut(ptr, len) };
    dst.copy_from_slice(&bytes);
    ((ptr as u64) << 32) | (len as u64)
}

/// 确保 dealloc 被链接（WASM 编译需要）
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plugin_unused_fix() {
    unsafe { let _ = dealloc(std::ptr::null_mut(), 0); }
}
```

### 4.3 编译与部署

```bash
# 1. 添加到 workspace（根 Cargo.toml）
# members = [..., "crates/plugins/my-auth"]

# 2. 编译 WASM
CARGO_TARGET_DIR=~/.target cargo build \
  --manifest-path crates/plugins/my-auth/Cargo.toml \
  --target wasm32-unknown-unknown --release

# 3. 部署到 assets
cp ~/.target/wasm32-unknown-unknown/release/my_auth_plugin.wasm assets/plugins/
```

### 4.4 注册到 site.json

```json
{
  "auth": {
    "enabled": true,
    "providers": [
      { "id": "myplatform", "plugin": "my_auth_plugin.wasm" }
    ]
  }
}
```

### 4.5 配置环境变量

```env
MYPLATFORM_CLIENT_ID=xxx
MYPLATFORM_CLIENT_SECRET=xxx
```

命名规则：`{PROVIDER_ID 转大写}_CLIENT_ID` / `_CLIENT_SECRET`。

---

## 5. 运行时流程详解

当用户点击登录按钮时：

1. **前端**调用 `get_auth_providers()` → 服务端读取 `site.json`
2. **服务端**遍历 `auth.providers`，对每个 entry 执行：
   - 检查 `assets/plugins/{plugin}` 文件是否存在
   - 检查 `{ID}_CLIENT_ID` 和 `{ID}_CLIENT_SECRET` 环境变量是否已设置
   - 加载 WASM 并调用 `get_display_info()` 获取展示信息
3. **前端**渲染登录模态框，动态显示可用的 Provider 按钮
4. 用户点击某个 Provider → 前端调用 `get_login_url(provider_id)`
5. **服务端**查找 `site.json` 获取插件文件名 → 加载 WASM 调用 `get_provider_config()` → 组装授权 URL
6. 浏览器 302 跳转到第三方授权页
7. 用户授权后回调到 `/api/auth/callback/{provider}?code=xxx`
8. **服务端**执行 Token 交换 → 获取 Profile → 加载 WASM 调用 `map_profile()` → 写入数据库

---

## 6. 开发检查清单

- [ ] **edition 2024**：使用 `#[unsafe(no_mangle)]` 而非 `#[no_mangle]`
- [ ] **三个导出函数**：`get_provider_config`、`get_display_info`、`map_profile` 均已实现
- [ ] **provider_id 一致**：`get_display_info` 返回的 `provider_id` 与 `site.json` 中的 `id` 一致
- [ ] **编译目标**：使用 `--target wasm32-unknown-unknown`
- [ ] **环境变量命名**：`{PROVIDER_ID 大写}_CLIENT_ID` 和 `_CLIENT_SECRET`
- [ ] **回调 URL**：在第三方平台配置 `{BASE_URL}/api/auth/callback/{provider_id}`
- [ ] **无 IO 操作**：插件内不发起网络请求或文件读写（由宿主完成）
- [ ] **内存安全**：通过 `alloc` 分配的内存由宿主 `dealloc` 释放
- [ ] **SVG 图标**：`icon_svg` 为标准 24x24 viewBox 的 `<path d="...">` 值
- [ ] **品牌色**：`brand_color` 为 6 位 hex 值（如 `#24292f`），系统会自动选择黑/白文字色

---

## 7. 已内置的 Auth 插件

| Provider | plugin 文件名 | 环境变量前缀 | OAuth 版本 | 备注 |
|----------|--------------|-------------|-----------|------|
| GitHub | `github_auth_plugin.wasm` | `GITHUB_` | OAuth 2.0 | 已验证 |
| Google | `google_auth_plugin.wasm` | `GOOGLE_` | OAuth 2.0 + OpenID | 需 Google Cloud 项目 |
| Discord | `discord_auth_plugin.wasm` | `DISCORD_` | OAuth 2.0 | 头像通过 CDN URL 构造 |
| Twitter/X | `twitter_auth_plugin.wasm` | `TWITTER_` | OAuth 2.0 + PKCE | 需补充 PKCE code_challenge |
