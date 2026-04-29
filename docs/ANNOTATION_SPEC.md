# 标注系统说明 (Annotations)

## 范围
适用于三类资源（叶子页粒度）：
- `blog`  → `<blog_id>`（如 `welcome`）
- `doc`   → 完整叶子路径（如 `axum/basic/router`）
- `course`→ `<slug>/<chapter>/<lesson>`（如 `rust-basics/01-fundamentals/01-what-is-rust`）

## 双层开关
- **全局**：`assets/site.json` 中
  ```json
  "annotations": { "course": true, "doc": true, "blog": false }
  ```
- **页面级**：Markdown frontmatter `annotations: true|false`（v1 暂未读取，预留语法）

`get_annotations_config()` 暴露给前端，关闭的 kind 上 `list/create` 直接返回空或 403。

## 数据模型
表 `annotations`（PostgreSQL）：

| 列 | 类型 | 说明 |
|----|------|------|
| id | BIGSERIAL | 主键 |
| user_id | INT | 作者 |
| resource_kind | VARCHAR(32) | `course` / `doc` / `blog` |
| resource_path | TEXT | 叶子页路径 |
| block_id | VARCHAR(64) | Markdown 顶层块 id（如 `b3`） |
| start_offset / end_offset | INT | 块内字符偏移 |
| exact_text | TEXT | 选中文本快照（失配回退用） |
| prefix_text / suffix_text | TEXT | 前/后 32 字符上下文 |
| style | VARCHAR(32) | `yellow|green|blue|pink|purple|underline|wavy|strikethrough` |
| note | TEXT | 选填备注 |
| visibility | VARCHAR(16) | v1 仅 `private`；预留 `course-public`/`doc-public`/`public` |
| created_at / updated_at | TIMESTAMPTZ | |

字段设计借鉴 W3C Web Annotation 的 TextQuoteSelector + TextPositionSelector。索引：
`(resource_kind, resource_path)` 与 `(user_id)`。

## 锚点策略
正文渲染时为顶层块（Paragraph / Heading / List / BlockQuote / CodeBlock）加 `data-block-id="b{N}"`。
偏移以**该块的 textContent 字符位置**为基准。跨块选区**拒绝**。

> v1 在 `crates/modules/blog/src/markdown.rs` 尚未注入 `data-block-id`；客户端运行时 JS 会跳过没有锚点的条目，作为优雅降级。后续 PR 在 Markdown 渲染流的 `Tag::Paragraph / Heading / List / BlockQuote / CodeBlock` 上注入即可启用全部能力。

## 前端运行时
- `assets/js/annotations.js`（全局加载）
- 暴露 `window.RIE_ANNO.apply({ kind, path, items })`：根据 `block_id + offset` 在 DOM 上包裹 `<span class="rie-anno rie-anno-yellow|green|...">`
- 鼠标 `mouseup` 自动捕获选区，弹出 5 色 + 3 样式（下划线/波浪线/删除线）小工具条 → POST `/api/annotations/create`
- 重新拉取后 `apply()` 把所有标注渲染上去

CSS 注入（5 色 + 3 样式）由 JS 自动完成，不需要全局 CSS。

## Server Functions
`crates/modules/course/src/server.rs`（共享给三类资源）：
- `get_annotations_config() -> AnnotationsConfig`
- `list_annotations(resource_kind, resource_path) -> Vec<Annotation>`（仅返回登录用户自己的）
- `create_annotation(payload) -> Annotation`（需登录；仅 `member`/`admin`）
- `update_annotation(id, style?, note?) -> Annotation`（仅作者）
- `delete_annotation(id) -> ()`（仅作者）

写入路径都通过 `current_session_user()` 鉴权；`require_writer()` 限制为 `admin`/`member`。

## 可见性
v1 默认 `private`（仅作者）。`visibility` 字段已入库，UI 暂不暴露其他选项。后期渐进开放：
`private` → `course-public` / `doc-public` → `public`。

## 局限性 / Roadmap
- v1 未在 Markdown 渲染层注入 `data-block-id`；标注创建可成功，但页面加载时如无锚点会跳过显示（待补）
- 跨块选区拒绝（不拆分）
- 无"孤儿标注"管理 UI（正文大改后失配的统一管理面板）
- 无 `visibility` 选择 UI
- blog/doc 默认开关在 `site.json` 中：`{ course: true, doc: true, blog: false }`

## 测试
- 服务端：`cargo test -p rustineverything-module-course --features server`
- 前端：在 `dx serve` 后选择含 `data-block-id` 的段落 → 选色 → 刷新页面应保持高亮（block-id 注入到位后）
