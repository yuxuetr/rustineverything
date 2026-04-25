# 组件

组件是 Dioxus 应用的基本构建块：

```rust
#[component]
fn UserCard(name: String, age: i32) -> Element {
    rsx! {
        div { class: "card",
            h3 { "{name}" }
            p { "年龄: {age}" }
        }
    }
}
```
