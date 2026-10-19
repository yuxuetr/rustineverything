# 页面与板块优化方案（阶段 O，2026-10-19）

> 来源：2026-10-19 对本地站点（`dx serve` + `rie-postgres` 容器）逐页走查（桌面 1280 / 手机 390）
> 与代码审计。任务清单见 `Todos.md` 「阶段 O」。

## 1. 问题证据

| # | 现象 | 证据 |
|---|------|------|
| E1 | 除博客文章外，所有页面 `<title>` 都是站名，无 description / og / canonical | curl `/`、`/ai`、`/case`、`/course`、`/podcast`、`/topics`、`/docs` 均为 `<title>Rust in Everything`；板块文章页丢弃 frontmatter：`modules/ai/src/ai.rs:233` `let (_meta, _body) = parse_mdx(..)` |
| E2 | sitemap / feed 只收博客 + 5 板块；无页面声明 feed | `app/src/server/seo.rs:15-50`；全仓无 `rel="alternate"` |
| E3 | 板块文章不存在时 200 + 红色报错框（博客 / 文档已改为 404） | `modules/ai/src/server.rs:135` 返回 `Err("文章未找到")`，`ai.rs:238` 渲染 |
| E4 | 板块页内链用 `a { href }`，点击整页刷新 | `ai.rs:173`（卡片）、`ai.rs:204`（返回） |
| E5 | 5 个板块 UI 文件除模块名外逐字相同 | `ai.rs` 与 `cli.rs` 替换模块名后 diff 仅剩替换误伤的行；E1/E3/E4 的修复要改 5 遍 |
| E6 | 播客 3 期全部无法播放 | 3 个 `episode.yaml` 的 `audio_url` 指向同一个文件；`build.rs:75` 不镜像 `audio/`，`/audio/…` 在 dev 404；一期 `guest: 张三`，日期均为 2024-01 |
| E7 | 播客列表 `use_resource`，SSR HTML 无节目 | `modules/podcast/src/podcast.rs:74`；内容来自 `assets/`，与 B6 的「论坛例外」理由不符 |
| E8 | DB 失败被显示成「空」 | 论坛 `list_topics(..).await.unwrap_or_default()`（`forum.rs:259-260`、`home_sections.rs:323`）：DB 未起时显示「还没有话题，第一个发表的就是你！」，标签栏停在「加载中…」 |
| E9 | 案例卡片封面全是占位大字母 | `assets/cases/*/cover.*` 0 个（17 个案例）；`cases/src/server.rs:169` 支持 cover 但没有素材 |
| E10 | 板块子分类筛选远多于文章 | 每板块 6 个 chip，文章 1–2 篇（`assets/topics/*`） |
| E11 | 导航：桌面无「播客」；手机站名被截成「R…」 | `layouts/classic.rs:118-134` 无播客；`classic.rs:113` `max-w-32 truncate` 在 390px 下只剩一个字母 |
| E12 | 页脚：标语重复两次；无板块入口、无 RSS | 走查截图；`classic.rs:293-302` |
| E13 | 首页「按领域浏览」11 张卡与上方「Rust / AI 生态」两卡、导航内容重叠 | 首页走查 |
| E14 | 搜索弹窗全中文硬编码；筛选只有 blog/doc/topic/case，板块已索引却不能筛 | `modules/search/src/search.rs:119,131,138,208`；`indexer.rs:252` |
| E15 | 其余硬编码文案 | `routes/mod.rs:563`、`docs.rs:239`、`course.rs:207`、`podcast.rs:176,186,295,306,340,345` |

## 2. 方案

### 2.1 板块共享组件（解决 E3/E4/E5 的「改 5 遍」）

Phase 6 选择了 5 个独立 crate，以便各板块分头演进。5 个月后 UI 仍逐字相同，没有出现分歧。
**不合并 crate**，只把逐字相同的两个页面组件提到 `widgets`：

- `widgets::board::{BoardIndex, BoardArticle}`：参数是板块 id、文案、`SUBTOPICS`、`FEATURED_CRATES`，
  以及列表 / 详情两个 server fn 的结果。
- 各板块 crate 保留 `server.rs`（数据）与 `lib.rs` 常量，`<board>.rs` 缩为几行调用。
- 某个板块以后需要分歧时，在它自己的 crate 里写页面，不改共享组件。

