# Candle
Candle 是 Hugging Face 的极简 ML 框架，强调小体积二进制与高性能，适合把模型推理塞进 serverless 与边缘环境。

## 看点
- PyTorch 风格的张量 API，CPU / CUDA / Metal 后端
- 内置 LLaMA、Whisper、Stable Diffusion 等模型示例
- 可编译到 WASM，在浏览器里跑推理
- 去掉 Python 运行时，部署即一个静态二进制

## 适合参考
当你想用 Rust 做模型推理服务、追求冷启动速度与低内存占用时，Candle 是 PyTorch 的轻量替代。
