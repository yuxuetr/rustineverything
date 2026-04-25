# Dioxus 入门

Dioxus 是一个用 Rust 编写的跨平台 UI 框架，使用类似 React 的声明式语法。

## Hello World

```rust
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! { h1 { "Hello, Dioxus!" } }
}
```

## RSX 语法

```rust
rsx! {
    div { class: "container",
        h1 { "标题" }
        p { "这是一段内容。" }
        button { onclick: |_| println!("clicked!"), "点击我" }
    }
}
```
