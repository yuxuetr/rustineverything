# 站点重设计规范：双生态首页 + 导航 + 付费课程 (Site Redesign)

> 本文是设计文档（先规划后编码）。落地拆分见 §6 路线图，逐项进 `Todos.md`。
> 关联规范：[COURSE_SPEC](./COURSE_SPEC.md)、[CASE_SPEC](./CASE_SPEC.md)、
> [MODULE_SPEC](./MODULE_SPEC.md)、[AUTH_SPEC](./AUTH_SPEC.md)、[SESSION_SPEC](./SESSION_SPEC.md)。

## 1. 定位
- **Rust 工业用途社区**：聚焦真实工程/生产实践，而非玩具示例。
- **两大生态为组织主线**：**Rust 生态** 与 **AI 生态**。
- **内容支柱**：案例（工业实践展示）、课程（含付费）、文档、博客、论坛、播客。
- **差异化核心 = 案例**（工业用法展示与实践）；**变现核心 = 课程**（付费视频课，配套文档/音频/图/代码）。

## 2. 信息架构 (IA)

### 2.1 两个正交的轴
当前导航把两个轴混在一个扁平列表里（博客/案例/论坛… 与 嵌入式/AI/Web3/WASM/CLI 混排），
这是“拥挤且不体现定位”的根因。重设计把两轴分开：

- **轴一 · 内容类型**（来这里想消费什么形式）：案例 / 课程 / 文档 / 博客 / 论坛 / 播客 —— 已有模块。
- **轴二 · 领域 / 生态**（关于什么主题）：Rust 生态、AI 生态及其子领域 —— 横切所有内容类型的**标签/分类法**。

### 2.2 双生态分类法 (taxonomy)
单一事实来源（一份配置驱动 mega 菜单、筛选页、首页 pillars）：

```
Ecosystem = { rust, ai }

Rust 生态 (rust):
  embedded  嵌入式
  web3      Web3
  wasm      WASM
  cli       CLI / 工具
  backend   后端 / 系统      (新增领域，覆盖 全栈/Axum/SeaORM 等)

AI 生态 (ai):
  llm        大模型 / LLM
  inference  推理 / 部署
  agent      Agent
  rust-ai    Rust AI (candle / burn)
```

**与现状的映射（尽量复用，不重造）**：
- 已有模块 `embedded / web3 / wasm / cli` → 归入 **Rust 生态** 的领域落地页（路由保留）。
- 已有模块 `ai` → 提升为 **AI 生态** 这一支柱；其内部按 `llm/inference/agent/rust-ai` 用 **tag** 细分，
  mega 菜单与 `/ai?tag=…`（或 `/ai/<domain>`）筛选页驱动。
- `cases` 已有 `category`（frontend|backend|fullstack|cli|embedded|ai|web3|library|tool|desktop）
  + `tags` + `favorite`（精选置顶）→ **直接复用**：做生态/领域筛选与首页“精选案例”。
- 统一约定：每类内容（case / doc / course / blog）以 `ecosystem` + `domain[]` 标签标注，
  作为 mega 菜单与筛选的统一驱动。`cases` 的 `category` 可派生 `ecosystem/domain`，无需立即改 schema。

### 2.3 导航方向（已选定）
**生态为主 · 双 mega 菜单**（见 §3）。理由：最强化“两大生态”身份定位；案例/课程仍保留顶层入口。

## 3. 导航栏设计

### 3.1 顶层结构
```
[Logo]  Rust 生态▾   AI 生态▾   案例   课程   博客   论坛      🔍  🎨 中 ☾  登录  [ 开始学习 ]
```
- 顶层 6 项（2 mega + 4 链接），≥lg 宽度从容容纳（对比旧版 9 项拥挤）。
- **博客**保留顶层（用户列出的核心内容类型之一）；**播客**并入博客页作为 Tab，不单列。
- 右侧控件沿用现状：搜索 / 主题 / 语言 / 暗色 / 登录·用户菜单 / 开始学习。
- 登录后“开始学习”可按是否拥有课程权益变为“我的课程”。

### 3.2 Mega 菜单内容（三栏卡片式）
**Rust 生态 ▾**
| 应用领域 | 学习资源 | 精选 |
|---|---|---|
| 嵌入式 · Web3 · WASM · CLI · 后端/系统 | Rust 文档 · 入门课程 · 实战案例 | 1–2 张精选案例缩略卡 |

