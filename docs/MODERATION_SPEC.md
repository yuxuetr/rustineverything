# Moderation & XSS Spec

> 适用阶段：Phase 1C.4 ModerationEngine 骨架 + Phase 4.2 XSS 防护（v2.1 Todos.md）。
> 本文记录用户内容的 **安全模型**（XSS / 内联事件 / 危险协议）与
> **审核流水线骨架**（ModerationStage / Verdict / Pipeline）。
> 完整 LLM/VLM 审核（Phase 4.3-4.5）待后续 PR 落地。

## 1. XSS 攻击面审计（Phase 4.2）

### 1.1 渲染管道回顾

```text
用户提交 (评论 / 话题 / 回复 / 标注)
        │
        ▼
   Markdown(props.untrusted = true)   ← Phase 4.2 新增 prop
        │
        ▼  ① sanitize_user_html  (defense in depth)
        │
        ▼
   pulldown_cmark::Parser
        │
        ▼  ② Event::Html fallback → span { "{h}" } (Dioxus 自动转义)
        │
        ▼
   render_stream → RSX 节点树
```

### 1.2 既有防御 — 默认安全

`crates/widgets/src/mdx.rs::render_stream` 第 189-197 行：

```rust
Event::Html(html) | Event::InlineHtml(html) => {
    let h = html.trim();
    if h.starts_with('<') {
        if let Some(component) = render_mdx_registry(h) {
            nodes.push(component);   // ← 仅匹配白名单 MDX 组件 (YouTube/Bilibili/5色/Underline/Strikethrough/PodcastCard)
            continue;
        }
    }
    nodes.push(rsx! { span { "{h}" } });   // ← 不匹配的 raw HTML 走文本字面值，Dioxus 自动 escape
}
```

**默认即免疫 `<script>` 注入**：raw `<script>...</script>` 经 cmark 后变成
`Event::Html("<script>...")`，不匹配任何注册组件，最终 RSX 输出
`<span>&lt;script&gt;...&lt;/span&gt;` — 显示为文本，不执行。

### 1.3 Defense in Depth — `sanitize_user_html`

`crates/widgets/src/sanitize.rs`。在 cmark 解析之前做一次字符串级清洗：

| 处理 | 规则 |
| --- | --- |
| `<script>` 块 | 整块删除（含属性、含跨行内容） |
| `<iframe>` / `<object>` / `<embed>` 块 | 同上 |
| `<style>` 块 | 同上（防 CSS 注入） |
| 内联事件 `on*=...` | 三种 value 形式（`"..."` / `'...'` / 裸值）均删 |
| `javascript:` URL | 替换为 `about:blank` |
| `data:text/html` URL | 同上 |

特性：
- **UTF-8 安全**：通过字符串切片拼接，不破坏中文/emoji。
- **大小写不敏感**：`<SCRIPT>` / `<sCrIpT>` 均被识别。
- **MDX 组件白名单未受影响**：`<YouTube id="..." />` 等不在此处理范围（cmark 解析后由 registry 渲染）。
- **代码块（` ``` `）** 中的字面 `<script>` 同样会被清洗，trade-off：教程作者请用 HTML 实体 `&lt;script&gt;` 或截图。

### 1.4 `dangerous_inner_html` 审计

全工作区共 2 处 `dangerous_inner_html`，均在 `crates/widgets/src/mdx.rs`：

| 行号 | 用途 | 数据来源 | 风险 |
| --- | --- | --- | --- |
| 184 | 内联数学公式（MathML） | `latex_to_mathml_string(...)` → pulldown-latex 库 | 低（库生成结构化 MathML，不直接回显用户输入字符） |
| 190 | 块级数学公式 | 同上 | 低 |

**结论**：当前 `dangerous_inner_html` 的输入完全由 `pulldown-latex`
库产出，不直接含用户输入字符串。若该库未来允许 escape，则 LaTeX
输入可能成为攻击向量；目前不需要额外处理，但需在升级该库时复核。

### 1.5 启用方式

```rust
use rustineverything_widgets::Markdown;

// 评论 / 话题 / 标注 等用户内容：
rsx! { Markdown {
    content: user_body,
    blog_id: "comment".into(),
    untrusted: true,           // ← 启用 sanitize_user_html
}}

