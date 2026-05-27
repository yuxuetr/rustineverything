# LLM Integration Spec

> 适用阶段：Phase 4 前置。位于 `crates/core/src/llm/`。
> 双模式 LLM 客户端：OpenAI 兼容 + Anthropic 兼容。审核 / 自由问答 / 摘要
> 等业务调用都通过 [`LlmClient`] trait 接入，对协议差异不可见。

## 1. 为什么是双模式

| 协议 | 覆盖厂商 |
| --- | --- |
| **OpenAI 兼容** | OpenAI / DeepSeek / Moonshot / Qwen / Zhipu / Together / Groq / Ollama / 几乎所有自托管推理引擎 |
| **Anthropic 兼容** | Claude API 本家 / DeepSeek `/anthropic` 路径 / Bedrock pass-through |

主流厂商几乎都实现了 OpenAI 兼容。Anthropic 协议是少数派但仍需要原生支持
（Claude 本家 + 少量兼容厂商）。同时支持两套是务实选择。

## 2. 配置

四个独立 env 变量，两两成对：

| 环境变量 | 必填 | 说明 |
| --- | --- | --- |
| `OPENAI_LLM_BASE_URL` | 二选一 | OpenAI 兼容 endpoint，**不含路径**（如 `https://api.deepseek.com`） |
| `OPENAI_LLM_API_KEY` | 二选一 | OpenAI 兼容 key |
| `OPENAI_LLM_MODEL` | 否 | 默认 `deepseek-chat` |
| `ANTHROPIC_LLM_BASE_URL` | 二选一 | Anthropic 兼容 endpoint（如 `https://api.deepseek.com/anthropic`） |
| `ANTHROPIC_LLM_API_KEY` | 二选一 | Anthropic 兼容 key |
| `ANTHROPIC_LLM_MODEL` | 否 | 默认 `deepseek-chat` |

**选择规则**：
1. `OPENAI_LLM_BASE_URL` + `OPENAI_LLM_API_KEY` 都非空 → OpenAI
2. 否则 `ANTHROPIC_LLM_BASE_URL` + `ANTHROPIC_LLM_API_KEY` 都非空 → Anthropic
3. 否则 → `None`（业务侧 fail-open）

**没有运行时 failover**：选定后请求只走该协议；失败原样返回错误。两个协
议的请求 / 响应 shape 不同，自动切换会产生不可预期的行为。

## 3. 接口

```rust
use rustineverything_core::llm::{default_client_from_env, LlmMessage};

if let Some(client) = default_client_from_env() {
  let reply = client
    .chat(vec![
      LlmMessage::system("你是审核员，返回 ALLOW 或 BLOCK。"),
      LlmMessage::user("待审内容..."),
    ])
    .await?;
}
```

[`LlmClient`] trait：

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
  fn provider(&self) -> LlmProvider;
  async fn chat(&self, messages: Vec<LlmMessage>) -> AppResult<String>;
}
```

[`LlmMessage`] 三个角色 (`system` / `user` / `assistant`) 是协议中性
抽象。Anthropic 客户端自动把 `system` 角色抽取到顶层 `system` 字段，
对调用方透明。

## 4. 协议差异（实现层处理）

| 维度 | OpenAI | Anthropic |
| --- | --- | --- |
| Endpoint | `POST {base}/v1/chat/completions` | `POST {base}/v1/messages` |
| 鉴权 header | `Authorization: Bearer <key>` | `x-api-key: <key>` |
| 版本 header | — | `anthropic-version: 2023-06-01` |
| system 位置 | messages 中一项 | 顶层 `system` 字段 |
| messages 起始 | 任意角色 | 必须 user |
| 必填字段 | `model`、`messages` | + `max_tokens`（默认 1024） |
| 响应路径 | `choices[0].message.content` | `content[].type=="text"` 的 `text` 串接 |
| 错误体 | `error: {type, message}` | `type=="error"` + `error: {type, message}` |

## 5. 错误处理

返回 [`AppError`]：
- `Validation`：参数级问题（空 messages / Anthropic conv 不以 user 起始）
- `Other`：HTTP 失败 / 非 2xx 状态 / 服务方 error envelope / 响应 JSON 解析失败 / 空 content

错误信息包含状态码、错误类型、消息体前 500 字符，足够定位但不暴露 key。

## 6. 测试

### 6.1 Mock（默认）

`cargo test --features server -p rustineverything-core llm` — 30 个单测
用 `mockito` 走完整 round-trip，验证：
- 请求 body 形状（model / messages / system / max_tokens）
- 鉴权 header 正确
- 协议特定 header（`anthropic-version`）
- 响应解析（choices / content blocks / 多个 text block 串接）
- 错误体识别（非 2xx + 2xx 内 error envelope）
- 参数校验拒绝（空 messages / assistant-only / Anthropic 非 user 起始）

> 测试 client 用 `Client::builder().no_proxy().build()` 显式禁用系统代
> 理 — 项目约定（CLAUDE.md::Rust HTTP Testing），防止 macOS 上 Clash 等
> 代理把 127.0.0.1 黑洞掉。

### 6.2 Live（需 env）

`cargo test --features server -p rustineverything-core --test llm_live -- --ignored --nocapture`

读取仓库根 `.env`，对两个协议各发一次「一加一等于几？」并断言非空回复。
任一 env 不全则跳过对应测试。

实测对 DeepSeek 已验证通过：
```
[openai-live]    reply = 2
[anthropic-live] reply = 2
```

## 7. 与其它系统的集成

- **Phase 4 ModerationStage**：把 `LlmClient` 包装成 `ModerationStage`，
  prompt 让模型输出 `score + label + reason` JSON。
- **Phase 5 moderation 插件**：WASM 插件只负责 `map_request` /
  `map_verdict`；HTTP + LLM 调度由 host 通过该模块完成。
- **管理后台 / Admin 助理**：摘要 / 关键词抽取 / 回复建议等都可直接调用。

## 8. 局限与后续

- 当前**无流式输出**（同步 `chat`）。未来 Phase 4 流式响应可加
  `async fn chat_stream(...) -> Stream<...>` API。
- **无 tools / function calling**。Anthropic / OpenAI 协议都支持但 shape
  差异较大；按需添加抽象层。
- **无 token 计数 / 计费监控**。生产可加 `metrics` crate 抽样 logging。
- **timeout 固定 30s**。如需更精细可在 `OpenAiChat::new` 后链 `with_client`
  注入自定义 reqwest::Client。
