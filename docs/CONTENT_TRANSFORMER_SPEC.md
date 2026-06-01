# Content Transformer 插件规范（Phase 9.3）

> 适用版本：SDK ABI v1 + Phase 9.3 起的宿主。本文档与
> [`crates/sdk/src/lib.rs`](../crates/sdk/src/lib.rs) /
> [`crates/core/src/engines/content_transformer.rs`](../crates/core/src/engines/content_transformer.rs)
> 同步演进。

## 1. 是什么

声明 capability `content-transformer` 的 wasm 插件，能在宿主**加载 markdown 文件之后、
传给前端 widget 渲染之前**对内容做字符串级修改。常见用途：

- 自动注入 `[[toc]]` 占位 / 目录锚点（见示例插件
  [`crates/plugins/content-toc`](../crates/plugins/content-toc/src/lib.rs)）
- 图片 lazy-loading 属性补全
- 自动外链 `target="_blank" rel="noopener"`
- 自动校对断链 / 死链占位
- 站点级别的 frontmatter 默认值补全（如 author / canonical）

它不是：

- **不**是 AST 操纵。宿主只传字符串，插件只回字符串。Markdown AST 跨 ABI 太重，
  Phase 9.3 故意没做。
- **不**是用户提交内容审核器。审核走 [`moderation-provider`](MODERATION_SPEC.md)。
- **不**是渲染层（Dioxus 组件）。

## 2. ABI

### 2.1 导出函数

```text
transform_markdown(ptr, len) -> u64   // u64 = (ptr<<32) | len，pack_output 风格
```

输入：[`sdk::TransformRequest`] 序列化为 JSON：

```json
{
  "content": "<markdown 原文>",
  "kind":    "blog",       // 业务类型，见 §3
  "stage":   "pre"          // 当前唯一支持的 stage，见 §4
}
```

输出：[`sdk::TransformResponse`] 序列化为 JSON：

```json
{
  "content": "<新 markdown>",
  "changed": true           // 提示位；宿主以 content 字段为准
}
```

### 2.2 必备 export

