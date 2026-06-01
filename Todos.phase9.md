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

- [x] **wasm import 静态扫描**（`crates/core/src/plugin_security.rs::scan_imports`）
   - `PluginManager::get_or_load_module` 在 `Module::new` 后、注册到缓存前调用
   - `validate_plugin_bytes` 在 instantiate 前调用（admin 上传链路第一时间拒）
   - 白名单 = ∅（当前宿主未暴露任何 host fn）；非空即 `AppError::Plugin("plugin declares disallowed import(s): env::log")`
   - 单测：用 `wat` crate 构造 `(import "env" "log" ...)` 验证拒绝；真实 i18n_fluent 验证通过
- [x] **manifest ↔ exports 一致性校验**（`crates/core/src/plugin_security.rs::verify_manifest_consistency`）
   - capability 期望表（`required_exports` 内表）：theme / i18n / auth-provider / moderation-provider
   - 通用必备：`get_manifest` / `alloc` / `memory`
   - 缺必备 export → 拒绝（带错误清单）；多余 export → 返回 extras 列表给 caller warn
   - 集成到 `PluginManager::scan_uploaded_plugin` 综合 API
   - 单测：真实 i18n 通过；i18n 假装 auth-provider 因缺 `exchange_code` 被拒
- [x] **theme CSS allowlist sanitizer**（`crates/core/src/plugin_security.rs::sanitize_theme_css`）
   - 黑名单字符串扫描（大小写不敏感）：`url(http://` / `url(https://` / `url(//` / 带单双引号变体 / `@import` / `expression(` / `behavior:` / `javascript:` / `vbscript:`
   - 集成到 `PluginManager::aggregate_theme_css_paths`：命中即整段跳过 + `tracing::warn` 记录命中 pattern
   - 不引入完整 CSS parser；已知 CSS 注入模式有限固定
   - 单测：3 正常 case + 8 攻击 case 全覆盖
- [x] **SHA256 lock**（`SiteConfig::plugins_lock` + `PluginManager`）
   - `SiteConfig` 新增 `plugins_lock: HashMap<String, String>` 字段（serde default 空 = 向后兼容）
   - `PluginManager` 新增 `set_plugins_lock` / 内部 `expected_sha256_for` / `plugin_security::verify_sha256`
   - `app/main.rs` 启动时读 site.json → 灌入全局 PluginManager；空表 warn-only
   - `get_or_load_module` 在 `fs::read` 后立即比对；不匹配 `AppError::Plugin("sha256 mismatch (expected X, got Y)")`
   - 单测：sha256 match / 不匹配 / 大小写不敏感 / SiteConfig 反序列化（空 / full / 空字符串视为缺失）4 + 3 = 7 个测试
- [-] **Ed25519 签名校验** —— 本 phase **不做**
   - fork 用户极少生成 PEM 公钥 + 自签发布；签名 detection 不验证没安全意义；完整链路（CLI 签名工具 + admin 验证 + 公钥管理 + UI 标识）工程量超 9.2 单 phase 预算
   - SHA256 lock 已挡 99% 的"文件被偷换"场景；签名能挡的"中间人换包+ 同时重算 hash 改 lock"是真实但小概率威胁，留待有实际需求再做
- [-] **接入 `admin_upload_plugin`** —— 本 phase 推迟
   - `PluginManager::scan_uploaded_plugin` API 已提供 + 测试通过；admin 端 UI 集成留 9.5 文档时一起做
- [x] CI：`cargo test -p app-core --features server --lib` 144 通过；`cargo clippy -p app-core -p app --features server --all-targets -- -D warnings` 0 warning

**完成定义**：恶意 wasm 含 `(import "wasi_snapshot_preview1" ...)` 加载即拒；恶意 theme CSS 含 `url(http://evil.com)` 不会进 `<style>`；plugin 文件改 1 字节即拒；admin UI 能看到每个插件的"signed/unsigned"标识。

---

## Phase 9.3 — `content-transformer` capability（🟡 扩展面证明）

> 来源：Phase 9 范围澄清（2026-06-01）。当前 capability 表里 `layout` / `notification` / `mdx-component` 都是 placeholder（SDK 有常量但宿主没接）。需要至少一个"真正实现过一遍"的扩展型 capability 证明 ABI 表能稳定加新行。`content-transformer` 是博客系统最自然的扩展点（自动加 TOC / 图片 lazy / 自动检测断链等）。

- [x] **SDK 加常量**（`crates/sdk/src/lib.rs`）
   - `capabilities::CONTENT_TRANSFORMER` + 新模块 `content_transformer::FN_TRANSFORM_MARKDOWN`
   - 新增类型 `TransformRequest { content, kind, stage }`（serde default 三字段全部可选）+ `TransformResponse { content, changed }`
   - 加 5 个 unit test：capability 常量 / FN 常量 / serde 双向 / 老 host 仅 content 字段向后兼容 / Response 构造器
