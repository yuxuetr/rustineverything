# Embassy
Embassy 把 Rust 的 `async/await` 带进嵌入式裸机环境：无需 RTOS，用 executor + 异步 HAL 写出零成本的并发固件。

## 看点
- `async` executor 运行在 Cortex-M / RISC-V / ESP32 等目标上，无动态分配
- 统一的异步 HAL：GPIO / I2C / SPI / UART / USB / 网络
- 一等公民的低功耗：`await` 空闲时自动进入睡眠
- `embassy-net` 提供异步 TCP/IP 协议栈

## 适合参考
当固件需要并发处理多个外设、又想避免手写中断状态机时，Embassy 的 async 模型是现代替代方案。
