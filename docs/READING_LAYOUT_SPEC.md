# 阅读页布局：左侧跨页导航，右侧页内目录（阶段 T，2026-10-22）

文档、博客文章、课程小节三类阅读页的侧栏规则，以及承载它们的组件放在哪里。

## 1. 现状与问题

| 页面 | 左 | 右 | 问题 |
|---|---|---|---|
| 文档 `/docs/*` | 文档树（`hidden lg:block`） | 无 | 手机上没有任何导航 |
| 博客 `/blog/:id` | 无 | 无 | 长文没有页内目录（标题 id 已由 M1 生成） |
| 课节 `/course/:slug/:chapter/:lesson` | 无，只有「← 返回课程目录」 | 有代码 / 下载时才有 | 看不到自己在课程里的位置；没有上一节 / 下一节；站内链接是 `a { href }`、`PageMeta` 嵌在 SuspenseBoundary 里——与 S5 修过的文档页同样会整页刷新 |

另外 cases、forum、about 也把 `PageMeta` 嵌在 SuspenseBoundary 里，可能有 S5 发现的 hydration 编号错位。

## 2. 规则

**位置按作用分，不按板块分：**

- **左侧 = 跨页导航**：「我在这本书 / 这门课的哪里」。只有按顺序阅读的内容才有：文档、课程。
- **右侧 = 页内目录**：「我在这一页的哪里」。正文有 3 个及以上 `##`/`###` 标题时才显示，`xl`（≥1280px）以上显示。
- **手机**：左侧导航收进抽屉，由正文上方的「目录」按钮打开；页内目录在手机上不显示（正文本身就是一列）。

| 页面 | 左 | 右 |
|---|---|---|
| 文档 | 文档树 | 页内目录 |
| 博客 | 无（文章之间没有阅读顺序） | 页内目录 |
| 课节 | 课程大纲：章 → 节，当前节高亮，✅ 已完成，🔒 付费未试看 | 页内目录，下面接代码 / 下载 |

## 3. 组件与放置

| 组件 | 位置 | 理由 |
|---|---|---|
| `Tree` / `TreeItem`（WAI-ARIA tree，键盘导航） | dioxus-shadcn 0.6.6（已有） | 通用；文档树和课程大纲都用它 |
| `SidebarProvider` / `Sidebar { off_canvas }` / `SidebarTrigger` | dioxus-shadcn（已有） | 同一份导航在桌面是 `aside`、在手机是模态抽屉（焦点陷阱、Esc、遮罩），DOM 里只有一份 |
| `widgets::heading_outline(markdown)` | 本仓库 `crates/widgets/src/mdx.rs` | 依赖本站的 Markdown 管线（frontmatter、`:::` 转换、M1 标题 id），id 必须与正文完全一致 |
| `widgets::OnThisPage { entries }` | 本仓库 `crates/widgets` | 三个使用方（文档、博客、课节）；滚动高亮 |
| 路由驱动的树（当前路径 → 选中项、祖先展开、选中即导航） | 本仓库 `crates/widgets` | 两个使用方（文档树、课程大纲），否则两边各写一份同样的胶水 |

**不放进 dioxus-ui 的原因**：上面本仓库的部分都依赖站点数据（Markdown 管线、`DocTreeNode`、课程进度与权限）；放进去意味着每次调布局都要发 dioxus-ui 新版本再升级本站。
**重估条件**：`OnThisPage` 的渲染外壳（输入 `(level, id, text)` 列表、滚动高亮）不依赖站点数据。等第二个项目需要它，再提到 dioxus-ui。本阶段不改 dioxus-ui；用的过程中发现它的问题，按惯例记到 `docs/DIOXUS_UI_FEEDBACK.md`。

## 4. 数据流

- **页内目录**：`heading_outline` 是纯函数，服务端和客户端用同一份正文各算一次，结果一致，不新增 server fn，也不会造成 hydration 不一致。与 `Markdown` 组件共用「frontmatter → `:::` 转换 → 解析 → `assign_heading_ids`」这一段，不另写一套。只取 h2、h3。
- **滚动高亮**：`document::eval` 里用 IntersectionObserver 观察正文标题，把当前标题 id `send` 回 Rust，高亮由 signal 渲染（不由 JS 改 Dioxus 管着的节点，S6 的教训）。换页时旧 observer 先断开。点击目录项是普通 `#id` 锚点；标题加 `scroll-margin-top`，不被 sticky 顶栏挡住。
- **课程大纲**：课节页额外用 `use_server_future(get_course)` 取课程结构（SSR 里就有大纲链接）；进度和已购沿用课程详情页的 `use_resource`（每用户数据，不进 SSR）。上一节 / 下一节由课程结构在客户端算出，不改 server fn。
- **路由驱动的树**：`Tree` 用受控的 `selected`（= 当前路径）和 `expanded`（含当前路径的所有祖先；换页时补进新祖先，用户手动展开 / 折叠也保留）。树项文字是 `Link`（爬虫与中键新开页都需要真实 `href`）；键盘 Enter 选中时由 `on_selected_change` 导航，路径与当前路由相同则不重复 push。

## 5. 风险与验证

- **Tree 用于站点导航**：WAI-ARIA APG 有 navigation treeview 示例，可用；但点击行时 `Link` 与行的 onclick 都会触发。T1 先在课程大纲上实测：点击、Enter、后退各一次，历史记录里不应有重复项。若行为不对，退回到纯 `nav > ul > li > Link` 列表，并把问题记进 DIOXUS_UI_FEEDBACK。
- **off-canvas 侧栏在手机上的 SSR**：`Sidebar` 在 hydration 前按桌面渲染（上游测试写明 "renders as the desktop aside until the page answers"），手机上会先看到一个挤在正文旁的侧栏。用只在非 modal 状态生效的 `max-md:hidden` 类遮住，实测手机宽度首屏不出现侧栏。
- **每项都要在浏览器里验**：SPA 换页不刷新（window 标记保留）、无 hydration 报错、SSR HTML 里有导航链接、手机宽度无横向滚动。

## 6. 任务

见 `Todos.md` 阶段 T：T0 升级 dioxus-shadcn 0.6.6；T1 课节页；T2 页内目录；T3 手机抽屉 + 文档树换成 `Tree`；T4 其余页面的 `PageMeta`。
