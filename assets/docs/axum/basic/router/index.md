---
title: 路由系统
description: 掌握 Axum 路由的定义、嵌套、分组和路径匹配规则
keywords: [axum, router, routing]
sidebar_label: 路由
sidebar_position: 1
---

# 路由系统

Axum 使用 Router 来定义应用的路由结构。

```rust
use axum::{routing::get, Router};

let app = Router::new()
    .route("/", get(|| async { "Hello!" }));
```
