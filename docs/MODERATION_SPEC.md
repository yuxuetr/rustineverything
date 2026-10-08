# Moderation & XSS Spec

> 适用阶段：Phase 1C.4 ModerationEngine 骨架 + Phase 4.2 XSS 防护（v2.1 Todos.md）。
> 本文记录用户内容的 **安全模型**（XSS / 内联事件 / 危险协议）与
> **审核流水线骨架**（ModerationStage / Verdict / Pipeline）。
> LLM/VLM 审核见 §3。

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
use widgets::Markdown;

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

`cargo test --features server -p widgets sanitize` → 15 passed; 0 failed.

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

## 3. LLM 审核（Phase 4.3-4.4；R4 起内置）

### 3.1 架构分层

```text
crates/core::engines::moderation   # Verdict / Label / Thresholds / 同步 trait
crates/llm                          # OpenAI + Anthropic 双协议 HTTP 客户端
crates/modules/moderation           # AsyncModerationStage + UrlBlocklistStage + LlmModerationStage + Pipeline
```

`LlmModerationStage`（`crates/modules/moderation/src/llm_stage.rs`）分三步：

1. `build_messages(submission)`：系统提示词 + 用户消息（`[场景: kind ref_path]` 前缀、
   正文、`[包含链接: ...]` 提示、图片块）
2. `LlmClient::chat` 发请求。端点与协议由 `crates/llm` 按 env 选择
   (`OPENAI_LLM_BASE_URL` / `OPENAI_LLM_API_KEY` / `ANTHROPIC_LLM_BASE_URL`
   / `ANTHROPIC_LLM_API_KEY`)
3. `parse_verdict(reply)`：先整体按 JSON 解析，失败取第一个 `{...}`（应对 markdown
   围栏）；未知 label 视为 allow，score 夹到 0–1

R4 之前这两步（提示词、结论解析）在 wasm 插件 `plugin-moderation-deepseek` 里，
现已编译进宿主，对同一批输入与原插件输出一致（迁移时用原 wasm 逐条对比过消息与结论）。

### 3.2 配置（site.json）

```jsonc
{
  "moderation": {
    "enabled": false,                                     // 默认 disabled
    "llm_review": false,                                  // 是否调 LLM 审核，默认关
    "url_blocklist": [                                    // 链接黑名单，可选
      "scam.com",                                         // 精确匹配
      "*.phishing.example",                               // 通配子域
      "bit.ly"                                            // 已知短链中介
    ],
    "thresholds": {                                       // 可选
      "block_above": 0.9,
      "flag_above": 0.5
    }
  }
}
```

R4 之前的 `"plugins": [...]` 字段已废弃：读取时忽略，不会开启 LLM 审核，
需改为 `"llm_review": true`。

**链接检测两层方案**：

| Layer | 类型 | 何时跑 | 谁判定 |
| --- | --- | --- | --- |
| **1 UrlBlocklistStage** | host-native 同步 stage | 流水线第一站 | 直接命中域名 → Block(1.0)，不调 LLM |
| **2 LLM prompt URL 上下文** | `build_messages` 内嵌 | LLM 调用时 | 模型基于 `[包含链接: ...]` 上下文判断仿冒/钓鱼/诱导 |

Layer 1 便宜确定，专治已知坏域名；Layer 2 智能但贵，专治未知拼写仿冒
（例如 `paypa1-security.com` 仿冒 `paypal.com`）。两层串联，先快后慢。

URL 黑名单 **空数组 = 该 stage 不注册**，零开销；Layer 2 只有当评论里有
URL 时才追加 prompt 上下文。

**默认安全**：
- `enabled = false` → 流水线为空，evaluate 总是返回 Allow（零开销）
- `enabled = true` 但 `llm_review = false` 且无黑名单 → 仍为空流水线
- `llm_review = true` 但 LLM env 没配 → 跳过 LLM stage + warning 日志
- 三重保险，**不会**因配置错误把用户提交吞掉

### 3.3 Fail-open 策略

LLM stage 任一步骤失败 → 返回 Allow + 写 warning 日志：
- LLM 调用失败（超时、网络、鉴权、配额）
- 模型回复里读不出结论 JSON