- [x] **宿主 `ContentTransformerEngine`**（`crates/core/src/engines/content_transformer.rs` 新建）
   - `apply(manager, content, kind, stage) -> String`：按 `transformers` 顺序串行 chain（前一个输出作为下一个输入）
   - **fail-open**：插件不存在 / wasmi trap / timeout / 输出超限 / 非法 JSON / `content` 字段空字符串 → 跳过 + `tracing::warn`，链路继续
   - 空 `transformers` 列表 / env `CONTENT_TRANSFORMER_DISABLE=true|1` → `apply` 直通返回原 content，**0 次 wasm 调用**
   - 全局 OnceLock 缓存 `default_content_transformer_engine()` + `invalidate_*`（与 ModuleEngine 同款），首屏读 site.json 一次后续 Arc::clone
   - 顶层便利 fn `apply_default_pre(content, kind)`：用 default engine + `shared_plugin_manager`，server fn 调一行
   - 单测 5 个：register/list / apply_site_config 过滤空名 / 空列表 disabled / 空列表零开销直通 / 不存在插件 fail-open
- [x] **Markdown 渲染管线接入** —— 改为在 **server fn 加载内容时** 触发，比 widget 入口接入更稳（用户选 #1）
   - `crates/modules/blog/src/server.rs::get_blog_content` 末尾 `apply_default_pre(&raw, "blog")`
   - 同款补丁应用到 wasm / ai / web3 / cli / embedded 5 板块（kind 取 BOARD_ID 字面值）
   - `crates/modules/docs/src/server.rs::get_doc_content`：在 `parse_doc_frontmatter` 之后对 `content` 跑 `apply_default_pre(_, "doc")`
   - `crates/modules/course/src/server.rs::get_lesson`：对 `lesson.doc.markdown` 字段跑 `apply_default_pre(_, "course")`
   - 不接：forum / comment / cases（理由见 [[content-transformer-untrusted-skip]]：user-submitted 内容由 sanitize_user_html 把关，不应被站点级 transformer 改写；cases 的 `read_case_from_dir` 是同步 fn 改造面太大）
   - `pre` hook only；`post` hook 因 Dioxus 直接渲染 Element 不易实现，未来 SSR pipeline 稳定后再加（[[content-transformer-post-stage]]）
- [x] **plugin_security 表扩展**：`required_exports("content-transformer") = ["transform_markdown"]`；2 个新 unit test（合成 WAT 一个缺一个齐全）
- [x] **SiteConfig 加 `content_transformers: Vec<String>`**（serde default 空）+ 3 个 unit test（default / back-compat / 解析有序列表）
- [x] **示例插件 `crates/plugins/content-toc`**（用 `#[plugin_export]` Phase 9.1 宏）
   - 0 unsafe；`transform_markdown(req: TransformRequest) -> TransformResponse`
   - kind 白名单 = blog / doc / course；其它 kind passthrough；非 `pre` stage passthrough
   - 纯函数 `inject_toc(md: &str) -> String` 覆盖 5 边界：无 heading / 仅 H1 + intro / 多级嵌套 / 已有 [[toc]] / 文末 heading；外加 `is_heading_line` 辅助测 + transform layer assertion，共 7 个测试
   - 加入 workspace members
- [x] **build_themes.sh 扩展**：新增 `ALL_CONTENT_TRANSFORMERS` 数组 + 合并到 `ALL_PLUGINS`；无参跑全量，命令行短名匹配（`content-toc` / `content-toc-plugin`）；不识别的参数报错并列出可用项
- [x] **`assets/site.json` + `crates/app/assets/site.json` 双份**：新增 `"content_transformers": []` 默认空数组（不破坏现网）
- [x] **PLUGIN_ABI.md §2.3 + §9 + §11**：加 `content-transformer | transform_markdown | TransformRequest JSON | TransformResponse JSON` 行 + 示例插件 content-toc 入插件清单 + 加 SPEC 文档链接
- [x] **`docs/CONTENT_TRANSFORMER_SPEC.md`**（新建）：完整 ABI 说明 + kind/stage 枚举 + chain 顺序 + fail-open 语义 + 未来扩展（post / async / kind 扩展）+ 性能 / 关停 / 测试章节

**完成定义**：开启 `content-toc` 插件后 markdown 内容流经 server fn 自动注入 `[[toc]]` 占位；插件 trap 不影响文章渲染（fail-open 单测覆盖）；ABI 表新增一个真正实现过的 capability + 端到端走通宏 (9.1) + plugin_security (9.2) + transformer (9.3) 三层。

---

## Phase 9.4 — 响应式适配 review（🟡 体验完善）

> 来源：Phase 9 范围澄清（2026-06-01）。博客读者主要在手机上看文章，但 Phase 1-8 全程基于桌面浏览器验证，从未系统跑过 mobile viewport。需要一次性扫核心页面 + 修 Tailwind 断点。