**AI 生态 ▾**
| 方向 | 学习资源 | 精选 |
|---|---|---|
| 大模型 · 推理/部署 · Agent · Rust AI(candle/burn) | AI 文档 · AI 课程 · 实战案例 | 1–2 张精选案例缩略卡 |

- “精选”列复用 `cases.favorite` 过滤（按 ecosystem 取 1–2 条），让菜单不空、引导到案例。
- 菜单项 = taxonomy 配置生成，新增领域只改一处。

### 3.3 响应式
- **≥lg**：完整顶栏；mega 菜单 hover/click 展开（点击空白 / Esc 关闭）。
- **<lg**：hamburger 抽屉；两个生态在抽屉内变**可折叠分组（accordion）**，其余链接平铺。
- mega 的 SSR/hydration：默认收起，开合用客户端 signal（与现有 ThemePicker/用户菜单一致）。

### 3.4 可访问性
- 键盘可达、`aria-expanded`/`aria-haspopup`、Esc 关闭、focus 管理；hover 与 focus 等价触发。

### 3.5 实现要点
- 继续用 `enabled_module_ids` 做模块开关 gating（领域链接随模块启停出现/隐藏）。
- mega 数据结构集中在一处 taxonomy 配置（Rust 常量或 server fn），导航 + 首页 pillars + 筛选页共用。

## 4. 首页设计

自上而下：

1. **Hero**（替换现有）
   - 标题：`用 Rust 构建工业级系统`；副标题：`聚焦 Rust 生态与 AI 生态的工业实战、课程与社区`。
   - 主 CTA：`浏览案例`（差异化优先）+ `查看课程`；内嵌或紧邻搜索框。
2. **两大生态 pillars**（新增，体现定位）
   - 左右两张大卡：**Rust 生态** | **AI 生态**，各列子领域 chips + “进入生态”入口（→ 生态筛选页）。
3. **精选案例 · 工业实践**（新增，旗舰）
   - 3–6 张 `cases.favorite` 案例卡，带 生态 / 领域 / 行业 / 技术栈 标签 + 仓库链接 → `全部案例`。
4. **课程**（新增）
   - 精选课程：免费课 + Pro 课程（资源徽章 🎬视频 📄文档 🎧音频 💻代码）+ `即将上线` → `全部课程`。
5. **社区动态**（新增，2 列）
   - 左：最新博客 / 播客；右：论坛热帖（复用 forum 列表）+ “加入社区”。
6. **按领域浏览**（复用现有 11 卡模块网格，下移为次级导航）。
7. **Footer（加厚）**：关于 / 内容（案例·课程·文档·博客·播客）/ 社区（论坛）/ 法律。

**组件清单**（新增 N / 复用 R）：
- `EcosystemMega`（N，导航）、`EcosystemPillars`（N，首页）
- `FeaturedCases`（N，封装现有 cases server fn + `favorite` 过滤）
- `CourseShowcase` + `CourseCard`（N，含资源徽章 + 价格/层级）
- `CommunityFeed`（N，组合 blog + forum 现有数据）
- `ModuleGrid`（R，§“按领域浏览”，已实现）
- `SiteFooter`（R→增强）

## 5. 课程与付费体系

### 5.1 现状：内容与播放器已具备（关键）
依据 [COURSE_SPEC](./COURSE_SPEC.md)，**“视频课程配套文档/音频/图/代码”的内容模型与播放体验已实现**：
- `Course → Chapter → Lesson`，Lesson 类型 `Doc | Video | Audio | Code`，自适应布局。
- 每个 Lesson 可同时挂：`index.md` 笔记、主/辅视频、主/辅音频、`code/` 多文件 Tab（只读高亮 + 复制 + 下载）、
  `images/` 图、`attachments/` 可下载附件。
- 已集成**标注层**与**讨论面板**。
> 即付费课所需的“每节课 视频+文档+音频+图+代码”**无需重建**，只差“谁能看”的访问层。

### 5.2 缺口（付费要新增）
1. **访问层级** `access_tier`：`free | paid | pro`（课程级）。
2. **预览** `preview`：Lesson 级，免费试看引流。
3. **权益 entitlement**：用户 ↔ 课程（或会员）的拥有关系。
4. **价格元数据**：`price` / `currency`。
5. **Paywall UI** + 访问控制（SSR + 客户端一致）。
6. **支付集成**（分阶段，见 §5.6）。