Block 决定必须由完整成功的流水线产出。这保证了 LLM 故障期间站点仍可用。

### 3.4 多模态（图像审核）

评论 / 话题中夹带的图片（站点 `/uploads/...`）也走同一条流水线。

**调用方式**：

```rust
use sdk::{ImageRef, ModerationSubmission};

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

`build_messages` 把 `submission.images` 中非空 URL 按顺序追加为图像块；系统
提示词含视觉审核维度（色情 / 血腥 / 政治符号 / 文本-图片不匹配的诱导）。

### 3.5 端到端实测

对接真实端点实测（R4 内置后重跑，结果一致）：

| 输入 | LLM | label | score | 由谁判定 |
| --- | --- | --- | --- | --- |
| 「感谢分享，这篇博客写得很清晰」 | gpt-4o-mini | Allow | 0.00 | LLM |
| 「你这个 sb，写的什么垃圾文章…」 | gpt-4o-mini | Block | 1.00 | LLM |
| 「分享一张 Rust 的 logo」 + Rust logo 图 | gpt-4o-mini | Allow | 0.00 | vision LLM |
| 「点 https://scam.example/x 领奖」 | — | Block | 1.00 | UrlBlocklistStage (精确) |
| 「https://login.phishing.example/verify」 | — | Block | 1.00 | UrlBlocklistStage (通配) |
| 「您的 PayPal 已冻结...登录 https://paypa1-security.com」 | gpt-4o-mini | Block | 1.00 | LLM（URL 上下文判定仿冒） |

复现命令：
```sh
cargo test -p module-moderation --test live_pipeline \
  -- --ignored --nocapture --test-threads=1
```
要求 `.env` 配好任一对 LLM env。

### 3.6 启用 / 禁用

```sh
# 启用：编辑 site.json 把 enabled 设 true（需要 LLM 审核再把 llm_review 设 true）
$EDITOR assets/site.json
docker compose restart app   # 重启使配置生效

