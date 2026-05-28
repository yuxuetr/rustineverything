---
title: "用 clap derive 写子命令式 CLI"
description: "clap 的 derive API：用结构体和枚举声明参数与子命令，让"不合法的调用无法构造"。"
date: "2026-05-19"
subtopic: "args"
tags: ["clap", "args", "cli"]
---

# 用 clap derive 写子命令式 CLI

[clap](https://github.com/clap-rs/clap) 是 Rust 事实标准的命令行解析器。它的 `derive` 模式把"参数定义"和"数据结构"统一成一件事：你声明一个结构体，clap 负责解析、校验、生成帮助和补全。

## derive 的核心思路

把 CLI 的形状写成类型，解析后直接拿到强类型数据：

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "rie", version, about = "Rust in Everything CLI")]
struct Cli {
    /// 输出更详细的日志
    #[arg(short, long)]
    verbose: bool,

    /// 重试次数
    #[arg(long, default_value_t = 3)]
    retries: u32,

    /// 输入文件
    input: std::path::PathBuf,
}

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("reading {}", cli.input.display());
    }
}
```

`Cli::parse()` 在参数非法时自动打印用法并以非零码退出——你拿到的 `Cli` 一定是合法的。这就是"让非法状态不可表示"在 CLI 层面的体现。

## 子命令 = 枚举

子命令天然对应枚举，每个变体带自己的参数：

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 构建项目
    Build {
        #[arg(long)]
        release: bool,
    },
    /// 部署到目标环境
    Deploy {
        /// 目标环境
        target: String,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

fn main() {
    match Cli::parse().command {
        Command::Build { release } => build(release),
        Command::Deploy { target, dry_run } => deploy(&target, dry_run),
    }
}
```

`match` 是穷尽的——加一个新子命令，编译器会逼你处理它。CLI 的分发逻辑因此永远不会漏分支。

## 校验与枚举值

用类型约束取值，而不是运行时 if：

```rust
use clap::ValueEnum;

#[derive(Clone, ValueEnum)]
enum Format { Json, Yaml, Toml }

#[derive(Parser)]
struct Cli {
    #[arg(long, value_enum, default_value_t = Format::Json)]
    format: Format,
}
```

传 `--format xml` 会被 clap 直接拒绝并提示合法取值，无需手写校验。

## 体验细节

- `#[arg(env = "RIE_TOKEN")]`：参数可从环境变量回退，方便 CI 与本地共用。
- `#[command(version)]`：自动接 `--version`，从 `Cargo.toml` 取版本号。
- `clap_complete`：生成 bash/zsh/fish 补全脚本。
- `clap_mangen`：生成 man page。

## 下一步

- 把输出做漂亮：[indicatif](https://github.com/console-rs/indicatif) 进度条 + [console](https://github.com/console-rs/console) 彩色文本。
- 做交互式全屏界面：[ratatui](https://ratatui.rs)。
- 给 CLI 写端到端测试：[assert_cmd](https://github.com/assert-rs/assert_cmd) 直接断言子进程的 stdout / 退出码。

clap derive 的价值不只是少写代码，而是把"参数协议"编码进类型系统——帮助文本、校验、补全全都从同一个真相来源生成，不会随时间漂移。
