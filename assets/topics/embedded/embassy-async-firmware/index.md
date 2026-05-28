---
title: "用 Embassy 写异步固件"
description: "为什么 async/await 适合中断驱动的固件，以及一个 Embassy 任务、Spawner 与 Timer 的最小例子。"
date: "2026-05-24"
subtopic: "embassy"
tags: ["embassy", "async", "no_std", "concurrency"]
---

# 用 Embassy 写异步固件

传统固件是一个大 `loop`：轮询外设、维护状态机、手写定时。逻辑一复杂，状态机就爆炸。[Embassy](https://embassy.dev) 把 Rust 的 `async/await` 带到裸机上——**没有操作系统、没有堆**，却能把"等一个中断"写成一次 `.await`。

## 为什么 async 适合 MCU

固件本质上全是"等待"：等 DMA 完成、等定时器、等 GPIO 边沿、等 UART 收满。同步写法要么忙等浪费电，要么手搓回调状态机。`async` 把这些等待点变成挂起点：任务 `.await` 时让出 CPU，事件到了再被唤醒。

Embassy 的执行器**不需要分配器**：任务是编译期已知大小的状态机，静态存放。等待时核心可以进低功耗 `wfi`，对电池设备至关重要。

## 一个最小例子

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::task]
async fn blink(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let led = Output::new(p.PIN_25, Level::Low);
    spawner.spawn(blink(led)).unwrap();
}
```

`Timer::after(...).await` 不是忙等——执行器把这个任务挂起、安排一个硬件定时器中断，然后让核心休眠。500ms 后中断唤醒任务，从 `.await` 之后接着跑。同样的核心可以同时跑十几个这样的任务而互不阻塞。

## 多任务与共享状态

`Spawner` 可以 spawn 多个任务并发跑：

```rust
spawner.spawn(blink(led)).unwrap();
spawner.spawn(read_sensor(i2c)).unwrap();
spawner.spawn(handle_uart(uart)).unwrap();
```

任务间共享数据用 Embassy 的同步原语（都是 `no_std` 友好的）：

- `Signal<T>`：一个任务发、一个任务收的最新值。
- `Channel<T, N>`：定长 MPSC 队列。
- `Mutex`：async 互斥锁，持锁时仍可让出。

```rust
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;

static TEMP: Signal<ThreadModeRawMutex, f32> = Signal::new();

// 生产者
TEMP.signal(reading);

// 消费者
let value = TEMP.wait().await;
```

## 与 HAL 的关系

Embassy 为主流芯片提供了自带 async HAL：`embassy-rp`（RP2040）、`embassy-stm32`、`embassy-nrf`。它们的外设 API 直接返回 future——比如 `i2c.read(addr, &mut buf).await` 在 DMA 期间挂起，而不是阻塞轮询。

## 心智模型

把 Embassy 想成"给固件用的、零分配的 tokio"：

1. 写任务，用 `.await` 标出等待点；
2. `Spawner` 把任务交给执行器；
3. 没有就绪任务时，核心睡觉省电；
4. 中断把对应任务标记为就绪、唤醒执行器。

结果是：中断驱动的并发，读起来却像顺序代码——状态机的复杂度交给编译器去生成。这正是 Rust 在嵌入式领域最有说服力的地方之一。
