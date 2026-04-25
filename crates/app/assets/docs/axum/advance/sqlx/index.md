# SQLx 集成

使用 SQLx 进行类型安全的数据库操作：

```rust
let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
    .fetch_one(&pool)
    .await?;
```
