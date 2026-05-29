# probe-rs
probe-rs 是一套用 Rust 写的嵌入式调试 / 烧录工具，目标是取代 OpenOCD + GDB 的繁琐配置。

## 看点
- 支持 CMSIS-DAP / J-Link / ST-Link 等常见调试探针
- `cargo embed` / `cargo flash`：像跑普通程序一样烧录固件
- 内建 RTT 日志、断点、内存读写
- 作为库被 VS Code 嵌入式扩展、defmt 生态广泛复用

## 适合参考
当你厌倦了 OpenOCD 脚本、想要 `cargo run` 直接烧到开发板时，probe-rs 是现代化的调试基座。
