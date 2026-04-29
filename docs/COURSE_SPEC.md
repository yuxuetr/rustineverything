# 课程系统说明 (Course)

## 路由
- `/courses` 课程列表（`CoursesIndexPage`）
- `/courses/:slug` 课程详情（`CourseDetailPage`，Hero + 章节手风琴 + 进度）
- `/courses/:slug/:chapter/:lesson` 课节页（`LessonPage`，按 `LessonKind` 自适应布局）

## 三级模型
**Course → Chapter → Lesson**。Lesson 仅以**目录形式**存在，类型为 `Doc | Video | Audio | Code` 之一。

## 目录约定
```
assets/courses/
  rust-basics/                        # ← Course
    course.yaml                       # 课程元数据（可选）
    cover.png                         # 自动探测：cover.{png,jpg,webp}
    01-fundamentals/                  # ← Chapter（数字前缀决定 order）
      _chapter.yaml                   # 章节元数据（可选；title/description/order）
      01-what-is-rust/                # ← Lesson（必须是目录）
        index.md                      # Doc 主正文（带 frontmatter）
        audio.mp3                     # 主/辅音频（自动探测）
        video.mp4                     # 主/辅视频（自动探测）
        code/                         # 代码示例（多文件 Tab；只读高亮）
          hello.rs
          Cargo.toml
        attachments/                  # 可下载文件（整目录列出）
          slides.pdf
        images/                       # 图片资源（也可与 index.md 同级）
          diagram.png
```

### 元数据
- `course.yaml`（可选）：`title, description, cover, tags, level, order`
- `_chapter.yaml`（可选）：`title, description, order`
- Lesson `index.md` frontmatter：`title, description, audio_url?, video_url?, duration?, sidebar_position?`

### 排序
- 数字前缀（`^\d+[-_]?`）→ 决定顺序；无前缀按字母序兜底
- `_chapter.yaml.order` / 课程 `course.yaml.order` 可覆盖

## Lesson 类型推断（递进优先级）
1. 有 `index.md` / `index.mdx` → `kind=Doc`
2. 否则有 `*.mp4|*.webm|*.mov` → `kind=Video`
3. 否则有音频文件（mp3/m4a/wav/ogg/flac/aac/opus）→ `kind=Audio`
4. 否则 `code/` 非空 或目录中有代码白名单文件（rs/toml/ts/js/py/sh/yaml/json/sql/...）→ `kind=Code`
5. 都没有 → 跳过该 lesson 目录

> 无论 `kind` 为何，所有资源都会被附挂到 `Lesson` 对象上：例如 Video Lesson 也可携带 `index.md` 作为讲解笔记，Doc Lesson 也能挂主音频/视频。

## 展示策略（按 LessonKind 自适应）
- **Doc**：主区 = Markdown 正文 + 标注层；顶部 sticky 紧凑音频条；可折叠视频块；右侧栏 = 代码 Tab + 下载
- **Video**：顶部 16:9 视频；下方 `index.md` 笔记/字幕；右侧栏 = 代码 Tab + 下载
- **Audio**：顶部音频卡片；下方 `index.md` 转写/笔记；右侧栏 = 下载
- **Code**：主区 = 代码 Tab 切换器（多文件，只读高亮，复制 + 下载）；下方 `index.md` 题解；右侧栏 = 下载

## 静态资源
全部走 `nest_service("/courses", ...)`。下载链接、媒体 URL、代码 raw 都是 `/courses/<slug>/<chapter>/<lesson>/<file>`。
绝对 `/...` 与 `http(s)://...` 在 frontmatter 与 Markdown 中原样保留。

## Server Functions
位于 `crates/modules/course/src/server.rs`：
- `list_courses() -> Vec<CourseSummary>`
- `get_course(slug) -> Option<Course>`
- `get_lesson(slug, chapter, lesson) -> Option<Lesson>`
- `get_progress(slug) -> Vec<LessonProgress>`（未登录返回空）
- `mark_lesson_complete(slug, lesson_path, completed)`（仅 `member`/`admin`）
- `get_last_lesson(slug) -> Option<String>`（用于 Hero "继续学习"）
- 标注相关 server fns 见 `ANNOTATION_SPEC.md`

## 数据库
新增表：
- `course_progress`（lesson 粒度，`PRIMARY KEY (user_id, course_slug, lesson_path)`）

详见 `init.sql`。SeaORM Entity：`crates/core/src/entities/course_progress.rs`。

## 权限
- 课程**全员可看**
- 进度记录与标注**需登录**；仅 `member` 与 `admin` 可写
- 不引入 paywall（Roadmap 项）

## Roadmap
- 代码 Lesson 在线运行（沙箱）
- 单文件 Lesson 形式
- 学习数据洞察（基于 `course_progress`）
- 课程付费 / 权限分级
