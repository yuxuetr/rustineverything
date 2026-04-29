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
- [x] **标注系统**：`annotations` 表 + 4 个 server fn + `assets/js/annotations.js` 运行时（5 色 / 下划线 / 波浪线 / 删除线）+ `site.json` 全局开关
- [x] 文档：`docs/COURSE_SPEC.md`、`docs/ANNOTATION_SPEC.md`
- 遗留待实施：Markdown 渲染层注入 `data-block-id`（启用标注可视回放）、标注 visibility 选择 UI、孤儿标注修复面板

### 2.4 论坛/话题系统 `/topics`
- [ ] 新建 `crates/modules/forum` crate
- [ ] 数据库表：`topics`(id, title, tag, content, user_id, created_at), `replies`
- [ ] 发帖和回复需登录，浏览公开
- [ ] 按 tag 分类 `/topics/:tag`

## 阶段三：高级功能与运营能力

### 3.1 Admin 后台
- [ ] role 权限校验中间件
- [ ] Admin 面板：文章管理、评论管理、用户管理
- [ ] 插件管理：查看/热更新 WASM 插件

### 3.2 搜索功能
- [ ] 全站内容搜索（博客、文档、论坛）
- [ ] 方案选型：tantivy-wasm 或 PostgreSQL 全文检索

### 3.3 AI 与 Web3 页面
- [ ] `/ai`：Rust AI 生态内容（可复用文档系统为可选方案）
- [ ] `/web3`：区块链教程与案例

### 3.4 Cases 案例展示
- [ ] 真实 Rust 项目案例展示
- [ ] GitHub 仓库嵌入和代码片段展示

---

## 下一步建议
推荐进入 **2.3 课程系统**：
- 进入需要数据库交互的阶段（记录学习进度）
- 与 2.1 文档系统可复用扫描 + frontmatter 思路（课程元数据从目录生成）
- 需要为进度追踪新建 `course_progress` 数据表。

或者 **2.4 论坛系统**：
- 与 2.3 复杂度相当，数据库驱动。
- 需要使用阶段一的会话体系。
