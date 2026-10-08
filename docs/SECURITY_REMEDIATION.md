# 安全整改计划

> 来源：2026-10-08 三路只读安全审计（站点前端攻击面、WASM 插件体系、dioxus-ui 组件库），在 dioxus-ui 迁移（[`DIOXUS_UI_MIGRATION.md`](DIOXUS_UI_MIGRATION.md)）过程中发起。
> 站点尚未上线：本计划按「上线前必须修完」组织，不做线上止血或数据排查。
> dioxus-ui 的问题记在 [`DIOXUS_UI_FEEDBACK.md`](DIOXUS_UI_FEEDBACK.md)（FB-02 起），在 dioxus-ui 仓库修复。

## 1. 原则

按优先级：

1. **dioxus-ui 组件默认安全**：使用者不读安全文档、按示例写代码，也不应因组件本身引入漏洞；必须由使用者负责的部分（如校验不可信 URL），文档要写明。
2. **站点上线前不留可被利用的漏洞**：用户提交的内容、公开接口、付费内容边界都要有防线，且每条防线有测试。
3. **WASM 插件**：宿主能保证的由宿主强制（沙箱、资源上限、输出校验、来源校验）；只能由插件作者保证的，写进开发者安全指南，并在宿主侧尽量 fail closed。

**验收要求（所有条目通用）**：每个修复都附一个**修复前会失败**的测试（或可重复的复现脚本），先确认它红，再修到绿。没有红过的测试不算验证。

## 2. 发现总表

核实状态：**已复现**＝本人运行确认；**代码确认**＝本人阅读代码确认；**静态审计**＝审计报告结论，修复时先复现。

