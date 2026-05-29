# RTIC
RTIC（Real-Time Interrupt-driven Concurrency）用硬件中断控制器直接调度任务，提供可证明无数据竞争的实时并发。

## 看点
- 任务即中断：调度交给 NVIC 硬件优先级，开销极低
- 编译期保证的资源共享（基于 Stack Resource Policy），无锁无死锁
- 无堆分配、无运行时，适合最严苛的实时约束
- 可与 `async` 任务混合使用

## 适合参考
当固件有硬实时截止期、要在中断之间安全共享状态时，RTIC 的并发模型比手写临界区更可靠。
