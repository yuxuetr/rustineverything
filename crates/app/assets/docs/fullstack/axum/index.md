# Axum 后端

Axum 是一个基于 Tokio 和 Tower 的 Web 框架，专注于人体工学和模块化。

## 基本路由

```rust
use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## 提取器

```rust
use axum::extract::Path;

async fn get_user(Path(user_id): Path<u32>) -> String {
    format!("User {}", user_id)
}
```
