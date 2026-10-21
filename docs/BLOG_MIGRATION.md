# 旧博客迁移：框架就绪评估（2026-10-21）

目标：把旧博客（预计 50–300 篇 Markdown，带图片、多语言代码、标签、旧 URL）导入 `assets/posts/<slug>/index.md`。
方法：读代码 + 本地 dx 实测 + 独立 `serde_yaml` 探针。任务见 Todos.md 阶段 Z。

## 1. 与来源无关、迁移前必须修的

| # | 问题 | 证据 | 后果 |
| --- | --- | --- | --- |
| Z2 | 文章页不显示 frontmatter 的标题与日期 | `app/src/routes/mod.rs:587` 只渲染 `inject_seo` 与正文；示例文章靠正文里重复写 `# 标题` | 正文没有 `# H1` 的文章（多数静态博客如此）页面上没有标题 |
| Z3 | frontmatter 一处不合就整块丢弃，且两套解析器 | `widgets/src/mdx.rs:25` `title` 无默认值；`tags: rust`（字符串）、`keywords: [a, b]` 解析失败；`unwrap_or_default()` 不记日志；列表另有一份 `FrontMatter`（`blog/src/server.rs:89`） | 缺 `title`、标签写成字符串的文章标题 / 描述 / 标签全部丢失 |
| Z3 | 日期按原字符串排序与输出 | `blog/src/server.rs:66` 字符串比较；`feed.rs` `atom_date` 只处理纯日期 | `2019-3-5` 排序错；`2019-03-05 10:20:00` 写进 Atom / sitemap 是非法格式 |
| Z3 | 没有草稿 | `draft: true` 照常发布 | 未完成文章上线 |
| Z4 | 非法 slug 进列表 / sitemap / 搜索但页面 404 | 列表不校验目录名（`server.rs:43`），文章页要求 `is_safe_slug`（只允许 `[A-Za-z0-9_-]`） | 含中文、空格、`.` 的目录名生成死链 |

设计：

- **Z2**：文章页在正文前渲染标题（h1）、日期、标签；正文开头若是与标题相同的 `# 标题` 则去掉，避免重复。
- **Z3**：博客列表改用 `widgets::parse_mdx`，只留一个 `PostMetadata`。`title` 缺省为空；`tags` / `keywords` 接受字符串或列表；`summary` / `excerpt` 作为 `description` 的别名；新增 `draft`、`updated`。解析失败记 warn。日期在解析时规范化：`YYYY-M-D`、`YYYY/MM/DD`、`YYYY-MM-DD HH:MM[:SS]` 归一为 `YYYY-MM-DD`，RFC 3339 原样保留；无法识别的原样保留并记 warn。草稿不进列表 / sitemap / feed / 搜索，文章页 404。
- **Z4**：列表与搜索索引跳过非法 slug 并记 warn；转换脚本负责生成合法 slug。

## 2. 取决于旧博客格式（待确认来源后决定）

- **旧 URL 跳转**：现在只有尾斜杠 308。需要一张「旧路径 → 新 slug」表，在服务端 301。形态取决于旧站的 permalink 规则。
- **原始 HTML**：正文里的 `<div>`、`<img>`、`<br>`、`<!-- more -->` 现在按文本显示（`mdx.rs:217`）。是在转换脚本里改写成 Markdown，还是让站点作者内容放行 HTML，要看旧文章里有多少。
- **TOML（`+++`）frontmatter、Hexo 标签插件（`{% asset_img %}` 等）**：转换脚本改写即可，不改站点。
- **外链图片**：CSP `img-src` 只允许本站与四个头像 CDN，外链图片会被浏览器拦下。建议转换时下载到文章目录；放开 CSP 是另一条路。
- **代码高亮语言**：现有 js、css、html、rust、bash、toml、json、yaml、python。go、ts、sql、dockerfile、c、cpp、java 只显示为无高亮代码。按旧文章实际出现的语言补。

## 3. 不做（当前版本）及重估条件

- **标题锚点改为文字 slug、目录（TOC）**：锚点 `b1`、`b2` 是标注功能的定位依据，改动会让已有标注失效。重估：旧文章里有站内 `#锚点` 链接，即
  `grep -rhoE '\]\(#[^)]+\)' assets/posts | wc -l` 非 0。
