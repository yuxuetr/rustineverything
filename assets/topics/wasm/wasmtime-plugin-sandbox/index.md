---
title: "用 wasmtime 把 WASM 做成插件沙箱"
description: "为什么 wasm 是理想的插件运行时，以及用 wasmtime 在宿主里安全加载、调用不受信代码的最小骨架。"
date: "2026-05-21"
subtopic: "runtimes"
tags: ["wasmtime", "wasi", "plugins", "sandbox"]
---

# 用 wasmtime 把 WASM 做成插件沙箱

让用户上传代码并在你的服务里运行，传统方案要么开容器（重），要么裸跑（不安全）。WebAssembly 给了第三条路：**默认无能力的沙箱**。模块除了你显式注入的函数，什么都碰不到——没有文件、没有网络、没有系统调用。[wasmtime](https://wasmtime.dev) 是这条路上最成熟的运行时。

## 为什么 wasm 适合插件

- **能力安全**：模块只能调用宿主显式提供的导入函数。不给文件 API，它就读不了文件。
- **确定的资源边界**：可设内存上限、燃料（fuel）上限，防失控插件拖垮宿主。
- **跨语言**：插件可以用任何能编译到 wasm 的语言写，宿主只认 ABI。
- **可移植**：同一份 `.wasm` 在 Linux / macOS / ARM 上行为一致。

本站的主题 / i18n / 审核插件系统正是基于这个思路（用的是更轻量的 wasmi，原理相同）。

## 最小宿主

```rust
use wasmtime::{Engine, Module, Store, Linker};

let engine = Engine::default();
let module = Module::from_file(&engine, "plugin.wasm")?;

let mut linker = Linker::new(&engine);
// 注入一个宿主函数，插件可以 import 它
linker.func_wrap("host", "log", |caller: Caller<'_, ()>, ptr: i32, len: i32| {
    // 从插件线性内存读字符串……
    println!("plugin says: ...");
})?;

let mut store = Store::new(&engine, ());
let instance = linker.instantiate(&mut store, &module)?;

// 调用插件导出的函数
let run = instance.get_typed_func::<(i32, i32), i32>(&mut store, "run")?;
let result = run.call(&mut store, (input_ptr, input_len))?;
```

关键点：插件能做的全部由 `linker` 里注入的导入决定。没注入 `host::read_file`，插件就没有读文件的途径——这是**结构性**的安全，不靠运行时检查。

## 设资源上限

防止恶意 / 失控插件：

```rust
let mut config = Config::new();
config.consume_fuel(true);
let engine = Engine::new(&config)?;

let mut store = Store::new(&engine, ());
store.set_fuel(10_000_000)?;        // 燃料耗尽 → 中断执行
store.limiter(|_| &mut MyLimiter);  // 限制线性内存增长上限
```

燃料机制把"执行多少条指令"变成可计费、可中断的资源，死循环插件会被强制停下，而不是挂死宿主线程。

## 内存与数据传递

wasm 的类型只有数字，复杂数据靠**线性内存 + (指针, 长度)** 约定传递：

1. 宿主调用插件导出的 `alloc(len)` 拿到一段内存指针；
2. 把输入字节写进插件内存；
3. 调用目标函数，传 `(ptr, len)`；
4. 函数返回 `(ptr, len)`（常打包进一个 `u64`），宿主从插件内存读回结果。

组件模型（[wit-bindgen](https://github.com/bytecodealliance/wit-bindgen) + WIT）正是为了把这套手工 ABI 自动化：你写一份 `.wit` 接口，绑定代码自动生成，告别手搓指针。

## 何时选 wasmtime vs wasmi

- **wasmtime**：JIT、高性能、WASI 完整、组件模型——服务端、需要吞吐时首选。
- **wasmi**：纯解释执行、体积小、易嵌入、`no_std` 友好——插件系统、嵌入式、对二进制体积敏感时更合适。

两者 ABI 思路一致，迁移成本低。先用 wasmi 把插件 ABI 跑通，需要性能时再换 wasmtime。
