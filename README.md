# Rust in Everything

专注 Rust 工业实战与 AI 生态的 Dioxus 全栈内容站：博客、文档、课程、案例、播客与社区一站式聚合。

## 技术栈

- **全栈 Rust**：Dioxus 0.7 fullstack（SSR + 水合）+ Axum，SeaORM + PostgreSQL。
- **UI 组件**：[`dioxus-shadcn`](https://github.com/yuxuetr/dioxus-ui) 0.6（自研，shadcn/ui 的 Dioxus 实现）。
  全站界面基于它重新开发，shadcn token 是唯一的配色来源，暖色（stone）中性色。
- **样式**：Tailwind CSS v4 + 深色模式；内置主题 ocean / sunset / catppuccin，访客可切换。
- **安全**：严格 CSP（无 `'unsafe-eval'`），用户内容经白名单清洗，删除类管理操作先确认。
- **全站搜索**：Tantivy 嵌入式搜索引擎，支持中英文分词。

## 功能

- **内容**（文件系统为单一来源，作者通过 git 写作，`assets/` 下）：
  - 博客：MDX（frontmatter、GFM、数学公式、Mermaid、代码高亮、嵌入组件）。
  - 文档：三级文档树。
  - 课程：课程 → 章节 → 课节，支持免费 / 付费课节、Pro 会员与在线支付（`payments` 特性）。
  - 案例展示、播客，以及 Rust 生态 / AI 生态下的嵌入式、AI、WebAssembly、Web3、CLI 板块。
- **互动**：论坛（话题 / 回复，可从文章发起讨论）、评论、段落标注、图片上传。
- **内容审核**：链接黑名单 + 可选 LLM 审核，可疑内容进入人工复核队列。
- **登录**：GitHub / Google / Discord / X（OAuth2 + PKCE），JWT 会话。
- **管理后台**（`/admin`，仅 admin 可见）：概览、用户与角色、评论、话题、审核队列、审核设置、课程权益。

主题、登录方式、审核都是编译进宿主的普通 Rust 代码，不加载运行时插件。

## 快速开始

```bash
# 1. 安装 Dioxus CLI
cargo install dioxus-cli

# 2. 配置环境变量（DATABASE_URL、JWT_SECRET、BASE_URL、OAuth 凭据等）
cp .env.example .env

# 3. 启动 PostgreSQL（应用启动时自动执行数据库迁移）
docker compose up -d postgres

# 4. 安装并编译 Tailwind CSS（首次，或新增了类名之后）
cd crates/app && npm install && npm run build

# 5. 启动开发服务器（在 crates/app 目录下）
dx serve
```

浏览器访问 `.env` 中 `BASE_URL` 对应的地址（如 `http://127.0.0.1:8080`）。
用 `localhost` 访问会因 OAuth 回调的 cookie 不匹配而登录失败。

首个管理员：用 OAuth 登录一次后，在仓库根目录运行 `set -a; source .env; set +a; scripts/promote_admin.sh <昵称>`（需要 `psql`），
再退出并重新登录，新签发的会话才带 admin 角色。详见 [管理后台](docs/ADMIN_SPEC.md)。

## 提交前检查

```bash
cargo fmt --all
cargo clippy --workspace --features server --all-targets -- -D warnings
cargo test --features server --workspace -- --test-threads=1
cargo check -p app --target wasm32-unknown-unknown --features web
```

## Tailwind CSS 构建

Tailwind 源文件和 npm 工具链位于 `crates/app/` 下：

- **源文件**：`crates/app/tailwind-input.css`（含 `@import "tailwindcss"` 和 `@source` 指令，也扫描 dioxus-shadcn 组件里的类名）
- **输出**：`crates/app/assets/tailwind.css`（不入库，需本地构建）→ 自动反向同步到 `assets/tailwind.css`
- **开发 Watch**：`cd crates/app && npm run dev`
- **一次性构建**：`cd crates/app && npm run build`

详见 [Tailwind CSS 指南](docs/TAILWIND_GUIDE.md)。

## 文档

- [开发者指南](docs/DEVELOPER.md)
- [架构评估总结报告](docs/ARCHITECTURE_ASSESSMENT.md)
- [引擎](docs/ENGINES_SPEC.md) / [模块](docs/MODULE_SPEC.md)
- [dioxus-shadcn 迁移](docs/DIOXUS_UI_MIGRATION.md) / [组件库反馈](docs/DIOXUS_UI_FEEDBACK.md)
- [MDX 内容](docs/MDX_SPEC.md) / [博客](docs/BLOG_SPEC.md) / [文档模块](docs/DOCS_MODULE_SPEC.md) / [课程](docs/COURSE_SPEC.md) / [支付](docs/PAYMENT_SPEC.md)
- [论坛](docs/FORUM_SPEC.md) / [评论](docs/COMMENTS_SPEC.md) / [标注](docs/ANNOTATION_SPEC.md) / [上传](docs/UPLOADS_SPEC.md)
- [内容审核](docs/MODERATION_SPEC.md) / [管理后台](docs/ADMIN_SPEC.md)
- [认证系统](docs/AUTH_SPEC.md) / [认证配置](docs/AUTH_GUIDE.md) / [会话](docs/SESSION_SPEC.md)
- [主题](docs/THEME_SPEC.md) / [搜索](docs/SEARCH_SPEC.md) / [SEO](docs/SEO_SPEC.md)
- [部署](docs/DEPLOY_GUIDE.md) / [运维](docs/OPERATIONS.md) / [安全整改记录](docs/SECURITY_REMEDIATION.md)

## 开源协议

MIT
