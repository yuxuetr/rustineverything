---
title: "no_std 入门：在裸机上运行 Rust"
description: "理解 #![no_std] 到底拿走了什么、留下了什么，以及如何为 MCU 搭起第一个可烧录的固件骨架。"
date: "2026-05-20"
subtopic: "no-std"
tags: ["no_std", "embedded", "core", "alloc"]
---

# no_std 入门：在裸机上运行 Rust

桌面和服务器上的 Rust 默认链接 `std`：它假设底层有操作系统，提供堆分配、线程、文件、网络和 `panic` 时的栈展开。微控制器上没有这些东西——没有 OS，常常只有几十 KB 的 RAM。`#![no_std]` 告诉编译器：**只链接 `core`，不要 `std`。**

## core / alloc / std 三层

- **`core`**：语言的最小内核。`Option`、`Result`、`Iterator`、整数与浮点、`slice`、`fmt`——全部不依赖分配器或操作系统。`no_std` 永远可以用 `core`。
- **`alloc`**：需要一个全局分配器后才能用的部分：`Box`、`Vec`、`String`、`Rc`。很多 MCU 项目**故意不用** `alloc`，改用定长容器（见 `heapless`）以获得确定的内存行为。
- **`std`**：`core` + `alloc` + 操作系统接口。裸机上不可用。

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

`#![no_main]` 表示我们不用标准的 `main` 入口（那需要运行时来调用它）。`#[panic_handler]` 是必须的：`std` 帮你提供的那个没了，你得自己定义 panic 时干什么——这里最简单地死循环。

## 入口点从哪来

裸机程序的真正入口由芯片架构决定（复位向量）。在 Cort-M 上，`cortex-m-rt` 提供 `#[entry]`：

```rust
use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    // 固件永不返回
    loop {
        // 读传感器、翻 GPIO、喂看门狗……
    }
}
```

注意返回类型是 `!`（never）：固件不会"退出"，它一直跑到掉电。

## 目标三元组与构建

为 MCU 构建要指定 target，例如 RP2040（Cortex-M0+）：

```bash
rustup target add thumbv6m-none-eabi
cargo build --target thumbv6m-none-eabi --release
```

`none` 段正是"没有操作系统"的意思。链接脚本（`memory.x`）告诉链接器 Flash 与 RAM 的地址范围，`cortex-m-rt` 会用它布置向量表。

## 不用 alloc 怎么办

没有堆，`Vec`/`String` 就别想了。用 [`heapless`](https://github.com/rust-embedded/heapless)：容量在类型里固定，全部栈上分配。

```rust
use heapless::Vec; // 注意是 heapless 的 Vec

let mut buf: Vec<u8, 64> = Vec::new(); // 最多 64 字节
buf.push(0x42).ok();
```

容量超了 `push` 返回 `Err`，不会偷偷分配，也不会 panic。这种"内存上限写在类型上"的确定性，正是嵌入式想要的。

## 下一步

- 用 [`embassy`](https://embassy.dev) 把轮询循环换成 `async`，让中断驱动的逻辑读起来像顺序代码。
- 用 [`defmt`](https://github.com/knurling-rs/defmt) + `probe-rs` 在没有 `println!` 的世界里做日志和调试。
- 读 *The Embedded Rust Book* 把链接脚本、向量表、HAL 这套串起来。

`no_std` 不是"残废版 Rust"——所有权、`Result`、迭代器、零成本抽象一样不少。你只是把"操作系统替你做的事"重新拿回到自己手里。
