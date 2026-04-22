# Rust in Everything

专注 Rust 技术栈的学习与实战的 Dioxus 跨端应用。

## 核心特性

- **插件化架构**：支持动态加载 WASM 插件（身份认证、主题、i18n 等）。
- **多端支持**：基于 Dioxus 0.6，支持 Web、Desktop 和 Server。
- **现代化 UI**：Vanilla CSS 驱动的深色模式优先设计。
- **全栈集成**：集成 Axum 后端、PKCE OAuth 2.0 流程。

## 快速开始

1. 安装 Dioxus CLI: `cargo install dioxus-cli`
2. 运行开发服务器: `dx serve`
3. 构建 Release: `dx build --release`

## 文档

- [开发者指南](docs/DEVELOPER.md)
- [认证系统说明](docs/AUTH_SPEC.md)
- [认证配置指南](docs/AUTH_GUIDE.md)

## 开源协议

MIT
