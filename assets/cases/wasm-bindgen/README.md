# wasm-bindgen
wasm-bindgen 解决 Rust 编译成 WASM 后如何与 JavaScript 双向通信的核心问题，是 Rust 前端生态的地基。

## 看点
- 在 Rust 与 JS 之间传递字符串、结构体、闭包、Promise
- `#[wasm_bindgen]` 宏自动生成 JS 胶水代码
- `web-sys` / `js-sys` 提供完整的 Web API 绑定
- Dioxus、Yew、Leptos 等框架都构建于其上

## 适合参考
想理解 Rust → WASM → 浏览器这条链路如何工作时，wasm-bindgen 是绕不开的一层；本站前端与插件也依赖同源技术。
