# OAuth 授权模块设计规范 (Auth Architecture Spec)

本文档定义了 **Rust in Everything** 的多平台 OAuth 认证架构。系统采用“宿主驱动 (Host) + 插件适配 (Plugin)”的混合模式，以支持 GitHub, Google, 飞书, 微信等多种登录方式。

---

## 1. 架构概览 (Architecture Overview)

```text
[浏览器] <--> [Axum Host (Server)] <--> [WASM Auth Plugin]
  |               |                         |
  | 1. 请求登录   | 2. 加载适配器插件       |
  |----------->   | <---------------------> |
  |               | 3. 获取 Auth URL        |
  | 4. 跳转授权   |                         |
  | <-----------  |                         |
  |               |                         |
  | 5. 回调 Code  | 6. 执行 Token 交换 (IO) |
  |----------->   | <---------------------> |
  |               | 7. 获取 Raw Profile     |
  |               |                         |
  |               | 8. 映射标准化用户       |
  |               | <---------------------> |
  | 9. 发放 JWT   | 9. 保存至数据库         |
  | <-----------  |                         |
```

---

## 2. 标准化用户模型 (Standard User Model)

所有认证插件必须将第三方数据转换为以下 `StandardUser` 结构（在 `crates/sdk` 中定义）：

```rust
pub struct StandardUser {
    pub external_id: String,      // 第三方平台唯一 ID
    pub nickname: String,         // 用户昵称
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub provider: String,         // "github", "google", "feishu" 等
    pub raw_data: String,         // 存储原始 Profile JSON 备份
}
```

---

## 3. 插件接口定义 (WASM Export Functions)

每一个 Auth 插件（适配器）必须导出以下函数：

### 3.1 `get_provider_config`
*   **输入**：无
*   **输出**：JSON 字符串
*   **字段**：
    *   `auth_url`: 平台授权页面地址。
    *   `token_url`: 获取 Token 的 API 地址。
    *   `profile_url`: 获取用户信息的 API 地址。
    *   `scopes`: 需要申请的权限列表（数组）。

### 3.2 `map_profile`
*   **输入**：第三方平台返回的原始 JSON 字符串。
*   **输出**：标准化 `StandardUser` JSON 字符串。
*   **职责**：处理字段映射逻辑（如 `login` -> `nickname`）。

---

## 4. 安全策略 (Security)

1.  **Secret 不下发**：`Client Secret` 永远保存在 Host 的环境变量中，严禁下发给 WASM 插件。
2.  **CSRF 防护**：Host 负责生成 `state` 参数并存储在加密的 Session Cookie 中，回调时进行强制校验。
3.  **数据清洗**：插件在 `map_profile` 时必须过滤掉敏感令牌信息，只保留基本资料。
4.  **生产环境回调**：确保在各平台后台注册的回调地址为 `https://rustineverything.app/api/auth/callback/{provider}`。

---

## 5. 权限控制 (Access Control)

系统将权限分为三级：
*   **Public**: 匿名可见（博客列表、文章详情、播客列表）。
*   **Member**: 登录可见（评论发表、收藏、案例实战课）。
*   **Admin**: 管理员可见（文章发布、插件热更新管理）。
