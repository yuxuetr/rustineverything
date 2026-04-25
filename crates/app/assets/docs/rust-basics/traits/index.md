# Trait 与泛型

Trait 是 Rust 实现多态和代码复用的核心机制，类似其他语言中的接口。

## 定义 Trait

```rust
trait Summary {
    fn summarize(&self) -> String;
}
```

## 为类型实现 Trait

```rust
struct Article {
    title: String,
    content: String,
}

impl Summary for Article {
    fn summarize(&self) -> String {
        format!("{}: {}...", self.title, &self.content[..20])
    }
}
```

## Trait Bound

```rust
fn notify(item: &impl Summary) {
    println!("Breaking news! {}", item.summarize());
}

// 等价于：
fn notify<T: Summary>(item: &T) {
    println!("Breaking news! {}", item.summarize());
}
```