# 禁用：把 enabled 设回 false
$EDITOR assets/site.json
docker compose restart app
```

无须改代码，无须重新 build 镜像。也可在 `/admin/moderation-settings` 修改，保存即热重载。

### 3.7 业务模块接入

5 个提交入口已 hook 到全局 pipeline：

| 入口 | 文件 | 行为 |
| --- | --- | --- |
| 评论 `post_comment` | `crates/modules/comments/src/server.rs` | DB 写入后入队；Block → ServerFnError |
| 话题 `create_topic` | `crates/modules/forum/src/server.rs` | 标题 + 正文合并审核 |
| 回复 `post_reply` | 同上 | 正文审核；事务提交后入队 |
| 标注 `create_annotation` | `crates/modules/course/src/server.rs` | 只审 `note` 字段（非空时）；空 note 不调 LLM |
| 上传 `upload_image` | `crates/modules/uploads/src/server.rs` | **写盘前** 用 base64 data URL 调 vision LLM；Block 不落盘 |

每个 hook 自动：
1. 从 markdown 抽 `![alt](url)` 图片 URL（站内 `/uploads/...` + 外站 https）
2. 相对路径 `absolutize_image_url` 拼成绝对 URL（用 `BASE_URL`），给 vision LLM 用
3. 装填 `ModerationSubmission { content, kind, ref_path, images }`
4. 走 `evaluate_submission` 流过 [`shared_pipeline`]
5. 按 Verdict label 决定返回 / 标记 / 通过

[`shared_pipeline`] 是进程级 `OnceLock<RwLock<Arc<ModerationPipeline>>>`，首次
访问时读 `site.json` + env 装载。**Phase 5.1 后支持 hot reload**：`reload_pipeline()`
原子替换全局 pipeline（重读 site.json + LLM env），admin 保存审核设置即生效，
无需重启进程。阈值在装载时经 `ModerationThresholds::validate()`
校验（越界 / NaN / block<flag → 回退默认 + 告警）。

### 3.8 审核队列（Phase 4.5）

Flag 决定自动入 `moderation_queue` 表等待 admin 复核。

**Schema** (`crates/migration/src/m20260530_000002_moderation_queue.rs`)：

| 列 | 类型 | 用途 |
| --- | --- | --- |
| `id` | BIGSERIAL PK | 队列 id |
| `kind` | VARCHAR(32) | `comment` / `topic` / `reply` / `annotation` |
| `ref_id` | BIGINT nullable | 业务表主键（comment.id 等） |
| `ref_path` | TEXT | 人类可读路径，例 `blog:welcome` / `topic:42` |
| `user_id` | INTEGER FK ON DELETE SET NULL | 提交者 |
| `content` | TEXT | 内容快照（避免业务行被删后失去上下文） |
| `images` | TEXT | URL 数组 JSON 字符串 |
| `score` / `label` / `reason` | — | LLM 评估结果 |
| `status` | VARCHAR(16) | `pending` / `approved` / `rejected` |
| `reviewer_user_id` / `reviewer_note` / `reviewed_at` | — | 复核痕迹 |

索引：`(status, created_at DESC)` + `(kind)`。

**Hook 流程**：业务行先入库拿到 id，再 `enqueue_if_flagged(db, verdict,
kind, Some(ref_id), ref_path, user_id, content, image_urls)`。Allow / Block
都是 no-op（Block 在前置阶段已被拒绝，不写业务库也不入队）。

**Admin 复核页** `/admin/moderation`：
- Tab 切换：待复核 / 已通过 / 已拒绝 / 全部
- 每行：状态徽章 + 类型 + 路径 + 作者 + 评分百分比 + 理由 + 内容快照 + 图片缩略图
- 待复核行：「通过」按钮 → `admin_approve_moderation`（保留内容，标记
  approved）；「拒绝（删除内容）」按钮 → `admin_reject_moderation`
  （按 kind+ref_id 删除业务表行，标记 rejected）
- Dashboard 卡片新增 "审核待办" 计数

**对已有部署的迁移注意**：如果你的 postgres 是通过 `init.sql` 直接灌入
（而非 `Migrator::up`），`seaql_migrations` 表里没有 initial_schema 记录，
应用启动时 Migrator 会反复尝试重跑初始迁移并失败（`relation "..."
already exists`）。一次性修复：

```sh
./scripts/repair_seaql_migrations.sh
```

脚本会建 `seaql_migrations` 表（如未建）并补一条 `initial_schema` 记录，
之后 Migrator 跳过 initial、正常跑 `m20260530_000002_moderation_queue`。

### 3.9 Phase 4.5 待补

| 项 | 说明 |
| --- | --- |
| `moderation_log` 表 | 持久化所有判定记录（用户 + 内容 + verdict + LLM 原文） |
| `moderation_queue` 表 | ✅ Flag 状态的内容入队，Admin 复核界面消费 |
| Admin 队列页 + 批量 approve/reject | ✅ 单条 + 批量通过/拒绝 + 作者历史违规徽章 |
| 阈值 schema 校验 | ✅ `ModerationThresholds::validate()`（范围/NaN/block≥flag），装载时校验 |
| 阈值在线图形编辑 | ⏳ hot reload 后改 site.json 点「重新载入」即生效；图形编辑器待补 |
| 评论 / 话题 / 回复 / 标注 / 上传 提交路径接入 | ✅ 5 条提交路径全部 hook 到 ModerationPipeline |

## 4. 与其他引擎的关系

| 引擎 | 关系 |
| --- | --- |
| `ModuleEngine` | 模块开关与审核正交。Phase 4 实现后，每模块可独立配置审核阈值 |
| `crates/llm` | LLM stage 的 transport；只在 `llm_review = true` 时被调用 |
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
| LLM 内容审核（内置 stage + 默认 disabled） | ✅ Phase 4.3-4.5（基础设施 + DB/Admin + 5 条 hook 全部就绪） |
| 视觉审核（图片评论） | ✅ 通过 `ModerationSubmission.images` + 多模态 LlmMessage，已对 gpt-4o-mini 实测 |
| Hot Reload 内存回收验证 | ✅ Phase 5.1（invalidate 即 Drop 旧 Module，单测验证缓存恒为 1；RSS 长跑见 OPERATIONS.md §2.4） |