| ID | 级别 | 位置 | 问题 | 核实 |
| --- | --- | --- | --- | --- |
| SEC-01 | 严重 | `crates/widgets/src/mdx.rs:182-191`、`:589`（`latex_to_mathml_string`）、`:89`（`ENABLE_MATH` 对不可信内容也开启） | 数学公式输出不经任何净化直接进 `dangerous_inner_html`；pulldown-latex 对 `\text{…}` 内容不转义。评论 / 论坛帖子写 `$\text{<img src=x/onerror=…>}$` 即对所有浏览者执行脚本（存储型 XSS），管理员浏览即账号接管 | **已修复（A1）**：库级复现 `<mtext><img src=x/onerror=alert(1)></mtext>`；解析错误信息（`<merror>`）同样回显原始输入 |
| SEC-02 | 高 | `crates/app/src/server/static_assets.rs:15` | `/courses` 是无鉴权的 `ServeDir`，付费课时正文、音视频可直接下载，绕过 `get_lesson` 的权益检查 | **已修复（A2）**：复现为未登录 `GET /courses/rust-basics/02-ownership/01-borrow-rules/index.md` → 200；修复后 404，试看课时与课程级文件仍 200 |
| SEC-03 | 高 | `crates/modules/docs/src/server.rs:226` | `get_doc_content(path)` 直接 `join(path)`，无 `..` / 绝对路径检查，且公开 | **已修复（A3）**：复现为 `{"path":"../courses/…"}` 返回课时正文；修复后返回「文档未找到」，正常文档不受影响 |
| SEC-04 | 高 | `crates/core/src/auth/mod.rs` profile 请求处；`crates/plugins/*-auth`（如 `github-auth/src/lib.rs:66`） | profile 响应不检查 HTTP 状态；插件取不到 `id` 时回退为 `"0"`。限流 / 错误响应会把不同用户映射到同一个 `(provider, "0")` 账号 | **已修复（A4）**：单测确认 401 / 403 / 429 原先被当作 profile |
| SEC-05 | 高 | `crates/core/src/auth/mod.rs:237`、profile/token 请求处 | `auth_url` / `token_url` / `profile_url` 由插件决定，宿主把 `client_secret` 发往插件给的 `token_url`；插件还决定 `external_id`。恶意插件可窃取 secret、SSRF、冒充任意用户 | 代码确认 |
| SEC-06 | 高 | `crates/core/src/lib.rs`（`get_or_load_module`）；auth `mod.rs:168`、moderation `pipeline.rs:130` / `plugin_stage.rs:43` | SHA-256 锁只在共享管理器的加载路径生效，auth / moderation 各自 `PluginManager::new()` 绕过；`assets/site.json` 无 `plugins_lock` 条目；文档提到的锁生成工具不存在 | 静态审计 |
| SEC-07 | 高（可用性） | `crates/app/src/server/security.rs:39`；全站约 20 处 `document::eval` | CSP 不含 `'unsafe-eval'`，`document::eval` 被拦截后 wasm panic，页面加载即失效：主题插件从不生效、暗色切换无效、交互不可靠 | 已复现（`bypassCSP` 对照实验） |
| SEC-08 | 中 | `crates/core/src/plugin_security.rs:83-99`；CSP `img-src https:` | 主题 CSS 黑名单可绕过：`image-set("https://…")`、`url(http:evil.com)`、反斜杠形式等，可经属性选择器外泄页面数据；不拒绝 `<`。（`</style>` 跳出：主题 CSS 只在客户端注入 `<style>` 的 innerHTML，不被解析为 HTML，SSR 输出中无主题样式——**当前不成立**，但应作为不变量防住） | 静态审计（绕过形式未在浏览器实测）；`</style>` 部分代码 + SSR 输出确认 |
| SEC-09 | 中 | `crates/app/src/server/auth_routes.rs:103-115`；`crates/widgets/src/sanitize.rs:202` | 登出是 GET；不可信 Markdown 图片允许任意站内相对路径与外部 https。评论里放 `![](/api/auth/logout)` 即登出所有浏览者；外部图片泄露浏览者 IP | 静态审计 |
| SEC-10 | 中 | `crates/gateway/src/main.rs:45-47,196` | gateway 用 `insert_header` **覆盖**应用 CSP，且其 CSP 无 `'wasm-unsafe-eval'`、不允许内联启动脚本：部署在 gateway 后时 hydration 整体失效 | 静态审计 |
| SEC-11 | 中 | `crates/app/src/server/mod.rs:41-57`；`crates/core/src/engines/theme.rs:95` | `site_theme` cookie 读取时不校验（只有设置时校验）：`../`、绝对路径、`.wasm.bak` 均被接受；路径变体各成缓存键，可无鉴权撑大模块缓存 | 静态审计 |
| SEC-12 | 中 | `crates/modules/moderation/src/plugin_stage.rs:63,89-140`；`crates/sdk-macros`（输入解码失败返回空） | 审核插件任何错误 / 空输出 / 无法解析 → 放行（fail open） | 静态审计 |
| SEC-13 | 低 | `crates/modules/course/src/server.rs:1043` | `require_writer` 只信 JWT 里的角色，不回查数据库；降级用户在 JWT 过期前（7 天）仍可写 | 静态审计 |
| SEC-14 | 低-中 | `crates/core/src/lib.rs:78-80` | wasmi `StoreLimits` 未限制 table 元素 / 实例 / 表 / 内存数量，超大 table 可越过 8 MiB 内存上限 | 静态审计 |
| SEC-15 | 低-中 | `crates/modules/admin/src/server.rs:799-815`；`crates/core/src/lib.rs:422` | 管理端上传插件不跑 `verify_manifest_consistency`、不查锁（`scan_uploaded_plugin` 无调用方）；文档声称已拦截「capability 伪装」 | 静态审计 |
| SEC-16 | 低 | `crates/core/src/lib.rs:353`；`crates/app/src/server/mod.rs:163` | 主题调用失败被静默吞掉；i18n 错误向匿名调用方返回插件路径；`translate_server` 无鉴权且每次新建实例（放大成本） | 静态审计 |
| SEC-17 | 低 | `crates/widgets/src/mdx.rs:547-556` | 用户内容中的 mermaid 代码块会被渲染（mermaid 默认 strict，但属于额外攻击面） | 静态审计 |
| SEC-18 | 信息 | `auth_modal.rs:130`、`theme_picker.rs:91` | eval 字符串用 `'{}'` 直接包值（值目前受服务端 / 管理员控制，未构成注入）；E1 改 web-sys 后自然消除 | 代码确认 |
| SEC-19 | 信息 | `crates/app/src/server/security.rs:61-84` | 应用自身无 HSTS / Permissions-Policy；`connect-src ws: wss:` 允许任意主机 | 静态审计 |
| SEC-20 | 信息 | `crates/app/src/server/mod.rs:564` | 公开调试端点 `/api/echo`；`get_site_config` 公开审核配置 | 静态审计 |
| SEC-21 | 信息 | `crates/core/src/engines/content_transformer.rs` | content-transformer 尚无生产调用方；接入时输出必须走 Markdown 安全渲染路径，不得作为原始 HTML | 静态审计 |
| SEC-22 | 低 | `crates/sdk/src/lib.rs:111` | 插件输出为空时宿主调用 `dealloc(0, 0)`，SDK 里 `Vec::from_raw_parts(null…)` 属未定义行为（被沙箱隔离，但可能 trap）；宏在输入解码失败时静默返回空 | 静态审计 |

