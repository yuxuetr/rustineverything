---
title: "用 candle 在本地跑 LLM 推理"
description: "candle 的设计取舍、加载 safetensors 权重做前向推理，以及为什么纯 Rust 推理栈对部署友好。"
date: "2026-05-22"
subtopic: "llm"
tags: ["candle", "llm", "inference", "safetensors"]
---

# 用 candle 在本地跑 LLM 推理

[candle](https://github.com/huggingface/candle) 是 HuggingFace 出的极简张量库。和 PyTorch 不同，它的目标是**轻量、可静态部署**：没有 Python 运行时，编译出的二进制可以直接丢到服务器或边缘设备上。

## 为什么是 Rust

LLM 推理的部署痛点是依赖：PyTorch + CUDA + Python 环境动辄几个 GB，冷启动慢。candle 把这些压成一个 Rust 二进制：

- **小**：无 Python，无巨型运行时；
- **快冷启动**：进程起来就能服务，适合 serverless；
- **多后端**：同一份代码可选 CPU、CUDA、Metal。

## 张量基础

candle 的核心是 `Tensor` 和 `Device`：

```rust
use candle_core::{Device, Tensor};

let device = Device::cuda_if_available(0)?;
let a = Tensor::randn(0f32, 1.0, (2, 3), &device)?;
let b = Tensor::randn(0f32, 1.0, (3, 4), &device)?;
let c = a.matmul(&b)?; // (2, 4)
println!("{c}");
```

注意每个算子都返回 `Result`——形状不匹配、设备不一致会在运行时报错而不是 panic，符合 Rust 的错误处理习惯。

## 加载权重做推理

模型权重一般是 `.safetensors`（[safetensors](https://github.com/huggingface/safetensors) 格式：零拷贝、内存映射、无任意代码执行风险）。典型流程：

```rust
use candle_core::Device;
use candle_nn::VarBuilder;

let device = Device::Cpu;
// mmap 权重文件，不全量读进内存
let vb = unsafe {
    VarBuilder::from_mmaped_safetensors(&["model.safetensors"], DType::F32, &device)?
};

// 用 VarBuilder 构造模型结构（各层从 vb 里按名字取权重）
let model = MyModel::load(&vb, &config)?;

// 前向
let logits = model.forward(&input_ids)?;
```

`candle-transformers` 已经内置了 Llama、Mistral、Phi、BERT 等常见结构，多数情况下你不用自己写模型层，直接用现成实现 + HuggingFace Hub 上的权重。

## 文本生成循环

自回归生成就是"前向 → 采样 → 追加 → 再前向"：

```rust
let mut tokens = tokenizer.encode(prompt)?.get_ids().to_vec();
for _ in 0..max_new_tokens {
    let input = Tensor::new(&tokens[..], &device)?.unsqueeze(0)?;
    let logits = model.forward(&input, tokens.len() - 1)?;
    let next = logits_processor.sample(&logits)?; // top-p / temperature
    tokens.push(next);
    if next == eos_token { break; }
}
let text = tokenizer.decode(&tokens, true)?;
```

生产实现会加 **KV cache**（缓存历史 token 的注意力键值），把每步从 O(序列长度) 降到 O(1)，candle-transformers 的模型已经内置。

## 何时该用 candle

- ✅ 想把推理打包成单个二进制部署，不想背 Python 环境；
- ✅ 边缘 / serverless / 冷启动敏感场景；
- ✅ 已经用 Rust 写服务端，想把推理收进同一个进程。
- ⚠️ 训练 / 快速实验仍然是 PyTorch 生态更顺手；candle 更偏推理与部署。

想要更完整的训练框架，可以看 [burn](https://burn.dev)——它提供了自动微分、优化器和更"框架化"的 API。
