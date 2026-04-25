# 提取器

提取器从 HTTP 请求中提取数据：

```rust
use axum::extract::{Path, Query, Json};

async fn get_user(Path(id): Path<u32>) -> String {
    format!("User {}", id)
}
```
