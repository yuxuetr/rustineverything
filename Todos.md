# 开发计划

## ✅ 阶段一：用户会话与核心体验闭环（已完成）

- [x] Session / JWT 管理（签发、验证、Cookie 设置）
- [x] `get_current_user()` server function
- [x] 前端全局用户状态（Context 共享 SessionUser）
- [x] Navbar 登录/已登录状态切换（头像、昵称、退出下拉菜单）
- [x] 评论系统迁移到 PostgreSQL（comments 表 + SeaORM entity）
- [x] 评论关联真实用户（头像、昵称显示）
- [x] 未登录用户：可查看评论，需登录才能发表
- [x] Logout 端点（清除 Cookie）

## 阶段二：内容模块补全

### ✅ 2.1 文档系统 `/docs`（已完成）
- [x] `assets/docs/` 按目录组织 Markdown，支持三级嵌套
- [x] server function `list_doc_tree()` 和 `get_doc_content(path)`
- [x] 自动扫描，从 `index.md` 提取标题，无需 `_meta.json`
- [x] frontmatter 支持 SEO（title/description/keywords/image）
- [x] `sidebar_label` / `sidebar_position` 侧栏控制
- [x] `sort_children: asc/desc` 可逆序排序（周报场景）
- [x] 前端布局：/docs 着陆页 + /docs/:path 树形导航 + Markdown 内容
- [x] DocPage 注入 SEO meta 标签
- [x] 15 个单元测试覆盖排序/嵌套/frontmatter 场景

### ✅ 2.2 Podcast 动态化（已完成）
- [x] `assets/podcasts/<slug>/episode.yaml` 元数据格式（id/title/description/duration/date/audio_url/guest/tags）
- [x] `crates/modules/podcast/src/server.rs` 新增 Episode 结构 + scan_episodes
- [x] server function `list_episodes()` 和 `get_episode_by_id(id)`
- [x] PodcastPage 重构为 use_resource 动态加载，加上嘉宾/标签显示
- [x] `<PodcastCard id="...">` MDX 组件重构为独立组件（不再依赖 EPISODES const）
- [x] `/podcasts` 静态路由使音频文件可访问
- [x] audio_url 支持三种格式：绝对路径 / http URL / 相对路径（自动拼接）
- [x] **自动检测音频文件**：YAML 中未填 audio_url 时扫描目录，支持 m4a/mp3/wav/ogg/flac/aac/opus/mpeg
- [x] **零配置模式**：仅需放音频文件，无需 YAML 即可生成节目（title 从文件名推断、id 由 slug 哈希生成、date 从文件 mtime 读取）
- [x] 默认按日期降序排序（同日期 id 大的在前）
- [x] 18 个单元测试覆盖 YAML 解析、排序、URL 处理、音频检测、边界场景

### ✅ 2.3 课程系统 `/courses`（已完成）
- [x] 新建 `crates/modules/course` crate
- [x] Course → Chapter → Lesson 三级模型（`Doc | Video | Audio | Code` 自适应布局）
- [x] `assets/courses/` 约定目录 + `course.yaml` / `_chapter.yaml` / Markdown frontmatter
- [x] LessonKind 推断 + 媒体/代码/附件/图片自动扫描
- [x] `/courses` 列表页 + `/courses/:slug` Hero+手风琴+进度条 + `/courses/:slug/:chapter/:lesson` 课节页
- [x] 数据库表 `course_progress`（lesson 粒度）+ SeaORM Entity + 4 个 server fn（list/get/get_lesson/get_progress/mark_lesson_complete/get_last_lesson）
- [x] Hero "继续学习" + Lesson "完成本节" 按钮 + 完成勾选
- [x] **标注系统 v1**：`annotations` 表 + 4 个 server fn + `assets/js/annotations.js` 运行时（5 色 / 下划线 / 波浪线 / 删除线）+ `site.json` 全局开关
- [x] **标注系统 v2**（PR-D 继续）：
  - [x] Markdown 渲染层注入 `data-block-id`（`crates/modules/blog/src/markdown.rs`）启用可视回放
  - [x] 博客页接入标注 + `site.json.annotations.blog=true`
  - [x] 顶部隐藏/显示标注的眼睛 toggle（右下角浮动按钮 + body.no-anno）
  - [x] `/me/annotations` 个人标注列表页：按资源分组 + #bN 跳转闪烁高亮
  - [x] visibility 面板：工具条加 4 选项（private / course-public / doc-public / public），`list_annotations` 考虑他人公开标注并回填 `author_nickname`
  - [x] `normalize_visibility` 对未知/恶意值兵底为 `private`
  - [x] 标注 DOM 包裹重写：跨已有 span 的选区拆多段逐个 `surroundContents`；create 后仅增量包裹不全量重画（修复多样式交替漏画）
  - [x] 39 个单元测试（24 已有 + 15 标注专项）+ `scripts/test_annotations.sh` 端到端冒烟脚本
  - [x] 4 项边界用例验证（visibility 注入 / 缺省 / 未启用 kind / update 注入）