- [x] **环境准备**：`dx serve --port 8080` + Playwright MCP；`crates/app/assets/tailwind.css` 需要手动跑 `npx -y @tailwindcss/cli -i crates/app/tailwind-input.css -o crates/app/assets/tailwind.css --minify`（dx 不自动 build Tailwind，详见 audit doc M0）
- [x] **扫描 mobile 375 5 个核心页面**（home / blog 列表 / blog 详情 / forum 列表 / ai 板块）+ desktop 1280 home 作为基线
   - admin 需登录留 manual review；forum 话题详情结构同 forum 列表
- [x] **问题清单**：`docs/PHASE9_RESPONSIVE_AUDIT.md` 完整记录 P0 / P1 / M0 三层问题与修复
- [x] **按 P0 清单修 navbar**（`crates/app/src/components/layouts/classic.rs`）
   - 站名加 `whitespace-nowrap inline-block truncate max-w-32 sm:max-w-none`
   - 站名父 div 加 `min-w-0`；登录 + 开始学习 button 加 `whitespace-nowrap`
   - 右侧 button group `gap-2 sm:gap-3` 紧凑 mobile spacing
   - 新增 `md:hidden` hamburger（☰/✕ 切换图标）
   - 新增 mobile drawer：9 板块纵列 + 开始学习，点链接自动收起
- [-] **触屏交互 / Lighthouse / 评论区 / Math / footer / blog list tag 间隔**：本 phase 不做，留后续 phase（不阻塞 mobile 主路径）

**完成定义**：mobile 375 navbar 不重叠、不 wrap、hamburger 抽屉提供完整板块导航；desktop 1280 无 regression；audit 文档 + before/after 截图入库。

---

## Phase 9.5 — PLUGIN_DEV.md 重写 + 审计指南（🟢 文档）

> 来源：Phase 9 范围澄清（2026-06-01）。9.1 / 9.2 / 9.3 都在动 ABI 表面或新加规范，文档必须同步。同时缺一节"第三方插件如何审计"，对开源 fork 模式至关重要。

- [x] **PLUGIN_DEV.md §3 重写**
   - 主题样例改用 `#[plugin_export]`，从 50 行 → ~12 行；§3.0 新增「为什么看不到 unsafe」说明宏展开
- [x] **PLUGIN_DEV.md §6.1 i18n 模板**改用 `#[plugin_export]` + `Deserialize` 结构体；引用完整 fluent 样例
- [-] **PLUGIN_DEV.md §6.3 moderation 模板补完** —— 留后续 phase（4.3 实现完整但样例补完不阻塞 fork 用户）
- [-] **PLUGIN_DEV.md §6.4 content-transformer 模板** —— 依赖 9.3，已砍
- [x] **PLUGIN_DEV.md §12「如何审计第三方插件」新章节**
   - §12.1 沙箱已挡（Phase 8.1 + 9.2 共 7 类攻击）
   - §12.2 永远检测不了的（4 类逻辑攻击，必须 review source）
   - §12.3 信任链建议（4 条 fork 用户行为准则）
   - §12.4 未来扩展（Ed25519 + audit 流程 + reproducible build，按需要再加）
- [x] **PLUGIN_DEV.md §11 参考**：加 sdk-macros + i18n-fluent 索引
- [-] **PLUGIN_ABI.md §2.3 / §9 更新** —— content-transformer 行不加（9.3 已砍）；moderation 标注小问题留后续

**完成定义**：第三方读 PLUGIN_DEV.md §3 ≤12 行 safe Rust 写出可工作主题；§12 审计指南覆盖"什么能挡 / 什么挡不住"，让 fork 者心里有数。

---

## 进度跟踪

| Phase | 状态 | 关键交付 | 依赖 |
|---|---|---|---|
| 9.1 | ✅ Mostly Done | `#[plugin_export]` proc macro + i18n 改造 0 unsafe + 集成测试通过（PLUGIN_DEV.md 样例重写留 9.5） | — |
| 9.2 | ✅ Mostly Done | wasm import scan + CSS sanitize + manifest 一致性 + SHA256 lock（Ed25519 + admin UI 集成本 phase 不做） | — |
| 9.3 | ✅ Mostly Done | content-transformer capability + ContentTransformerEngine + content-toc 示例 + SPEC（pre stage 落地；post stage 因 Dioxus Element pipeline 与 SSR-only 路径冲突，记为未来扩展） | 9.1 (macro) / 9.2 (manifest 一致性表加新行) |
| 9.4 | ✅ Mostly Done | mobile 375 navbar 修复（hamburger 抽屉 + 站名 truncate + nowrap）+ desktop 1280 无 regression + audit 文档（评论区/Math/footer/list tag 小问题留后续） | — |
| 9.5 | ✅ Mostly Done | PLUGIN_DEV.md §3 + §6.1 改用 #[plugin_export] / §12 审计指南完整章节（PLUGIN_ABI.md content-transformer 相关更新随 9.3 一起砍） | 9.1 |

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