**审计确认没问题的部分**（不需要改）：Markdown 原始 HTML 输出为转义文本，链接 / 图片协议白名单正确；上传仅允许 png/jpg/gif/webp（魔数校验，无 SVG），带 nosniff；session cookie 为 HttpOnly + SameSite=Lax（https 时 Secure）；所有 server function 为 POST、无 CORS 层；登录 / 回调 / 登出只跳转固定路径（无开放重定向）；admin 与权益相关接口都经 `require_admin` 回查数据库；客户端未编入任何密钥；插件沙箱无宿主导入、有 fuel（1 亿 / 次）、8 MiB 内存与输出上限、5 秒超时、trap 被隔离。

## 3. 整改阶段

### 阶段 A：可被直接利用的漏洞（最先做）

| 任务 | 覆盖 | 做法 | 先红后绿的测试 |
| --- | --- | --- | --- |
| A1 | SEC-01 | ~~输出白名单过滤~~ → 实际做法：读 pulldown-latex 0.7.1 写出器确认，用户文本只经 `Content::Text/Number/Function` 与解析错误信息进入输出（属性值全是数值或固定串），故在交给它渲染前把这些字符串转义（`mdx.rs::latex_to_mathml_string`）。比事后解析 HTML 小一个数量级；升级 pulldown-latex 时测试 `assert_only_mathml_tags` 兜底。可信与不可信内容同一路径 | `widgets` 单测：`$\text{<img src=x/onerror=alert(1)>}$`、`\text{<a href="javascript:…">}`、`/onerror=` 形式输出中不含 `<img` / `onerror` / `javascript:`；常见公式（分式、上下标、矩阵）输出不变 |
| A2 | SEC-02 | 实际做法：`/courses` 仍是 ServeDir，前面加中间件 `guard_course_file`；课时目录（第 4 段起）内的任何文件按 `get_lesson` 同一规则（抽成 `lesson_access_allowed` 共用）判定，课程 / 章节目录下的文件（封面）公开；路径按 ServeDir 方式逐段解码，`..` / 解码出的 `/` `\` / 查不到课时或课程 / 查库失败一律 404。限制：封面放在课程目录下两层以上会被当作课时文件拒绝 | 集成测试：未登录请求课时 `index.md` / 媒体 → 401/403/404；封面 → 200 |
| A3 | SEC-03 | 路径拒绝 `..`、绝对路径、反斜杠；canonicalize 后必须以 docs 根目录为前缀（复用 forum 已有的同类函数）。实际做法：forum 的 `safe_join_under` 移到 `app_core::utils` 共用；docs 对完整的 `<path>/index.md(x)` 校验，文件本身是根外符号链接也拒绝 | 单测：`../courses/…`、`/etc`、`..\\x`、URL 编码形式 → 错误；正常文档路径 → 成功 |
| A4 | SEC-04 | profile 响应 `error_for_status()`；宿主拒绝空 / `"0"` 的 `external_id`；~~四个认证插件缺 `id` 时报错而非回退~~（未改：插件唯一的回退值就是 `"0"`，宿主已拒绝，改插件不改变行为却要重建 4 个 wasm；「不要伪造 ID」写进 D1 指南） | 单测：profile 返回 401 或缺 `id` → 登录失败且不创建 / 匹配账号 |

### 阶段 B：站点加固

| 任务 | 覆盖 | 做法 |
| --- | --- | --- |
| B1（即迁移计划 E1） | SEC-07、SEC-18 | 站点所有 `document::eval` 改为 web-sys 直接调用；CSP 保持不含 `'unsafe-eval'`。验收：严格 CSP 下首页、课程、论坛控制台无 EvalError、无 panic，主题插件生效，暗色切换可用 |
| B2 | SEC-09 | 登出改为 POST；不可信内容图片只允许 `/uploads/`（外部图片如需支持，改走代理） |
| B3 | SEC-08 | 主题 CSS 改为基于 tokenizer 的白名单：拒绝 `<`、`@import`、`image-set`；`url()` 仅允许 `data:image/` 与 `/assets/`；收紧 CSP `img-src`。测试覆盖审计列出的每种绕过形式 |
| B4 | SEC-11 | cookie 读取侧复用设置侧的校验（`[A-Za-z0-9_-]+\.wasm` 且在主题列表内） |
| B5 | SEC-10 | CSP 只由应用设置；gateway 不覆盖（或与应用共用同一份定义） |
| B6 | SEC-13 | `require_writer` 改用回查数据库的会话校验 |
| B7 | SEC-17 | 不可信内容中的 mermaid 作为普通代码块显示 |
| B8 | SEC-19、SEC-20 | 应用侧补 HSTS / Permissions-Policy；`ws:` 仅开发构建允许；删除 `/api/echo`；裁剪公开配置 DTO |

### 阶段 C：插件宿主加固

| 任务 | 覆盖 | 做法 |
| --- | --- | --- |
| C1 | SEC-05 | OAuth 端点由宿主配置固定（每个 provider 一份 https 白名单），插件只负责 profile 字段映射；`client_secret` 只发往宿主配置的端点 |
| C2 | SEC-06、SEC-15 | 所有插件加载（含 auth / moderation）统一经带锁的共享管理器；提供锁生成命令；增加严格模式（未登记插件拒绝加载，上线配置默认开启）；上传时执行 `scan_uploaded_plugin`，硬失败即拒绝 |
| C3 | SEC-12 | 审核插件失败可配置为「送人工复核」，默认 fail closed；失败计数入日志 / 指标 |
| C4 | SEC-14 | `StoreLimits` 补 `table_elements` / `instances` / `tables` / `memories` 上限，附超限模块的测试 |
| C5 | SEC-16 | 主题调用失败记日志；对外错误不含路径；翻译结果缓存，`translate_server` 限流 |
| C6 | SEC-22 | SDK 对空输出不调用 `dealloc`（或 `dealloc` 处理空指针）；宏在输入解码失败时返回明确错误 |

### 阶段 D：插件开发者安全指南

新增 `docs/PLUGIN_SECURITY.md`，并修正 `docs/PLUGIN_DEV.md`：

- **威胁模型**：沙箱只限制资源与 I/O，不判断输出是否正确；插件在其能力范围内是完全受信的。
- **按能力的输出契约**：输出去向（页面 `<style>`、OAuth 流程、身份映射、审核结论、Markdown）、宿主校验什么、不校验什么。
- **认证插件**：端点由宿主固定（C1 之后）；`external_id` 必须是稳定、非空的 provider uid，缺失时报错，不得伪造占位值（宿主自 A4 起拒绝空串与 `"0"`；内置四个认证插件源码仍是 `unwrap_or("0")`，做 D 时连同 wasm 一起改掉，免得被当成示例照抄）；不在输出中回显 token。
- **主题插件**：只用白名单内的 CSS 构造；不引用外部资源；不做覆盖页面、伪造界面的样式（主题对所有访客全局生效）。
- **审核插件**：用户内容是不可信输入，会进入提示词——要分隔、防提示词注入；无法判断时不得默认放行。
- **ABI 注意事项**：输入为 JSON；空输出的处理；panic 即该次调用被跳过；fuel / 内存 / 输出上限及其配置项。
- **供应链**：在自己的 CI 从源码构建；锁的生成与更新流程；`plugins/` 目录中不留 `.bak`；管理端上传插件等同于在认证流程中执行代码。
- **修正现有文档中言过其实的说法**：`PLUGIN_DEV.md` §12.1 关于 SHA-256 锁（实际可选、被 auth/moderation 绕过、无工具）、manifest 一致性（加载与上传时都未执行）、CSS 净化（可绕过）的描述；§10 的 Ed25519 签名尚未实现，标注为计划项。文档与实现以 C 阶段完成后的状态为准同步更新。

### 阶段 E：dioxus-ui（在 dioxus-ui 仓库修复）

见 [`DIOXUS_UI_FEEDBACK.md`](DIOXUS_UI_FEEDBACK.md) FB-02 ~ FB-12。站点迁移中依赖这些修复的组件（Dialog、Dropdown、Popover、Command、Sheet、NavigationMenu、Tabs 键盘导航，以及接收用户 URL 的 Avatar / Breadcrumb 等）在对应 FB 修复发版后再用。

## 4. CSP 路线图

| 级别 | 内容 | 状态 |
| --- | --- | --- |
| L1 | 不含 `'unsafe-eval'` | 当前 CSP 已是 L1；B1（站点）与 FB-02（dioxus-ui）完成后，站点在 L1 下可正常运行 |
| L2 | 脚本不含 `'unsafe-inline'`（nonce / hash） | **上线后下一阶段**。收益最大：内联事件处理器（SEC-01 一类载荷）被浏览器直接拒绝。站点侧：Prism 初始化脚本外置为文件；Dioxus hydration 启动脚本可用 hash；`window.initial_dioxus_hydration_data` 每请求不同，需要 nonce——Dioxus 0.7 不支持给 SSR 脚本加 nonce，需上游支持或 Axum 中间件改写 HTML。dioxus-ui 组件不输出 `<script>`，已满足 |
| L3 | 样式不含 `'unsafe-inline'` | 暂不做：收益小（样式注入危害有限），成本高（主题 `<style>` 注入、组件 `style` 属性、`document::Style` 均需改造）。重新评估条件：L2 完成且出现需要防 CSS 注入的场景 |

## 5. 顺序与依赖

- A1–A4 互不依赖，先做；每项一提交。
- B1 依赖 web-sys（Dioxus web 已间接依赖）；完成后 FB-02 的上游修复才有站点侧的验证环境（严格 CSP 下运行）。
- C1 与 A4 改同一模块，A4 先做（小改动），C1 在其基础上重构端点来源。
- D 在 C 完成后定稿（文档描述以实现为准）；草稿可提前写。
- dioxus-ui 迁移（U3 起）与本计划并行：不依赖 eval、不接收用户 URL 的组件（Button、Input、Textarea、Card、Badge、Table、NativeSelect、Spinner 等）不受影响。
