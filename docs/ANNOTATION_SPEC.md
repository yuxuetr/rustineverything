# 标注系统说明 (Annotations)
## 适用范围
对三类资源在**叶子页粒度**启用标注：

| `resource_kind` | `resource_path` 形态 | 示例 |
|---|---|---|
| `blog`   | `<blog_id>` | `welcome` |
| `doc`    | 完整叶子路径 | `axum/basic/router` |
| `course` | `<slug>/<chapter>/<lesson>` | `rust-basics/01-fundamentals/01-what-is-rust` |

跨页/跨块标注**不支持**。同一资源内标注按字符偏移定位。
## 双层开关
- **全局** — `assets/site.json`：
  ```json
  "annotations": { "course": true, "doc": true, "blog": true }
  ```
  关闭的 kind：`list_annotations` 直接返回 `[]`，`create_annotation` 抛 "当前资源未启用标注"。
- **页面级**（预留语法，待启用）— Markdown frontmatter `annotations: true|false`，优先级高于全局。

`get_annotations_config()` 暴露 `{ course, doc, blog }` 给前端，`AnnotationLayer` 据此决定是否渲染。
## 数据模型
表 `annotations`（PostgreSQL，DDL 在 `init.sql`）：

| 列 | 类型 | 说明 |
|---|---|---|
| `id` | `BIGSERIAL` | 主键 |
| `user_id` | `INTEGER` FK `users(id)` | 作者 |
| `resource_kind` | `VARCHAR(32)` | `course` / `doc` / `blog` |
| `resource_path` | `TEXT` | 叶子页路径（见上表） |
| `block_id` | `VARCHAR(64)` | Markdown 顶层块 id（`b1`、`b2` …） |
| `start_offset` / `end_offset` | `INTEGER` | 该块内字符偏移（基于 `textContent`）|
| `exact_text` | `TEXT` | 选中文本快照（重定位/孤儿恢复用） |
| `prefix_text` / `suffix_text` | `TEXT` | 前/后 32 字符上下文 |
| `style` | `VARCHAR(32)` | `yellow|green|blue|pink|purple|underline|wavy|strikethrough` |
| `note` | `TEXT` | 选填备注 |
| `visibility` | `VARCHAR(16)` | `private` \| `course-public` \| `doc-public` \| `public` |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | 默认 `NOW()` |

索引：`(resource_kind, resource_path)` 与 `(user_id)`。

