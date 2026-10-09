# Admin 后台规范 (3.1)

## 1. 定位
当前站点定位为 **个人/小团队站(方案 A)**:博客内容继续以 `assets/posts/` 文件系统为单一来源,作者通过 git 写作。Admin 后台聚焦"已经存放在数据库里的运营对象":用户、评论、论坛话题/回复、审核队列与审核设置、课程权益与会员。

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
| `/admin/moderation` | `AdminModerationPage` | 审核队列复核(见 §7) |
| `/admin/moderation/settings` | `AdminModerationSettingsPage` | 审核开关 / 阈值 / LLM 失败处理(见 §8) |
| `/admin/entitlements` | `AdminEntitlementsPage` | 课程权益与 Pro 会员的手动授予 / 撤销(见 §9) |

`AdminEntitlementsPage` 在 `crates/app/src/components/admin_entitlements.rs`(组合根):
它复用 admin 模块的 `AdminShell`,调用的是 course 模块的 server fn,放在这里是为了不让 admin 反向依赖 course。
其余页面都在 `crates/modules/admin/src/admin.rs`。

Navbar 用户下拉菜单中,只有 admin 可见 "🛡️ 管理后台" 链接。

**删除类操作一律先确认**:评论删除、话题删除、审核「拒绝(删除内容)」与「批量拒绝」都经 `ConfirmDelete`
弹出 AlertDialog,确认框写明删除对象(批量写明条数),确认后才调用 server fn。删除是硬删除,无法恢复。

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
| `POST /api/admin/moderation/list` | `filter_status: Option<String>, limit: Option<u64>` | `Vec<ModerationQueueRow>` |
| `POST /api/admin/moderation/approve` | `id: i64` | `()` |
| `POST /api/admin/moderation/reject` | `id: i64` | `()` |
| `POST /api/admin/moderation/bulk-approve` | `ids: Vec<i64>` | `u64`（更新条数） |
| `POST /api/admin/moderation/bulk-reject` | `ids: Vec<i64>` | `u64`（处理条数） |
| `POST /api/admin/moderation/settings/get` | - | `ModerationSettings` |
| `POST /api/admin/moderation/settings/set` | `settings: ModerationSettings` | `()` |
| `POST /api/courses/entitlements/list` | - | `Vec<EntitlementInfo>` |
| `POST /api/courses/entitlements/grant` | `user_id: i32, course_slug: String` | `()`(幂等) |
| `POST /api/courses/entitlements/revoke` | `user_id: i32, course_slug: String` | `()` |
| `POST /api/courses/membership/list` | - | `Vec<MembershipAdminInfo>` |
| `POST /api/courses/membership/grant` | `user_id: i32, days: i64` | `()`(在现有有效期或当前时间上续 `days` 天) |
| `POST /api/courses/membership/revoke` | `user_id: i32` | `()` |

后 6 个定义在 `crates/modules/course/src/server.rs`,同样以 `require_admin()` 开头。

`AdminPage<T>` 字段:`items, total, page, page_size`。`ADMIN_PAGE_SIZE = 50`,`MAX_PAGE = 10000`。

## 5. 数据约束
- 删除话题依赖 `topic_replies` 上的 `ON DELETE CASCADE`(已存在于 `init.sql`)。
- 删除单条回复使用事务:删除 + 重新计算 `topics.reply_count`。
- 评论硬删除,无软删除/审计(后续 PR 增加)。

## 6. 本地引导管理员
首次没有 admin 时,先用 OAuth 登录一次(建出用户行),再在仓库根目录运行(需要 `psql`,读 `.env` 里的 `DATABASE_URL`):

```sh
set -a; source .env; set +a
scripts/promote_admin.sh <你的昵称>
```

脚本将 `users.role` 改为 `admin`。角色写在 JWT 里,所以要退出并重新登录一次,新签发的 JWT 才带 admin 角色。
之后再用 OAuth 登录不会覆盖已有角色;降级 / 升级其他用户在 `/admin/users` 里操作。

## 7. 审核队列复核(`/admin/moderation`,Phase 4.5 + 增强)
- 列表按状态 Tab 过滤(待复核/已通过/已拒绝/全部),每行展示状态徽章/类型/路径/作者/
  评分/理由/内容快照/图片缩略图,以及**作者历史违规徽章**(累计命中数 + 已拒绝确认违规数)。
- 单条操作:「通过」(保留内容)/「拒绝(删除内容)」(按 kind+ref_id 删业务行)。
- **批量操作**:全选/单选 checkbox + 「批量通过」(单条 `UPDATE … WHERE id IN`)/
  「批量拒绝(删除内容)」(逐条复用 `reject_one`)。
- 拒绝与批量拒绝会删除业务内容,点击后先弹确认框(见 §3)。

## 8. 审核设置(`/admin/moderation/settings`)
- 编辑 `site.json` 的 `moderation` 块:总开关 `enabled`、`llm_review`(还需配置 LLM 环境变量)、
  `on_llm_failure`(`review` 送人工复核 / `reject` 拒绝提交,默认 `review`)、
  阈值 `flag_above` / `block_above`(留空用默认 0.5 / 0.9;须在 0–1 且 `flag_above ≤ block_above`)。
- 保存时服务端先校验,再写回 `site.json` 并 `reload_pipeline()`,立即生效,无需重启。

## 9. 课程权益(`/admin/entitlements`)
- 支付接入前,线下售卖后由管理员手动开通:按用户 ID 授予 / 撤销单门课程权益,或授予 / 续期 / 撤销 Pro 会员。
- 授予课程权益是幂等的,重复授予只刷新来源与时间。

## 10. 不在本期范围
- 软删除 / 操作审计 / 通知。
- 用户禁言、IP 封禁、邀请码。
- 文章发布(博客继续走 `assets/posts/` git 工作流)。
- API Token 与 `/api/v1/...` 公开接口。

## 11. 测试覆盖
`crates/modules/admin/src/server.rs` 提供纯逻辑单测:`clamp_page` / `validate_role` / `check_self_role_change` / 审核设置校验与 site.json 写回。
`crates/modules/admin/src/admin.rs` 提供 `compute_total_pages` 单测。
跑测试:

```sh
cargo test --features server -p module-admin
```