**重估条件**：某个板块的 `<board>.rs` 不再调用共享组件，或有 2 个以上板块需要给共享组件加开关时，重新评估。

### 2.2 页面元信息（E1/E2）

- 文章页（5 板块）：用 frontmatter 走现有 `inject_seo`，与博客一致。
- 列表 / 落地页：新增一个最小的 `widgets::PageMeta { title, description }`，输出 title、description、og:title、og:description 和 canonical。
  它服务约 10 个调用点，不再做更多抽象。
- sitemap / feed 由 `collect_content_entries` 补全：docs 页面、案例详情、课程详情。
  课时属于付费内容，不收；播客没有详情页，不收。
- 布局 `<head>` 加 `<link rel="alternate" type="application/rss+xml" href="/feed.xml">`，页脚加 RSS 链接。

### 2.3 加载与错误状态（E7/E8）

- 论坛保持 B6 结论（`use_resource`），只修「错误显示成空」：不再 `unwrap_or_default()`，`Err` 时显示「加载失败，请稍后重试」，空列表才显示空状态。
  范围是论坛列表、标签栏和首页「论坛热帖」。
- 播客改为 `use_server_future`，理由同 B6 对内容页的结论（数据来自 `assets/`，属于 SEO 内容）。
- 文档侧栏、案例筛选项暂不迁移：它们是导航 / 筛选，SSR 时正文已经在 HTML 里。
  **重估条件**：搜索引擎收录报告中出现「文档页内链缺失」，或禁用 JS 时文档页无法跳转到其他章节。

### 2.4 内容（E6/E9/E10，需作者决定的部分见 §4）

- 播客：每期的 `audio_url` 必须指向各自存在的文件。修复 dev 下 `/audio` 404：静态挂载改为指向根 `assets/audio`，或在 build 时软链。
  先确认生产 Dockerfile 的行为再选方案。
- 案例卡：没有 cover 时不渲染封面区（卡片变矮），不再画占位字母。有 `cover.*` 的案例照常显示。
- 板块子分类 chip：只显示有文章的子分类，「全部」始终保留。

### 2.5 导航与首页（E11/E12/E13）

- 桌面导航加入「播客」（受模块开关控制，与手机抽屉一致）。
- 手机站名：390px 下显示完整站名或 Logo 字标，不再截成一个字母。
- 页脚：删除底部重复的标语；新增「专题」列（5 个板块，受开关控制）；社区列加 RSS。
- 首页：删除「按领域浏览」区块（入口已由生态卡、导航和页脚覆盖）。见 §4 D3。

### 2.6 搜索与 i18n（E14/E15）

- 搜索弹窗文案接入 `t()`；筛选 chip 补上 5 个板块。
- 课程（标题 + 简介，不含课时正文）进索引；播客等有详情页后再进索引。
- E15 列出的文案接入 i18n，中英文都补齐。

## 3. 暂不做（当前版本）

| 项 | 理由 | 重估条件（可执行） |
|----|------|------------------|
| 文章 TOC / 上下篇 / 相关文章 | 每个板块只有 1–2 篇，上下篇和相关文章都会是空的 | `for b in ai cli embedded wasm web3; do [ $(ls assets/topics/$b \| wc -l) -ge 5 ] && echo $b; done` 有输出 |
| 板块文章评论 | 博客评论区目前没有评论数据，先观察 | `SELECT count(*) FROM comments` > 20 |
| 播客详情页 | 3 期，列表页内即可切换播放 | `ls assets/podcasts \| wc -l` ≥ 10 |
| 面包屑 | 层级最深只有 3 层，已有返回链接 | 出现 4 层路由 |
| 列表分页（板块） | 文章数远小于一页 | 任一板块文章数 ≥ 20 |

## 4. 需要作者决定

- **D1 薄内容板块**：保持现状（只做 chip 收敛），还是在文章数为 1 的板块挂「持续更新中」提示？内容本身需要作者补写。
- **D2 播客**：「张三」与日期是占位还是真实数据？3 期对应的音频文件分别是什么？没有音频的期是删除还是下线？
- **D3 首页「按领域浏览」**：删除（推荐），还是保留并删掉上方的两张生态卡？
- **D4 案例封面**：只做「无封面不渲染」（推荐），还是另外制作 17 张封面？