参考 [PLUGIN_ABI §2.3](PLUGIN_ABI.md#23-能力相关函数按-capability-而异)：

- `get_manifest`（capability 必须列入 `content-transformer`）
- `transform_markdown`
- `alloc` / `memory`（所有插件通用）

[`crate::plugin_security::verify_manifest_consistency`] 在加载时强校验。

### 2.3 推荐写法

用 [`sdk::plugin_export`] 宏（Phase 9.1），完全没有 `unsafe`：

```rust
use sdk::{capabilities, plugin_export, PluginManifest, TransformRequest, TransformResponse};

#[plugin_export]
fn get_manifest() -> PluginManifest {
  PluginManifest::new("my-plugin", "My Plugin", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::CONTENT_TRANSFORMER)
}

#[plugin_export]
fn transform_markdown(req: TransformRequest) -> TransformResponse {
  // 自定义逻辑
  TransformResponse::changed(format!("> 来自 my-plugin\n\n{}", req.content))
}
```

## 3. `kind` 枚举

宿主目前会传以下 `kind` 值：

| kind | 来源 server fn | 备注 |
| --- | --- | --- |
| `blog` | `get_blog_content` | 博客 mdx |
| `doc` | `get_doc_content` | 站内文档 |
| `course` | `get_lesson` | 课程 lesson markdown 正文 |
| `ai` / `cli` / `embedded` / `wasm` / `web3` | 5 个内容板块文章 server fn | 用 `BOARD_ID` 字面值 |

未列出的 kind（如 `podcast`、`forum-topic`）当前**不**走 transformer。论坛 / 评论
等用户提交内容不应该被站点级 transformer 改写，避免 XSS / 内容偏移。

插件遇到未知 kind 推荐直接 passthrough（返回 `TransformResponse::unchanged(req.content)`）。

## 4. `stage` 枚举

| stage | 含义 | 状态 |
| --- | --- | --- |
| `pre` | markdown 解析前的字符串变换 | ✅ Phase 9.3 落地 |
| `post` | 渲染后的 HTML 字符串处理 | ⏳ 未来；详见 §7 |

插件应**显式检查 stage**。未来宿主可能向后兼容地增加 stage，插件遇到陌生 stage 应
passthrough 而非 panic。

## 5. Chain 与执行顺序

宿主在 `site.json` 读取 `content_transformers` 列表，按声明顺序串行调用：

```json
{
  "content_transformers": [
    "content_toc_plugin.wasm",
    "content_lazyload_plugin.wasm"
  ]
}
```

第一个插件输出的 `content` 成为第二个的 `content` 输入。链路中一个插件
trap / timeout / 返回非法 JSON / 空 content → 跳过该插件，下一个仍以前一个的输出
为输入。

参见 [`ContentTransformerEngine::apply`](../crates/core/src/engines/content_transformer.rs)。

## 6. Fail-open 语义

**整条链路保证至少和原文一致**。下面任何一种情况都**不**会让用户看到一篇空白文章：

- 插件文件不存在 / 加载失败
- 插件 wasm trap（除零 / 越界 / panic / fuel 耗尽）
- `tokio` timeout 触发（默认 5s，参见 [PLUGIN_ABI §6](PLUGIN_ABI.md)）
- 输出长度超过 host 限制（默认 8 MiB）
- 返回值不是合法 JSON
- 返回 JSON 但 `content` 字段为空字符串

宿主在 `tracing::warn` 中记录失败 plugin + 原因，运维可据此排查。

设计动机：单插件挂掉**不能**弄死一整篇文章。这与审核插件 `fail-closed`（拒绝提交）
的方向相反；因为审核是面向用户提交的把关，而 transformer 是面向已有内容的增强。

## 7. 未来扩展

### 7.1 `post` stage（HTML 后处理）

当前 Dioxus 直接渲染 `Element`，没有"HTML 字符串"中间产物可以喂给插件。要落地 `post`
stage 需要在 SSR 路径切一条只渲染 HTML 的支线。Phase 9.3 决定**不**做：

- 收益小：现有 widget 已经能用 dioxus 组件 + dangerous_inner_html 自定义渲染
- 复杂度高：需要 SSR-only 路径，破坏 client hydration

如果将来真要做，预期签名：

```rust
TransformRequest { content: "<rendered html>", kind: "blog", stage: "post" }
TransformResponse { content: "<modified html>", changed: true }
```

### 7.2 `kind` 扩展

新增 server fn 调 `apply_default_pre(content, "<kind>")` 即可；不需要协调宿主版本号。
插件遇到陌生 kind 应 passthrough。

### 7.3 异步 / 网络访问

**不支持**。content-transformer 与所有现有插件一样跑在 `wasmi` 沙箱里，宿主未暴露
任何 host fn（[Phase 9.2 import scan](PLUGIN_ABI.md) 强校验 ∅ 白名单）。插件只能用
纯函数 + crate 内置算法。

如果你需要"调 LLM 改写文章"这类语义，应该用另一类 capability（未来的
`async-content-transformer`），不应在这一层做。

## 8. 性能 & 关停

- 空 `content_transformers` 列表（默认）→ 零开销直通，连一次 wasm 调用都不发生
- env `CONTENT_TRANSFORMER_DISABLE=true` → 全局短路；用于性能 benchmark / 紧急回滚
- transformer 与 markdown 内容一起走 server-side cache（Phase 8.5 `DirListingCache`
  不直接命中，但单文件 mtime 不变时 wasm 调用结果会被 [Module 缓存]间接复用）

## 9. 测试

- 纯函数单测：把变换逻辑写成独立 `fn pure_logic(s: &str) -> String`，在 host crate
  跑 `cargo test`；不依赖 wasm runtime
- 集成测：
  - [`crates/core/src/plugin_security.rs::tests::manifest_consistency_passes_for_synthetic_content_transformer`]：
    构造 WAT 验证 capability ↔ exports 一致性
  - [`crates/core/src/engines/content_transformer.rs::tests::apply_skips_nonexistent_plugin_fail_open`]：
    fail-open 路径
  - [`crates/plugins/content-toc/src/lib.rs::tests`]：5 个 inject_toc 边界
- 端到端：`./scripts/build_themes.sh content-toc` 构建插件 → 写入 site.json
  `content_transformers` → `cd crates/app && dx serve` 验证 `/blog/welcome` 自动出现
  `[[toc]]` 占位

## 10. 参考

- [PLUGIN_ABI.md](PLUGIN_ABI.md)：ABI v1 全貌
- [PLUGIN_DEV.md](PLUGIN_DEV.md)：30 分钟插件开发指南
- [MODERATION_SPEC.md](MODERATION_SPEC.md)：审核插件（fail-closed 对照）
- [`crates/plugins/content-toc/src/lib.rs`](../crates/plugins/content-toc/src/lib.rs)：完整示例插件