// 站点作者内容（blog / docs / lessons / cases）：
rsx! { Markdown {
    content: post_body,
    blog_id: post.slug,
    // untrusted 默认 false，保留原渲染行为
}}
```

`untrusted = true` 已应用至：
- `crates/app/src/components/comment.rs`（评论编辑预览 + 已发评论）
- `crates/modules/forum/src/forum.rs`（话题正文 + 回复 + 新话题预览）

### 1.6 测试覆盖

`crates/widgets/src/sanitize.rs::tests`：15 个单测

- ✅ 普通 markdown 透传（无副作用）
- ✅ `<script>` / `<SCRIPT>` / `<sCrIpT>` 等大小写变体
- ✅ 带属性的 `<script src="evil">`
- ✅ `<iframe>` / `<object>` / `<embed>` / `<style>` 块
- ✅ `onclick="..."` / `onerror='...'` / `onerror=...` 三种属性形式
- ✅ `javascript:` URL → `about:blank`
- ✅ `data:text/html,<script>...</script>` 完全中和
- ✅ Polyglot：`<svg onload=alert(1)><script>alert(2)</script>`
- ✅ Markdown 围栏（` ``` `）中的字面 `<script>` 也被清洗（已知 trade-off）
- ✅ 误伤防护：text 中孤立 `onenote` 不剥离

`cargo test --features server -p rustineverything-widgets sanitize` → 15 passed; 0 failed.

## 2. ModerationEngine 骨架（Phase 1C.4）

### 2.1 数据类型

`crates/core/src/engines/moderation.rs`：

```rust
pub enum ModerationLabel { Allow, Flag, Block }

pub struct Verdict {
    pub score: f32,           // 0.0 ~ 1.0
    pub label: ModerationLabel,
    pub reason: String,
}

pub trait ModerationStage: Send + Sync {
    fn name(&self) -> &'static str;
    fn evaluate(&self, content: &str) -> Verdict;
}
```

### 2.2 Pipeline 行为

| 输入 stages | 行为 |
| --- | --- |
| 空 | 总是返回 `Verdict::allow()` |
| 含 Block | 早停，返回首个 Block |
| 仅 Flag | 取分数最高的 Flag |
| 仅 Allow | 返回 `Verdict::allow()` |

### 2.3 测试覆盖

`crates/core/src/engines/moderation.rs::tests`：6 个单测

- ✅ Engine name = "moderation"
- ✅ 空 pipeline → Allow
- ✅ Block 早停（后续 stage 不执行）
- ✅ Flag 取最高分
- ✅ 全 Allow → Allow
- ✅ Score clamp 至 [0.0, 1.0]

## 3. LLM 审核插件（Phase 4.3-4.4）

### 3.1 架构分层

```text
crates/core::engines::moderation   # Verdict / Label / Thresholds / 同步 trait
crates/llm                          # OpenAI + Anthropic 双协议 HTTP 客户端
crates/modules/moderation           # AsyncModerationStage + PluginModerationStage + Pipeline
examples/plugin-moderation-deepseek # 演示 wasm 插件（build_prompt + parse_verdict）
```

**插件管 policy，宿主管 transport**：
- **插件**：写 prompt、解释 LLM 输出。每个站点可以装多个不同策略的插件。
- **宿主**：通过 `crates/llm` 的 `LlmClient` 实际发 HTTP 请求，复用四个 env
  变量 (`OPENAI_LLM_BASE_URL` / `OPENAI_LLM_API_KEY` / `ANTHROPIC_LLM_BASE_URL`
  / `ANTHROPIC_LLM_API_KEY`)。

### 3.2 ABI（capability = `moderation-provider`）

| 函数 | 输入 (JSON) | 输出 (JSON) |
| --- | --- | --- |
| `get_manifest` | — | `PluginManifest` |
| `moderation_build_prompt` | `ModerationSubmission { content, kind, ref_path }` | `Vec<LlmMessage>` |
| `moderation_parse_verdict` | LLM 原始文本 | `ModerationVerdict { score, label: "allow"\|"flag"\|"block", reason }` |

类型定义在 `crates/sdk/src/lib.rs`，插件只需声明 capability `MODERATION_PROVIDER`
即被 [`PluginEngine::filter_by_capability`](ENGINES_SPEC.md) 识别。

### 3.3 配置（site.json）

```jsonc
{
  "moderation": {
    "enabled": false,                                     // 默认 disabled
    "plugins": ["plugin_moderation_deepseek.wasm"],       // 装载顺序 = 评估顺序
    "thresholds": {                                       // 可选
      "block_above": 0.9,
      "flag_above": 0.5
    }
  }
}
```

**默认安全**：
- `enabled = false` → 流水线为空，evaluate 总是返回 Allow（零开销）
- `enabled = true` 但 `plugins = []` → 仍为空流水线
- `enabled = true` + plugins 配了但 LLM env 没配 → 也是空流水线 + warning 日志
- 三重保险，**不会**因配置错误把用户提交吞掉

### 3.4 Fail-open 策略

每个 stage 任一步骤失败 → 返回 Allow + 写 warning 日志：
- 插件文件不存在
- 插件 `build_prompt` 调用失败 / 返回非法 JSON / 0 messages
- LLM 调用失败（超时、网络、鉴权、配额）
- 插件 `parse_verdict` 调用失败 / 返回非法 JSON

Block 决定必须由完整成功的流水线产出。这保证了 LLM 故障期间站点仍可用。