- [x] 文档：`docs/COURSE_SPEC.md`、`docs/ANNOTATION_SPEC.md`（v2 重写）
- 遗留待实施：跨块选区拆分、孤儿标注修复面板、页面级 frontmatter `annotations: false` 读取

### ✅ 2.4 论坛/话题系统 `/topics`（已完成）
- [x] 新建 `crates/modules/forum` crate（`lib.rs` + `server.rs` + `forum.rs`）
- [x] 数据库表：`topics` / `topic_replies` + SeaORM Entity 与关联索引
- [x] 7 个 server function：`list_topics`/`list_topics_by_ref`/`list_tags`/`get_topic`/`create_topic`/`post_reply`/`list_my_topics`
- [x] 发帖/回复需登录，浏览公开；Markdown 正文与预览
- [x] 路由：`/topics`、`/topics/new`、`/topics/tag/:tag`、`/topics/:id`、`/me/topics`
- [x] **资源引用关联**：`topics.ref_kind/ref_path` + `TopicRef` + Blog/Doc/Lesson 页面底部 `<DiscussionPanel>`
- [x] 资源发起讨论：`/topics/new?ref_kind=&ref_path=` query 自动预填引用卡片 + tag
- [x] tag 规整函数 `normalize_tag` + 18 个单元测试覆盖输入边界
- [x] `scripts/test_forum.sh` 端到端冒烟 + `docs/FORUM_SPEC.md` 设计文档

## 阶段三：高级功能与运营能力

### ✅ 3.1 Admin 后台（已完成，个人站定位）
- [x] `core::session` 下沉 role/session 共享工具：`require_session` / `require_admin` / `is_known_role` / `SessionUser::is_admin`
- [x] 新建 `crates/modules/admin` crate，遵循 forum 风格（`server.rs` + `admin.rs` + `lib.rs`）
- [x] 10 个 admin server fn：overview / users 列表 + 改角色 / comments 列表+删除 / topics 列表+删除 / replies 删除 / plugins 列表+reload
- [x] 5 个后台页面 `/admin`、`/admin/users`、`/admin/comments`、`/admin/topics`、`/admin/plugins` + `AdminShell` 布局
- [x] Navbar 用户下拉菜单：仅 admin 可见 “🛡️ 管理后台”入口；增加“我的话题/标注”
- [x] `scripts/promote_admin.sh`：本地以 nickname 升级 admin
- [x] `docs/ADMIN_SPEC.md`：权限模型/API/页面/数据约束/本地引导
- [x] 纯逻辑单测：`clamp_page` / `validate_role` / `check_self_role_change` / `classify_plugin_kind` / `compute_total_pages` / role 常量
- 遗留待实施：软删除与审计日志、单条回复删除的前端入口、插件热更新

### 3.1 后续扩展（需产品定位确认后启动）
- [ ] 多作者博客发布 (`articles` 表 + 站内 Markdown 编辑器 + 草稿/定时发布)
- [ ] API Token + `/api/v1/...` 公开接口 + 限流
- [ ] Admin 操作审计日志
- [ ] 用户禁言 / 封禁 / 邀请码

### ✅ 3.2 搜索功能（已完成，Tantivy embedded 方案）
- [x] 方案选型：Tantivy 0.26 + tantivy-jieba 0.19。决策记录于 `docs/SEARCH_SPEC.md`
- [x] 新建 `crates/modules/search` crate，5 个模块（text / indexer / engine / server / search）
- [x] 索引源：博客 Markdown、文档 Markdown、论坛 topic（DB）
- [x] Server fn：`/api/search/query` （公开）+ `/api/search/reindex` （admin）
- [x] schema：kind/ref_id/title/body/url/created_at；title boost = 3.0
- [x] 前端 `SearchModal` + `SearchButton` + `Cmd+K`/`Esc` 全局快捷键 + 200ms 防抖
- [x] Navbar 集成搜索按钮，App 根挂载全局模态框
- [x] kind 分类 chip、彩色徽章（BLOG/DOC/TOPIC）、snippet 200 字窗口
- [x] 中文分词：jieba；ASCII 小写 + Unicode 安全
- [x] 全局单例 RAM 索引，Lazy 首查构建，admin 可重建
- [x] 34 个单元测试（text 11 + indexer 5 + engine 13 + server 5）
- [x] `docs/SEARCH_SPEC.md`：选型/Schema/API/UI/生命周期全面记录
- 遗留待实施：typo tolerance、MmapDirectory 持久化、课程/评论/标注索引源、服务端 HTML 高亮

