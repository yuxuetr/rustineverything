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

## 3. Phase 4.3-4.5 后续路线图

| 阶段 | 工作 |
| --- | --- |
| 4.3 ModerationProvider ABI | `get_endpoint() / map_request() / map_verdict()` 三函数；宿主负责 HTTP + 5s 超时 + 1 次重试 |
| 4.4 内置审核插件 | `moderation-openai` / `moderation-anthropic` / `moderation-llamaguard`（本地 ollama fallback） |
| 4.5 数据库 + Admin | `moderation_log / moderation_decisions / moderation_queue` 表 + Admin 队列页 |
| 4.7 验收门禁 | 审核 P95 ≤ 1.5s；模拟违规正确 Block/Flag；LLM 失败 fail-open + 日志 |

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
| LLM/VLM 内容审核 | ⏳ Phase 4.3-4.5 |
| Hot Reload 内存回收验证 | ⏳ Phase 5.1 |
