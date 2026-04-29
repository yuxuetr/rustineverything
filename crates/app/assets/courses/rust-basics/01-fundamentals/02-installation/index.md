---
title: 安装与环境
description: 安装 rustup，并完成第一次构建。
duration: "8:00"
---

# 安装与开发环境

最佳安装方式是使用 `rustup`，它统一管理工具链：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

安装完成后验证版本：

```bash
rustc --version
cargo --version
```

接着创建一个新项目：

```bash
cargo new hello-rust
cd hello-rust
cargo run
```

右侧栏的代码 Tab 提供完整的 `Cargo.toml` 与 `main.rs` 示例，可直接复制或下载本地编辑。
