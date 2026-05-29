# Wasmtime
Wasmtime 是 Bytecode Alliance 维护的 WebAssembly 运行时，把 WASM 带出浏览器，作为服务端的安全沙箱与插件引擎。

## 看点
- 基于 Cranelift 的 JIT / AOT 编译，性能优异
- 完整的 WASI 支持（文件、网络等受控系统接口）
- 内存隔离的沙箱，适合运行不可信代码
- 可作为库嵌入 Rust 宿主，暴露受控 host 函数

## 适合参考
做插件系统、多租户函数执行或不可信代码沙箱时，Wasmtime 与本站的 wasmi 插件运行时同属一类设计——面向更高性能场景。
