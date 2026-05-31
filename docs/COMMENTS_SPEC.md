# Comments 模块 SPEC

> 范围：`crates/modules/comments` —— 博客详情页底部评论功能，DB 存储 +
> 内容审核 hook。挂在站点 `/blog/:slug` 详情页（前端组件层组装，本 crate
> 仅暴露 server fn）。

## 1. 设计选择

| 维度 | 选择 | 原因 |
| --- | --- | --- |
| 存储 | PostgreSQL `comment` 表（SeaORM） | 关联 `user` 表展示昵称/头像；按 blog_id 索引 |
| 鉴权 | server fn 内 `current_session_user()` 必须存在 | 匿名评论易被滥用，强制登录后写入 |
| 审核 | 写库前过 ModerationEngine | Phase 4：Allow → 直接落库；Flag → 落库 + 入审核队列；Block → 拒绝并返回审核理由 |
| 关联资源 | `blog_id` 是字符串（不限定 blog） | 同一 server fn 可挂到 doc / topic 等任意页面，资源 id 由调用方决定 |
| 排序 | DB 端 `ORDER BY created_at DESC` | 最新评论置顶，免去前端排序 |

## 2. 数据结构

### Comment（前后端共享）

```rust
pub struct Comment {
  pub id: String,                 // i32 → String，前端避免 number precision
  pub blog_id: String,
  pub content: String,
  pub author: String,             // user.nickname；user 已删 → "已注销"
  pub author_avatar: Option<String>,
  pub user_id: Option<i32>,
  pub date: String,               // "YYYY-MM-DD HH:MM" 格式化字符串
}
```

### DB schema（`comment` 表，由 `crates/migration` 管理）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | i32 PK auto | 评论 id |
| `blog_id` | varchar | 资源 id（blog slug / doc path / topic id 等） |
| `user_id` | i32 FK → user.id | 作者；ON DELETE CASCADE |
| `content` | text | 评论正文（已过 XSS sanitize） |
| `created_at` | timestamptz | 创建时间 |

完整迁移见 `crates/migration/src/m20260527_000001_initial_schema.rs`。

## 3. server fn 契约

```rust
#[post("/api/comments/list")]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError>;

#[post("/api/comments/post")]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError>;
```

### `get_comments`

- 无鉴权（评论默认公开可读）。
- SeaORM `find_also_related(user)` 一次 join 拉评论 + 作者信息。
- 用户已注销（FK CASCADE 删除）→ `author = "已注销"`，`author_avatar = None`。
- DB 不可用 → `ServerFnError`。

### `post_comment`

流程：
1. **鉴权**：`current_session_user()` 必须返回 `Some(SessionUser)`，否则拒。
2. **内容审核**：调 `module_moderation::pipeline()` 评估（含 `content`、
   `author.id`、解析出的 `image_urls`）。结果三态：
   - `Allow` → 继续。
   - `Block` → 立即返回 `ServerFnError("评论被审核拒绝：<reason>")`。
   - `Flag` → 写库**并**入审核队列（`enqueue_if_flagged`）。
3. **写库**：插入 `comment` 行，user_id / created_at / content 全设。
4. **返回最新列表**：内部调 `get_comments(blog_id)` 返回该资源完整评论流。

详细审核策略见 [`MODERATION_SPEC.md`](MODERATION_SPEC.md)。

## 4. 路由 + UI

本 crate **不包含 UI 组件**——前端组件 `crates/app/src/components/Comments.rsx`
组装：调 `get_comments` 拉列表 + 输入框调 `post_comment`。

## 5. ModuleEngine 集成

`site.json::modules.comments.enabled = false`：

- `get_comments` / `post_comment` 仍可调用（无前端入口时不会发起）。
- 前端组件层应自行检查 `modules.comments.enabled`（详见 [`MODULE_SPEC.md`](MODULE_SPEC.md)）。

未来若需要严格 server-side gating，可在 server fn 顶部加：

```rust
if !site_config_module_enabled("comments") { return Err(ServerFnError::new("disabled")); }
```

但 Phase 1C 评估认为此处不强制（评论 fn 调用本身已要求登录）。

## 6. XSS 防护

`content` 走 `widgets::sanitize_user_html` 过滤后写库（Phase 4.2）。前端渲染时
不用 `dangerous_inner_html`，而是 plain text + `[link](...)` markdown 子集。

## 7. 性能 / 容量

- 单 `blog_id` 评论数预期 < 1000 量级。
- `comment.blog_id` 有索引，单查询 P95 ≈ 数十 ms（Phase 1A.2 连接池 + Phase 1A 性能基准 `scripts/bench_comments.sh`）。
- 大于 1000 条评论后建议按 `(blog_id, created_at)` 分页（当前未实现）。

## 8. 测试覆盖

```bash
cargo test --features server -p module-comments
```

**`server.rs` 当前无单测**（写库 + 审核 hook + session 三者耦合，纯单测意义有限）。
覆盖通过：

- `module-moderation` 单测覆盖审核结果三态（Allow / Flag / Block）。
- `app-core::session` 单测覆盖 session 解析。
- 端到端：人工触发评论流；live-DB 集成测试见 `app-core::auth::sync_user_to_db_rolls_back...`
  同套路写一份 live `post_comment` 测试是未来工作。

## 9. 不在本期范围

- 评论树（@回复、嵌套层级）—— 当前是平铺列表
- 评论编辑 / 删除（用户自己 + 管理员均无入口）
- Emoji 反应 / 点赞
- 富文本评论（图片 / 代码块 / Markdown 全集）—— 当前只是 plain text + 基础链接
- 反垃圾速率限制（依赖 ModerationEngine 的 LLM 判断，未做基于 IP 的 rate limit）
