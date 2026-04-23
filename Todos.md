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

### 2.1 文档系统 `/docs`
- [ ] `assets/docs/` 下按目录组织 Markdown 文件
- [ ] server function `list_doc_tree()` 和 `get_doc_content(path)`
- [ ] 前端：左侧树形导航 + 右侧 Markdown 内容（复用 blog 的 Markdown 组件）

### 2.2 Podcast 动态化
- [ ] 将 `const EPISODES` 迁移到 `assets/podcasts/`（YAML/JSON 元数据 + 音频文件）
- [ ] server function `list_episodes()` 和 `get_episode(id)`

### 2.3 课程系统 `/courses`
- [ ] 新建 `crates/modules/course` crate
- [ ] 定义 Course 数据模型（title, description, chapters, progress）
- [ ] 章节列表展开、进度追踪（需登录）

### 2.4 论坛/话题系统 `/topics`
- [ ] 新建 `crates/modules/forum` crate
- [ ] 数据库表：topics, replies
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
- [ ] `/ai`：Rust AI 生态内容
- [ ] `/web3`：区块链教程与案例

### 3.4 Cases 案例展示
- [ ] 真实 Rust 项目案例展示
- [ ] GitHub 仓库嵌入和代码片段展示
