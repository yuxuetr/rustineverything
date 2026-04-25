# 状态管理

使用 Signal 管理组件状态：

```rust
let mut count = use_signal(|| 0);
let doubled = use_memo(move || count() * 2);

rsx! {
    button { onclick: move |_| *count.write() += 1, "Count: {count}" }
    p { "Doubled: {doubled}" }
}
```
