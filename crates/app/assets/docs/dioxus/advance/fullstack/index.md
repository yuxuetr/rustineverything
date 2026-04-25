# 全栈开发

Dioxus Fullstack 支持 Server Function，实现前后端一体化：

```rust
#[post("/api/greet")]
async fn greet(name: String) -> Result<String> {
    Ok(format!("Hello, {}!", name))
}
```