### 3.3 AI 与 Web3 页面
- [ ] `/ai`：Rust AI 生态内容（可复用文档系统为可选方案）
- [ ] `/web3`：区块链教程与案例

### ✅ 3.4 Cases 案例展示（已完成）
参考 Docusaurus showcase 的交互模式，以 “Rust 项目案例库” 为定位：网格卡片 + 一级分类筛选 + 标签侧栏过滤 + 顶部关键词搜索 + “提交你的项目” 入口。

#### 产品范围与资产约定
- [x] 定位：个人站策划案例集，内容走 git/PR，文件系统作为 SoT（与博客/文档一致）
- [x] 项目存放于 `assets/cases/<slug>/`，含：
  - `case.yaml`（必须）
  - `cover.png` / `cover.jpg` / `cover.webp`（可选，缺省渲染渐变占位 + 首字母）
  - `README.md`（可选，详情页渲染）
- [x] 温柔替换现有 `Cases` 占位页（`crates/app/src/routes/mod.rs`）
- [x] 不引入数据库表：MVP 仅文件系统；stars 手填，后续可加同步脚本

#### `case.yaml` Schema
- [x] 字段：`name`/`slug`（可选，缺省为目录名）/`description`/`category`（`frontend|backend|fullstack|cli|embedded|ai|web3|library|tool|desktop`）/`tags: [String]`/`repo`/`website`（可选）/`author`/`author_url`（可选）/`language`（`rust|wasm|mixed`）/`stars: i64`（可选，缺省 0）/`favorite: bool`（默认 false）/`date_added: YYYY-MM-DD`
- [x] 预定义 tag 表（记于 `docs/CASE_SPEC.md`）：`axum/actix/dioxus/tauri/leptos/tokio/sea-orm/wasm/cli/embedded/web3/ai/fullstack/library/opensource/commercial/favorite`
- [x] tag 规整函数 `normalize_tag`（记于后端）：小写 + ASCII alphanumeric + `-_`、去重
- [x] 未知 tag 不报错，但记录为“其他”分组供贡献者发现

#### 模块划分 `crates/modules/cases/`
- [x] 与 forum/admin/search 风格一致：`Cargo.toml` + `src/{lib.rs, server.rs, cases.rs, text.rs}`
- [x] `text.rs`（纯逻辑，可跨前后端）：`normalize_tag`、`matches_query`（在 name+description+tags 中不区分大小写包含匹配）、`compare_cases`（favorite 优先 → stars desc → date_added desc → name asc）
- [x] `server.rs`：
  - `Case` / `CaseSummary` / `TagSummary` DTO
  - `list_cases(tags: Option<Vec<String>>, category: Option<String>, q: Option<String>) -> Vec<CaseSummary>`：服务端过滤 + 排序
  - `list_case_tags() -> Vec<TagSummary>`：tag 与其计数
  - `get_case(slug) -> Option<Case>`：含完整元数据 + `readme_md`、`cover_url`
  - YAML 用 `serde_yaml` 解析，错误不会阻塞其他项目（记录 warning 则跳过）
- [x] `cases.rs`：Dioxus 页面组件
  - `CasesIndexPage`：布局 = 左侧 tag 侧栏多选 + 右侧网格；顶部“提交你的项目”与关键词输入
  - `CaseCard`：封面缩略图 + name + description + category + tag chips + repo/website 外链 + stars
  - `CaseDetailPage`：Hero(封面 + 名称 + tag) + Markdown(README) + 外链按钮 + DiscussionPanel（复用论坛 `ref_kind="case"`）
- [x] 样例项目至少 2 个（如 `axum-realworld` 与 `dioxus-fullstack-template`） 作为默认种子

#### 路由与静态服务
- [x] `crates/app/src/routes/mod.rs::Route` 调整：
  - SPA 路由采用单数 `/case`，指向 `CasesIndexPage`（覆盖占位页）
  - 新增 `/case/:slug` `CaseDetail { slug: String }`
