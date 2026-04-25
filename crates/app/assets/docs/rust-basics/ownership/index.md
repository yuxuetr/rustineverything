# 所有权与借用

Rust 通过所有权系统在编译期保证内存安全，无需垃圾回收。

## 所有权规则

1. 每个值都有一个**所有者**（owner）
2. 同一时刻只能有一个所有者
3. 所有者离开作用域时，值被丢弃（drop）

## 借用

```rust
fn main() {
    let s = String::from("hello");
    let len = calculate_length(&s); // 不可变借用
    println!("'{}' 的长度是 {}", s, len);
}

fn calculate_length(s: &str) -> usize {
    s.len()
}
```

## 可变借用

同一时刻只能有**一个**可变引用：

```rust
let mut s = String::from("hello");
let r = &mut s;
r.push_str(", world");
```