字段设计借鉴 [W3C Web Annotation](https://www.w3.org/TR/annotation-model/) 的
`TextQuoteSelector` + `TextPositionSelector` 双锚定：偏移 + 文本快照 + 前后缀，便于
在正文修改后做尽力重定位。
## 锚点策略
Markdown 渲染层（`crates/modules/blog/src/markdown.rs`）为顶层块
（`Paragraph` / `Heading` / `List` / `BlockQuote` / `CodeBlock`）注入 `id="bN"` 与
`data-block-id="bN"`。前端选区时找最近祖先 `data-block-id` 得到稳定锚点。

- 偏移：以**该块的 `textContent` 字符位置**为基准（`TreeWalker.SHOW_TEXT` 累计）
- **跨块选区拒绝**（`captureSelection` 检测 `startBlock !== endBlock` 即 return null）
- 跨已有 `<span class="rie-anno">` 的选区：通过 `collectSegments` 拆成多段逐个 `surroundContents`，避免单调用跨节点报错
## Server Functions
位置：`crates/modules/course/src/server.rs`（共享给三类资源）。

| 端点 | 入参 | 鉴权 |
|---|---|---|
| `POST /api/annotations/config` | — | 公开 |
| `POST /api/annotations/list` | `resource_kind, resource_path` | 公开（自人 + 他人非 private）|
| `POST /api/annotations/list_my` | — | 登录（仅本人）|
| `POST /api/annotations/create` | `payload: AnnotationCreate` | `member` / `admin` |
| `POST /api/annotations/update` | `id, style?, note?, visibility?` | 仅作者 |
| `POST /api/annotations/delete` | `id` | 仅作者 |

写入路径统一过 `current_session_user()` 解 cookie，再过 `require_writer()` 限角色。
### `list_annotations` 可见性合并
同一资源路径下：
- **本人**所有标注（任意 visibility）
- **他人**所有 visibility ≠ `private` 的标注

返回结果对他人标注会**回填 `author_nickname`**（一次批量 IN 查询，避免 N+1）。
未登录可看他人公开标注。
### `normalize_visibility` 兜底
所有写路径（create / update）经过：
```rust path=null start=null
match v.unwrap_or("private") {
    "public" => "public",
    "course-public" => "course-public",
    "doc-public" => "doc-public",
    _ => "private",
}
```
未知/恶意值（含 SQL 关键字、大小写错配等）一律落 `private`，配合 SeaORM 参数化查询双重防护。
## 前端运行时
脚本：`crates/app/assets/js/annotations.js`（在 `main.rs` 通过 `<script src="/js/annotations.js">` 全局加载，幂等防重入）。
### 暴露接口
```js path=null start=null
window.RIE_ANNO = {
  apply({ kind, path, items }), // 全量重画（unwrap-all + rewrap-all）
  captureSelection(),           // 捕获当前选区
  isVisible(), setVisible(v), toggleVisible(),  // body.no-anno 视图层切换
  flashTargetFromHash(),        // 跳转闪烁目标块
};
```
### 渲染流
1. `AnnotationLayer` 组件挂载时调 `list_annotations` 拉取列表 → `RIE_ANNO.apply(data)` 全量重画。
2. 用户 `mouseup` → `captureSelection()` → 弹工具条（5 色 + 3 样式按钮 + visibility `<select>`）→ `create()` 走 fetch。
3. **增量包裹**：`create()` 成功后只调 `applyOne(item)` 把新条目包上 span，不重画其他标注（解决多样式交替时漏画 bug）；`appliedSet` 全局去重保证同 id 不重复包。
4. 跨已有 span 的范围由 `wrapRange()` 拆成多段，每段 `surroundContents` 自成一个新 span（共享 `data-anno-id`）。
### 视觉
- CSS 全部在 JS 里以 `<style id="rie-anno-styles">` 注入，避免 Tailwind 预编译漏类
- 5 种背景色 + 3 种装饰（`underline` / `underline wavy` / `line-through`）
- 他人公开标注：额外加 `.rie-anno-by-other`（虚线 outline）+ `title="作者: ..."` 鼠标悬停
- 浮动眼睛按钮（`AnnotationToggle` 组件，inline style 固定右下角）切换 `body.no-anno` 类，CSS 把所有 `.rie-anno` 的背景与装饰透明化（仅视图层、不动数据）
- `localStorage` 持久化：`rie-anno-visible`（隐藏开关）、`rie-anno-last-visibility`（工具条上次可见性选择）
- `#bN` URL hash 跳转时闪烁目标块（`.rie-anno-flash` 关键帧动画 1.6s × 2）
## 个人标注列表 `/me/annotations`
组件：`MyAnnotationsPage`（`crates/modules/course/src/course.rs`）。

- `list_my_annotations()` 拉取当前用户全部标注（按 `created_at desc`）
- 客户端 `group_annotations()` 按 `(resource_kind, resource_path)` 分组，组内保留输入序
- 每条展示：色板小点 + 文本快照（line-clamp-3）+ 备注 + 创建时间 + 样式名 + `#bN` + 可见性徽标 + 作者
- "跳转 →" 链接由 `build_jump_url(kind, path, block_id)` 拼成：
  - `course` → `/course/<path>#<block_id>`
  - `doc` → `/docs/<path>#<block_id>`
  - `blog` → `/blog/<path>#<block_id>`
- 着陆后由 `annotations.js` 的 `flashTargetFromHash()` 滚动并闪烁
## 测试
### 服务端单元测试（39 项）
```sh path=null start=null
cargo test -p rustineverything-module-course --features server -- --test-threads=1
```
覆盖：
- `parse_order_prefix` / `humanize_title` / `lang_from_ext`（课程扫描）
- `rewrite_image_urls`（含 UTF-8 边界保护）
- `infer_lesson_kind` 5 个分支
- `scan_attachments` / `scan_code_files` / `read_lesson` / `scan_courses`
- **标注专项**：`normalize_visibility` 已知值 + 兜底；`default_annotation_enabled`；`build_jump_url` per kind / 空 block / 未知 kind；`kind_badge` / `visibility_label` / `style_swatch_class` 全部样式；`group_annotations` 输入顺序保持 / 同 path 不同 kind 分离 / 空输入
### 端到端冒烟测试
脚本：`scripts/test_annotations.sh`

```sh path=null start=null
# 浏览器登录后从 DevTools 复制 session cookie
RIE_COOKIE='session=eyJ...' bash scripts/test_annotations.sh
```

7 个步骤：
1. baseline list = 0
2. 连续创建 5 条 (style × visibility) 组合
3. list 数量 = baseline + 5；逐条校验 (style, visibility) 全部入库
4. list_my 至少包含本次 5 条
5. update style + visibility 双字段，再 list 读回校验
6. delete 一条，list 数量 −1
7. cleanup 恢复 baseline
### 边界用例（已验证 4/4）
- visibility=`hacker-attempt` → DB 落 `private` ✓
- 缺省 visibility 字段 → DB 落 `private` ✓
- `kind=ai`（site.json 未启用） → 500 "当前资源未启用标注" ✓
- update 注入 `visibility=DROP TABLE` → DB 落 `private` ✓
## Roadmap
- 跨块选区拆分（v1 直接拒绝）
- 孤儿标注修复面板（正文大改后失配的统一管理）
- 页面级 frontmatter `annotations: false` 真正读取并禁用
- 公开标注的举报/标注互动
- 标注列表搜索 / 筛选 / 排序（按样式、按时间、按可见性）
