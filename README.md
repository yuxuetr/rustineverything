# Rust in Everything

专注 Rust 技术栈的学习与实战的 Dioxus 全栈应用。

## 核心特性

- **插件化架构**：支持动态加载 WASM 插件（身份认证、主题、i18n 等）。
- **全栈 Rust**：基于 Dioxus 0.7 + Axum，支持 Web、Desktop 和 Fullstack Server。
- **现代化 UI**：Tailwind CSS v4 + 深色模式，Rust 主题配色。
- **全站搜索**：Tantivy 嵌入式搜索引擎，支持中英文分词。
- **案例展示**：Rust 项目 Showcase，分类 + 标签 + 论坛讨论。

## 快速开始

```bash
# 1. 安装 Dioxus CLI
cargo install dioxus-cli

# 2. 安装 Tailwind CSS 依赖（首次）
cd crates/app && npm install && cd ../..

# 3. 编译 Tailwind CSS
cd crates/app && npm run build && cd ../..

# 4. 启动开发服务器
dx serve --package app
```

## Tailwind CSS 构建

Tailwind 源文件和 npm 工具链位于 `crates/app/` 下：

- **源文件**: `crates/app/tailwind-input.css`（含 `@import "tailwindcss"` 和 `@source` 指令）
- **输出**: `crates/app/assets/tailwind.css` → 自动反向同步到 `assets/tailwind.css`
- **开发 Watch**: `cd crates/app && npm run dev`
- **一次性构建**: `cd crates/app && npm run build`

注意：本项目使用 **Tailwind CSS v4**，请参考 `crates/app/tailwind.md` 了解 v4 类名变更（如 `bg-linear-*` 代替 `bg-gradient-*`）。

## 文档

- [架构评估总结报告](docs/ARCHITECTURE_ASSESSMENT.md)
- [开发者指南](docs/DEVELOPER.md)
- [认证系统说明](docs/AUTH_SPEC.md)
- [认证配置指南](docs/AUTH_GUIDE.md)
- [课程系统](docs/COURSE_SPEC.md)
- [标注系统](docs/ANNOTATION_SPEC.md)
- [论坛系统](docs/FORUM_SPEC.md)
- [搜索系统](docs/SEARCH_SPEC.md)
- [案例展示](docs/CASE_SPEC.md)
- [Tailwind CSS 指南](docs/TAILWIND_GUIDE.md)

## 开源协议

MIT
