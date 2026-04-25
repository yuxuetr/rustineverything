# Handler 函数

Handler 是处理 HTTP 请求的异步函数：

```rust
async fn hello() -> &'static str {
    "Hello, World!"
}

async fn json_response() -> Json<User> {
    Json(User { name: "Alice".into() })
}
```
