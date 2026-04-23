# Session / JWT 会话管理 & 评论系统

## 概述

阶段一实现了完整的用户会话链路：OAuth 登录 → JWT 签发 → Cookie 存储 → 前端用户状态 → 评论关联真实用户。

## 架构

### 会话流程

```
用户点击登录 → OAuth 授权 → 回调 /api/auth/callback/{provider}
  → auth_callback_internal() 验证 code 并同步用户到 DB
  → create_jwt(user) 签发 JWT（7天有效期）
  → Set-Cookie: session=<jwt>; HttpOnly; Path=/; SameSite=Lax
  → 重定向到 /
```

### 前端获取用户

```
App 组件加载 → use_effect 调用 get_current_user()
  → server function 通过 FullstackContext 读取 Cookie
  → parse_session_from_cookie_header() 验证 JWT
  → 返回 Option<SessionUser>
  → 通过 use_context_provider 共享到全局
```

### 登出

`GET /api/auth/logout` → 设置空 Cookie (Max-Age=0) → 重定向到 /

## 关键类型

### SessionUser (`core/src/session.rs`)

前后端共享的用户信息结构体：

```rust
pub struct SessionUser {
    pub id: i32,
    pub nickname: String,
    pub avatar_url: Option<String>,
    pub role: String,
}
```

### Comment (`app/src/server/mod.rs`)

评论 DTO（数据库读取后映射）：

```rust
pub struct Comment {
    pub id: String,
    pub blog_id: String,
    pub content: String,
    pub author: String,
    pub author_avatar: Option<String>,
    pub user_id: Option<i32>,
    pub date: String,
}
```

## 数据库

### comments 表

```sql
CREATE TABLE IF NOT EXISTS comments (
    id SERIAL PRIMARY KEY,
    blog_id VARCHAR(255) NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

执行 `init.sql` 创建表。

## API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/auth/login/{provider}` | GET | 重定向到 OAuth 授权页 |
| `/api/auth/callback/{provider}` | GET | OAuth 回调，签发 JWT Cookie |
| `/api/auth/logout` | GET | 清除 session Cookie |
| `/api/auth/me` | POST | 获取当前登录用户 |
| `/api/auth/providers` | POST | 获取可用的 OAuth 提供商列表 |
| `/api/comments/list` | POST | 获取指定博客的评论列表 |
| `/api/comments/post` | POST | 发表评论（需登录） |

## 环境变量

在 `.env` 中添加：

```
JWT_SECRET=your-secret-key
```

## 前端 Hook

- `use_session_user()` — 获取全局用户 Signal（`Signal<Option<SessionUser>>`）
- `use_auth_modal()` — 获取登录弹窗状态 Signal（`Signal<bool>`）