### 3.5 多模态（图像审核）

评论 / 话题中夹带的图片（站点 `/uploads/...`）也走同一条流水线，无需独立插件。

**调用方式**：

```rust
use rustineverything_sdk::{ImageRef, ModerationSubmission};

let submission = ModerationSubmission::new(comment_body)
  .with_kind("comment")
  .push_image(ImageRef::url("https://example.com/uploads/x.jpg"));
let verdict = pipeline.evaluate(submission).await;
```

**URL 形态选择**：

| 形态 | 适用 | 注意 |
| --- | --- | --- |
| 绝对 https URL | 生产 / 公网可达 | LLM 厂商服务器侧 fetch；要求图片端公开访问 |
| `data:image/...;base64,...` | 私有 / localhost / 不想公开图 | 流量随 prompt 一起发；OpenAI / Anthropic 都接受 |
| 相对 `/uploads/x.jpg` | 不允许 | hook 调用方必须先补全成绝对 URL |

**协议转换**（`crates/llm` 自动处理）：

- OpenAI 兼容：始终发 `image_url`，data URL 原样传
- Anthropic 兼容：data URL 自动拆为 `source.base64`；http(s) URL 走 `source.url`

**插件 build_prompt** 自动 detect `submission.images`，按顺序追加图像块到
user message，并升级 system prompt 加入视觉审核维度（色情 / 血腥 / 政治
符号 / 文本-图片不匹配的诱导）。

### 3.6 端到端实测

`examples/plugin-moderation-deepseek` 已对接两个端点实测：

| 输入 | LLM | label | score |
| --- | --- | --- | --- |
| 「感谢分享，这篇博客写得很清晰」 | DeepSeek | Allow | 0.10 |
| 「你这个 sb，写的什么垃圾文章…」 | DeepSeek | Block | 0.95 |
| 「分享一张 Rust 的 logo」 + Rust logo 图 | gpt-4o-mini | Allow | 0.00 |

复现命令：
```sh
cargo test -p rustineverything-module-moderation --test live_pipeline \
  -- --ignored --nocapture --test-threads=1
```
要求 `.env` 配好任一对 LLM env，且 wasm 已 `cp` 到 `assets/plugins/`。

### 3.7 启用 / 禁用

```sh
# 启用：编辑 site.json 把 enabled 设 true，列上需要的插件
$EDITOR assets/site.json
docker compose restart app   # 重启使配置生效

# 禁用：把 enabled 设回 false（plugins 字段可留着备用）
$EDITOR assets/site.json
docker compose restart app
```

无须改代码，无须重新 build 镜像。

### 3.8 Phase 4.5 待补

| 项 | 说明 |
| --- | --- |
| `moderation_log` 表 | 持久化所有判定记录（用户 + 内容 + verdict + LLM 原文） |
| `moderation_queue` 表 | Flag 状态的内容入队，Admin 复核界面消费 |
| Admin 队列页 | 列表 + 批量 approve/reject |
| 阈值在线调整 | 当前需要改 site.json + 重启；可加 admin server fn 写回 |
| 评论 / 话题 / 标注 提交路径接入 | 目前流水线已就绪但还没 hook 到具体 server fn |

## 4. 与其他引擎的关系

| 引擎 | 关系 |
| --- | --- |
| `ModuleEngine` | 模块开关与审核正交。Phase 4 实现后，每模块可独立配置审核阈值 |
| `PluginEngine` | 审核插件通过 PluginEngine 加载，capability = `moderation_provider` |
| `AuthEngine` | 用户标识用于审核日志（用户重复违规可触发 ban） |
| `ContentEngine` | XSS 防护层位于此处之前 — sanitize 在 cmark 之前 |

## 5. 安全清单（持续维护）

| 项目 | 状态 |
| --- | --- |
| OAuth state CSRF 校验 | ✅ Phase 1A.4 |
| 图片上传白名单 + 大小限制 | ✅ Phase 1A.4 |
| access_token 加密存表 | ✅ Phase 1A.4 |
| JWT_SECRET / BASE_URL 强制配置 | ✅ Phase 1A |
| Cookie Secure flag (生产) | ✅ Phase 1A |
| **用户 Markdown XSS 防护** | ✅ Phase 4.2 |
| **`dangerous_inner_html` 审计** | ✅ Phase 4.2（仅 2 处，pulldown-latex 输出，无用户字面回显） |
| LLM 内容审核（插件 + 默认 disabled） | ✅ Phase 4.3-4.4（基础设施完成；4.5 DB/Admin 待补 + comment/forum hook 待接） |
| 视觉审核（图片评论） | ✅ 通过 `ModerationSubmission.images` + 多模态 LlmMessage，已对 gpt-4o-mini 实测 |
| Hot Reload 内存回收验证 | ⏳ Phase 5.1 |
