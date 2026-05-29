# 论坛 / 话题系统（2.4）

## 设计目标

1. 任何访客可以浏览话题列表与详情；发帖与回复必须登录。
2. 单帖多回复，按 tag 分类聚合，支持 Markdown 富文本。
3. 复用阶段一的会话体系（`SessionUser` + Cookie/JWT）与评论模块的鉴权 / 数据范式。
4. 与博客 / 文档 / 课节资源建立直接引用关联：从源页面发起讨论，话题详情显示原文卡片。

## 数据模型

### `topics` 表

| 字段            | 类型              | 说明                                                |
|-----------------|-------------------|-----------------------------------------------------|
| id              | SERIAL PK         |                                                     |
| title           | VARCHAR(255)      | 话题标题                                            |
| tag             | VARCHAR(64)       | 主标签（V1 仅 1 个）                                |
| content         | TEXT              | 正文（Markdown）                                    |
| user_id         | INTEGER FK users  | ON DELETE CASCADE                                   |
| reply_count     | INTEGER DEFAULT 0 | 由 server 在事务内维护                              |
| last_reply_at   | TIMESTAMPTZ NULL  | 最近一次回复时间                                    |
| ref_kind        | VARCHAR(32) NULL  | `blog` / `doc` / `course` / `lesson` 之一           |
| ref_path        | TEXT NULL         | 资源叶子路径，与 ref_kind 同步                      |
| created_at      | TIMESTAMPTZ       |                                                     |
| updated_at      | TIMESTAMPTZ       |                                                     |

索引：`(tag)`、`(user_id)`、`(last_reply_at DESC NULLS LAST, created_at DESC)`、`(ref_kind, ref_path)`。

### `topic_replies` 表

| 字段       | 类型               | 说明 |
|------------|--------------------|------|
| id         | SERIAL PK          |      |
| topic_id   | INTEGER FK topics  | ON DELETE CASCADE |
| user_id    | INTEGER FK users   | ON DELETE CASCADE |
| content    | TEXT               | Markdown |
| created_at | TIMESTAMPTZ        |      |

索引：`(topic_id, created_at)`。

## API（Server functions）

所有写接口前都会调用 `current_session_user().ok_or("请先登录")?`。

| Method+Path                       | 入参                                              | 返回                       | 鉴权 |
|-----------------------------------|---------------------------------------------------|----------------------------|------|
| POST `/api/topics/list`           | `tag: Option<String>, page: Option<u32>`          | `Vec<TopicSummary>`        | 公开 |
| POST `/api/topics/list-by-ref`    | `kind: String, path: String`                      | `Vec<TopicSummary>`        | 公开 |
| POST `/api/topics/tags`           | -                                                 | `Vec<TagSummary>`          | 公开 |
| POST `/api/topics/get`            | `id: i32`                                         | `Option<TopicDetail>`      | 公开 |
| POST `/api/topics/create`         | `input: NewTopicInput`                            | `TopicSummary`             | 登录 |
| POST `/api/topics/reply`          | `topic_id: i32, content: String`                  | `TopicDetail`              | 登录 |
| POST `/api/topics/mine`           | -                                                 | `Vec<TopicSummary>`        | 登录 |

### 共享类型

```rust path=null start=null
pub struct TopicRef { pub kind: String, pub path: String, pub title: String }

pub struct TopicSummary {
    pub id: i32, pub title: String, pub tag: String,
    pub author: String, pub author_avatar: Option<String>, pub user_id: i32,
    pub reply_count: i32, pub last_reply_at: Option<String>, pub created_at: String,
    pub reference: Option<TopicRef>,
}

pub struct TopicDetail {
    pub id: i32, pub title: String, pub tag: String, pub content: String,
    pub author: String, pub author_avatar: Option<String>, pub user_id: i32,
    pub created_at: String, pub updated_at: String,
    pub reference: Option<TopicRef>, pub replies: Vec<Reply>,
}

pub struct NewTopicInput {
    pub title: String, pub tag: String, pub content: String,
    pub ref_kind: Option<String>, pub ref_path: Option<String>,
}
```