- **服务端分页、标签 / 归档页**：sitemap 已收录全部文章，不靠分页页被抓取；列表摘要每篇约 300 字节。重估：迁移后 `/blog` 的 HTML 超过 300KB，即
  `curl -s localhost:8080/blog | wc -c` 大于 300000。
- **feed 全文、feed 条数**：现为全站最近 50 条摘要，够订阅用。重估：读者提出要全文订阅。
- **图片懒加载与宽高**：迁移后看文章页图片数量再定。重估：`grep -rcE '!\[' assets/posts --include='*.md' | awk -F: '$2>10' | wc -l` 非 0（有文章超过 10 张图）。

以上门已核对（2026-10-21）：锚点链接现为 0，构造 `[x](#sec)` 时为 1；`/blog` 现为 50KB；图片门现为 0，构造 11 张图的文章时为 1。

## 4. 进度

§1 的 Z2–Z4 已完成。§2 按来源（Docusaurus）的结论见 §5，任务为 Todos.md 阶段 M。

## 5. 来源：Docusaurus（`yuxuetr.com/data/blog`，2026-10-21 统计）

165 篇（单文件 `YYYY-MM-DD-slug.mdx` 与目录 `YYYY-MM-DD-slug/index.mdx` 混用），旧网址 `/blog/YYYY/MM/DD/<slug>`（与 `build/blog` 的 164 个页面逐一核对；`about` 是页面不是文章）。统计脚本按代码块内外区分。

| 写法 | 数量 | 处理 |
| --- | --- | --- |
| 标题显式 id `## 标题 {#id}` | 222 | M1 站点：开启 heading attributes，用作 `id` |
| 站内锚点链接 `](#引言)` | 251 处 / 23 篇 | M1 站点：无显式 id 的标题按 Docusaurus 规则（github-slugger）由文字生成 `id`；标注仍按 `data-block-id`（`annotations.js` 先找 `data-block-id` 再找 `id`），不受影响。不可信内容（评论 / 论坛）仍用 `bN`，避免与页面元素 id 冲突 |
| 表格里的 `<br/>` | 11 行 | M2 站点：行内 `<br>` / `<br/>` 渲染为换行 |
| `<AudioPlayer src title artist />` | 2 篇 | M3 站点：注册同名组件，渲染 `<audio controls>`（CSP `media-src` 已允许 https）|
| 代码语言 swift 294、jsx 107、tsx 33、ts / typescript 24 | | M4 站点：补 prism 语言包（同版本 1.30.0）。latex 17、markdown 8、clojure 7 等少量的不补 |
| 旧网址 | 164 | M5 站点：`/blog/YYYY/MM/DD/<slug>`（含 `/en` 前缀）308 到 `/blog/<新 slug>`；`/blog/tags/*`、`/blog/page/*`、`/blog/archive`、`/blog/authors*` 到 `/blog`。规则是确定的（新 slug = 旧 slug 中 `.` 换成 `-`），不需要映射表 |
| `import` / `export` 行、`{/* */}` 注释 | 56 / 19 | M6 脚本删除（代码块内不动）|
| `<RedText text=…/>` 等颜色组件 | 312 | M6 脚本改名为站点已有的 `Pink` / `Blue` / `Green` / `Yellow` / `Purple`（属性同为 `text`）|
| 图片：相对路径 102、外链 10 | | M6 脚本把引用到的图片（共享 `images/` 与文章目录内）拷进新文章目录，路径改为文件名；外链图片下载到文章目录，`.mp4` 改成链接；下载失败的列出来 |
| 文章互链 `](../2025-01-06-swiftui-intro/index.mdx)` | | M6 脚本改为 `/blog/<新 slug>` |
| frontmatter | | 保留 title / description / tags / keywords / image；日期取 `date` 或文件名；删 authors / hide_table_of_contents 等 |
| `:::note` 等提示块 | 46 | 站点已支持，不处理 |
| slug 含 `.`（`rust-1.82` 等）| 6 | 新 slug 把 `.` 换成 `-`；无重名 |

脚本 `scripts/migrate_docusaurus.py <源目录> <assets/posts>`：一次性工具，可重复运行（覆盖输出），结束时打印每篇的警告。迁移后跑 §3 的门，并用 curl 检查每篇文章页 200、页内 `href="#x"` 都有对应 `id="x"`、图片都 200。

旧站域名（yuxuetr.com）是否指向新站，决定 M5 的跳转能否被旧链接用到；规则本身与域名无关。
