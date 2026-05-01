# Admin 后台规范 (3.1)

## 1. 定位
当前站点定位为 **个人/小团队站(方案 A)**:博客内容继续以 `assets/posts/` 文件系统为单一来源,作者通过 git 写作。Admin 后台聚焦"已经存放在数据库里的运营对象":用户、评论、论坛话题/回复、插件状态。

> 多用户在站内发博客(方案 B)、API Token 自动发布、文章审核队列等不在 3.1 范围,设计上保留多作者/多角色友好的接口,以便后续扩展时无需拆表。

## 2. 权限模型
- 所有用户默认 `role = member`(由 OAuth 注册流程统一设置)。
- 取值集合(`crates/core/src/session.rs::ALL_ROLES`):`admin | member | guest`。
- 鉴权流程:
  - `core::session::current_session_user()` 从 Dioxus FullstackContext 的 cookie 中解析 JWT。
  - `core::session::require_session()` 强制要求登录。
  - `core::session::require_admin()` 强制要求 `role == admin`,失败时返回中文 "需要管理员权限"。
- 自我降权保护:`server::check_self_role_change` 在 `admin_set_user_role` 中预检 ——
  操作目标是自己 + 新 role 不是 admin + 系统中没有其他 admin → 拒绝,避免最后一个 admin 自己踢自己。
- 客户端额外做了一层"页面级 403 占位",但权限的最终判定永远来自服务端 `require_admin()`。

## 3. 路由与页面
路由声明在 `crates/app/src/routes/mod.rs`。所有页面共享 `AdminShell`(左侧导航)。

| 路径 | 组件 | 用途 |
|---|---|---|
| `/admin` | `AdminDashboardPage` | 概览统计(用户/管理员/评论/话题/回复/标注) |
| `/admin/users` | `AdminUsersPage` | 用户列表 + 切换 role |
| `/admin/comments` | `AdminCommentsPage` | 评论列表 + 删除 |
| `/admin/topics` | `AdminTopicsPage` | 话题列表 + 删除(级联回复) |
| `/admin/plugins` | `AdminPluginsPage` | wasm 插件状态视图 + 重新载入 |

Navbar 用户下拉菜单中,只有 admin 可见 "🛡️ 管理后台" 链接。

## 4. Server API
所有 server fn 均要求 `require_admin`,响应风格统一。

| API | 入参 | 返回 |
|---|---|---|
| `POST /api/admin/overview` | - | `AdminOverview` |
| `POST /api/admin/users/list` | `page: Option<u32>` | `AdminPage<AdminUserRow>` |
| `POST /api/admin/users/set-role` | `user_id: i32, role: String` | `AdminUserRow` |
| `POST /api/admin/comments/list` | `page: Option<u32>` | `AdminPage<AdminCommentRow>` |
| `POST /api/admin/comments/delete` | `id: i32` | `()` |
| `POST /api/admin/topics/list` | `page: Option<u32>` | `AdminPage<AdminTopicRow>` |
| `POST /api/admin/topics/delete` | `id: i32` | `()` |
| `POST /api/admin/replies/delete` | `id: i32` | `()` |
| `POST /api/admin/plugins/list` | - | `Vec<AdminPluginRow>` |
| `POST /api/admin/plugins/reload` | - | `String` |

`AdminPage<T>` 字段:`items, total, page, page_size`。`ADMIN_PAGE_SIZE = 50`,`MAX_PAGE = 10000`。

## 5. 数据约束
- 删除话题依赖 `topic_replies` 上的 `ON DELETE CASCADE`(已存在于 `init.sql`)。
- 删除单条回复使用事务:删除 + 重新计算 `topics.reply_count`。
- 评论硬删除,无软删除/审计(后续 PR 增加)。

## 6. 本地引导管理员
首次没有 admin 时,运行:

```sh
DATABASE_URL=postgres://postgres:password@localhost/rustineverything \
  scripts/promote_admin.sh <你的昵称>
```

脚本将 `users.role` 改为 `admin`,然后重新登录一次,新签发的 JWT 即带 admin 角色。

## 7. 插件视图
- 数据来源:`assets/site.json`(auth providers + active_theme) + `assets/plugins/*.wasm` 实际文件。
- 字段:`kind / id / filename / configured / credentials_ready / present / size_bytes / modified`。
- "重新载入"按钮:当前 `PluginManager` 每次调用都重新读 wasm,无缓存可清,接口仅返回成功 + 打日志,用作未来缓存接入点。

## 8. 不在本期范围
- 软删除 / 操作审计 / 通知。
- 用户禁言、IP 封禁、邀请码。
- 文章发布(博客继续走 `assets/posts/` git 工作流)。
- API Token 与 `/api/v1/...` 公开接口。

## 9. 测试覆盖
`crates/modules/admin/src/server.rs` 提供纯逻辑单测:`clamp_page` / `validate_role` / `check_self_role_change` / `classify_plugin_kind`。
`crates/modules/admin/src/admin.rs` 提供 `compute_total_pages` 单测。
跑测试:

```sh
cargo test --features server -p rustineverything-module-admin
```