### 校验规则

由纯函数 `validate_new_topic` / `validate_new_reply` / `normalize_tag` 实现，单元测试覆盖：

- `title.trim()` 非空且 ≤ 255 字符
- `normalize_tag(tag)` 后非空（仅保留 `[a-z0-9_-]`，超长截断到 64）
- `content.trim()` 非空且 ≤ 64KB
- 引用约束：`ref_kind` 与 `ref_path` 必须同时存在或同时缺省；`ref_kind ∈ {blog, doc, course, lesson}`
- 回复内容非空且 ≤ 32KB

### 引用标题解析

`resolve_ref_title(kind, path)` 在服务端按 kind 分发：

- `blog`：扫描 `assets/posts/<path>/index.{md,mdx}` frontmatter `title:`
- `doc`：扫描 `assets/docs/<path>/index.{md,mdx}` frontmatter / 一级 `# `
- `course`：解析 `assets/courses/<path>/course.yaml` 的 `title:`
- `lesson`：path 必须形如 `<slug>/<chapter>/<lesson>`，扫描该 lesson 目录的 `index.md`
- 任何失败兜底使用 path 字符串本身

## 前端路由（在 `crates/app/src/routes/mod.rs`）

| 路径                  | 组件                | 说明                                       |
|-----------------------|---------------------|--------------------------------------------|
| `/topics`             | `TopicsIndexPage`   | 最新话题流 + tag 云                        |
| `/topics/new`         | `NewTopicPage`      | 创建话题（识别 `?ref_kind=&ref_path=`）   |
| `/topics/tag/:tag`    | `TopicsByTagPage`   | 按 tag 过滤的话题列表                      |
| `/topics/:id`         | `TopicDetailPage`   | 话题详情 + 引用卡片 + 回复列表 + 回复编辑器 |
| `/me/topics`          | `MyTopicsPage`      | 当前用户发布的话题                         |

> 注意：因 `:id` 是 `i32`，与静态 `/topics/new`、`/topics/tag/:tag` 的优先级冲突由 Dioxus 路由按声明顺序解决，所以将静态路径写在前面。

## 资源页讨论嵌入

- `Blog`、`DocPage`、`Lesson` 三个页面在底部插入 `<DiscussionPanel resource_kind path />`。
- 该组件调用 `list_topics_by_ref` 显示已有讨论，并提供「发起讨论」按钮跳到 `/topics/new?ref_kind=...&ref_path=...`。
- `NewTopicPage` 会读取 query string 自动渲染引用卡片，并预填 tag 为 `from-<kind>`。

## tag 规范

- 客户端输入任意大小写、空格、Unicode；服务端 `normalize_tag` 自动小写化、过滤为 `[a-z0-9_-]`、最长 64 字符。
- 完全规整后为空字符串则视为非法，要求重填。
- 自动补全：`/topics/new` 的标签输入框带 `<datalist>`，列出现有 tags（来自 `/api/topics/tags`）。

## 测试

- `crates/modules/forum/src/server.rs` 内置 18 个单元测试覆盖 `validate_new_topic` 边界、`validate_new_reply`、`normalize_tag`、`extract_title_line`。
  ```
  cargo test -p module-forum --features server
  ```
- `scripts/test_forum.sh`：端到端冒烟（创建 / 引用 / 反查 / 回复 / 个人列表），需要 `RIE_COOKIE` 环境变量。

## 未来扩展（不在 V1 范围）

- 删除 / 编辑 / 置顶 / 锁帖
- 点赞 / 收藏 / @通知
- 标注系统接入 `topic` 资源 kind
- 全文搜索（属于阶段 3.2）
- 一帖多 tag、富 tag 元数据（颜色 / 图标）
- 锚点级引用：将某条标注一键转成话题
- 反向计数缓存：在源资源 frontmatter / yaml 旁记录已有讨论数
