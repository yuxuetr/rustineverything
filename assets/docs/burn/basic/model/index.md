# 模型定义

使用 Burn 定义神经网络模型：

```rust
#[derive(Module, Debug)]
struct MLP<B: Backend> {
    linear1: Linear<B>,
    linear2: Linear<B>,
    activation: Relu,
}
```
