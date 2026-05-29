# Yew
Yew 是较早成熟的 Rust 前端框架，用组件 + 虚拟 DOM 的模式（类 React）在浏览器里运行 WASM。

## 看点
- `html!` 宏写 JSX 风格的视图
- 组件、属性、消息驱动的状态更新
- 通过 wasm-bindgen 调用浏览器 API
- 配合 Trunk 打包，纯 Rust 写 SPA

## 适合参考
想用纯 Rust 写交互式前端、又熟悉 React 心智模型时，Yew 是平滑的切入点，也便于对比 Dioxus 的设计取舍。