- [x] `crates/app/src/main.rs` 补上 `ServeDir`：
  - 静态资源采用 `/cases`，复刻课程模块“单数 SPA `/course` + 复数静态 `/courses`”约定，避免路由冲突
  - 选定后在 `Route::CaseDetail` `cover_url` 中拼接正确前缀
- [x] 未提供封面时生成渐变占位 + 首字母（避免破图与额外请求）
- [x] 封面优先级：`cover.webp` > `cover.jpg` > `cover.png`；文档建议封面 ≤ 200KB、宽度 ≤ 1200px，优先 WebP 以保证响应速度

#### 与论坛/搜索联动
- [x] 详情页底部插入 `<DiscussionPanel resource_kind="case" resource_path=slug>`（必要时在 forum `ref_link_for` 中补 `case` 映射）
- [x] `crates/modules/search/src/indexer.rs::collect_documents` 增加 `kind="case"` 源：
  - title = name
  - body = description + tags + readme 不超 4000 字
  - url = `/case/<slug>`
  - kind 徽章颜色添到前端 HitRow `match` 分支
- [x] `search.rs::normalize_kind` 加入 `case`

#### 贡献入口
- [x] `.github/ISSUE_TEMPLATE/add-case.yml`（GitHub Issue Form）提供表单：name/repo/website/description/tags/language/cover
- [x] `提交你的项目` 按钮跳转 URL：`https://github.com/54yyyu/rustineverything.app/issues/new?template=add-case.yml`
- [x] `docs/CASE_SPEC.md` 里接入 PR 指南：提交 yaml + cover 到 `assets/cases/<slug>/`

#### 权限与 Admin
- [x] 本期不补 admin 管理面板（文件系统内容由维护者走 git）
- [ ] 下期可选：`admin_list_cases` + favorite 切换（改写 yaml）

#### 测试
- [x] `text` 纯逻辑：`normalize_tag` 边界、`matches_query`（大小写/部分匹配/中文）、`compare_cases` 排序优先级
- [x] `server` 集成测试（使用 tempfile 写 `case.yaml` 后调用 `list_cases`）：OR 标签过滤、搜索、favorite 置顶
- [x] yaml 错误项不会打断其他 cases 加载
- [x] 目标：总计 12+ 个单测

#### 文档与脚本
- [x] `docs/CASE_SPEC.md`：Schema/贡献指南/tag 表/路由决策/排序规则
- [x] `scripts/test_cases.sh`（可选）：本地冒烟脚本，运行 cases/search 自动测试验证样本项目可读
- [x] `Todos.md` 勾选 3.4 完成

#### 验证门禁
- [x] `cargo test --features server -p rustineverything-module-cases`【新增】
- [x] `cargo test --features server -p rustineverything-module-search`【验证 case kind 集成】
- [x] `cargo build --features server -p rustineverything-app`
- [x] 本地手动：访问 `/case` 看到网格与例子、点击详情页检查外链与讨论面板、⌘K 搜索能命中 case（已通过 API + 页面状态码验证）

#### 不在本期范围
- GitHub stars 自动同步
- 站内提交表单（走 admin 后台写 yaml）
- 页面静态截图自动生成、OG image 抓取
- i18n 双语描述字段
- 分页与虚拟滚动（当 case 超过 50 个后补）

#### 判断点（已确认）
- [x] 静态资源路径 vs SPA 路由：采用 “`/case` SPA + `/cases` 静态”
- [x] 启用 `case` 资源 kind 接入 forum 讨论（需修改 forum/server.rs `resolve_ref_title` 加一个分支）
- [x] 同时处理 `cover.webp` / `.jpg` / `.png` 三种格式，优先 WebP 并控制图片体积
- [x] 案例库不局限于前端站点，增加 `category` 一级分类覆盖 frontend/backend/fullstack/cli/embedded/ai/web3/library/tool/desktop

---

## 下一步建议
推荐进入 **阶段三**（Admin/搜索/案例展示）任意一模块：
- **3.1 Admin** 依赖 role 中间件；论坛/评论/文章都已上线，是运营必需
- **3.2 搜索** 可为论坛/文档/博客/课程提供入口，补齐资源发现能力
- **3.4 Cases** 在论坛发酷间可被引用，补齐产品走向展示

或者补齐 **2.3 / 2.4 遗留**：
- 论坛：删除/编辑/置顶/点赞；标注接入 `topic` 资源 kind；被回复的通知
- 课程标注：跨块选区拆分、孤儿标注修复面板