### 5.3 数据模型
- `course.yaml` 扩展（保持文件系统单一事实来源）：
  ```yaml
  access_tier: paid        # free | paid | pro，缺省 free
  price: 9900              # 分；currency 默认 CNY
  currency: CNY
  ```
- Lesson `index.md` frontmatter 扩展：`preview: true`（标记免费试看课节）。
- **Entitlement（DB / SeaORM 新表）**：
  ```
  entitlements(user_id, course_slug, source, granted_at)
  source ∈ { purchase | membership | coupon | admin_grant }
  ```
- （未来）`orders` / `payments` 表：`user_id, course_slug, amount, provider, provider_ref, status`。

### 5.4 访问控制逻辑
```
可看(lesson, user) =
    course.access_tier == free
 || lesson.preview == true
 || has_entitlement(user, course)        // purchase / membership / admin_grant
```
- 不可看 → 渲染 **Paywall** 覆盖层（价格 + 购买/登录 CTA + 已含试看入口）。
- 鉴权统一走 `core::session`（[SESSION_SPEC](./SESSION_SPEC.md)）；server fn 二次校验，**不可只靠前端隐藏**。

### 5.5 Lesson 播放器（复用 + 微调）
- 沿用现有按 `LessonKind` 的自适应布局。
- 锁定课节：主区显示 Paywall；侧栏目录给锁图标；试看课节正常播放。
- 侧栏增加：课程进度、章节折叠、当前/已购状态。

### 5.6 支付分阶段（可独立上线）
- **A**：内容 + 播放器（全部 free/preview），课程标注 `access_tier/price`（仅展示，不收费）。
- **B**：Entitlement 表 + **Admin 手动授权**（[ADMIN_SPEC](./ADMIN_SPEC.md)）→ 可先**线下/社群售卖**，手动开通。
- **C**：接支付网关 + webhook → 自动写 entitlement。
  - 国内：微信支付 / 支付宝；海外：Stripe / Paddle / Lemon Squeezy（含税务代收）。
  - 优先 Rust 生态封装（参考 [[feedback_rust_ecosystem_first]]）；支付多为 HTTP API，Rust 侧封装 client。
- **D**（可选）：Pro 订阅会员（一次订阅看全部 pro 课程）。

## 6. 分阶段实施路线（建议进 Todos.md）
- **M1 导航重构**：双生态 mega 菜单（桌面 + 移动 accordion）+ taxonomy 配置单一源。
- **M2 首页重排**：Hero → 两大生态 → 精选案例 → 课程 → 社区动态 → 领域网格 → Footer。
- **M3 分类法统一**：`ecosystem/domain` 标签 + 生态/领域筛选页；`ai` 子领域 tag 化。
- **M4 课程付费地基**：`access_tier` + `preview` + Entitlement + Paywall + Admin 授权（= §5.6 A+B）。
- **M5 支付集成**：网关 + webhook（§5.6 C）。
- **M6（可选）Pro 会员**（§5.6 D）。

> M1/M2 不依赖付费，可先上线见效；M4 起才动数据库与鉴权。

## 7. 迁移与兼容
- 现有 `/embedded /ai /web3 /wasm /cli` 路由**保留**，作为生态子领域落地页，由 mega 菜单链接进入。
- **路由命名统一**：COURSE_SPEC 写 `/courses`，实际 Route 为 `/course`（`Courses{}`）。择一统一并对另一个做 301 重定向（建议留意 SEO，见 [SEO_SPEC](./SEO_SPEC.md)）。
- 现有“11 卡模块网格”不废弃，下移为首页“按领域浏览”。
- i18n：新增 `nav.eco.rust` / `nav.eco.ai` / `mega.*` / `home.pillars.*` / `course.tier.*` / `paywall.*` 等键，
  zh/en 同步（`assets/i18n/{zh,en}.ftl`，parity 由 `app_core::i18n::tests::zh_and_en_key_sets_match` 守护）。
  Tailwind 新类需 `cd crates/app && npm run build`（参考 [[project_tailwind_dx_build]]）。

## 8. 风险与取舍
- **mega 菜单复杂度**：移动端 accordion、SSR/hydration 下拉状态、a11y 需仔细处理。
- **付费鉴权一致性**：entitlement 引入后，SSR 与客户端必须同源校验，server fn 不可被绕过。
- **支付合规**：发票 / 退款 / 税务（海外用 Paddle/LemonSqueezy 可代收，降低合规负担）。
- **内容供给**：精选案例 / 课程需足量优质内容才撑得起首页旗舰位；初期可用“即将上线”占位。
