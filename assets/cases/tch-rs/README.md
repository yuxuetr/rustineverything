# tch-rs
tch-rs 是 LibTorch（PyTorch 的 C++ 库）的 Rust 绑定，让你在 Rust 中直接使用 PyTorch 的张量与自动微分。

## 看点
- 贴近 PyTorch 的 API，迁移成本低
- 可加载 TorchScript 导出的预训练模型
- CPU / CUDA 支持，复用成熟算子库
- 适合「Python 训练、Rust 部署」的混合工作流

## 适合参考
当团队模型在 PyTorch 训练、生产想用 Rust 做推理服务时，tch-rs 是衔接两端最直接的桥。
