# dioxus-ui 实践反馈记录

> 在 rustineverything.app 迁移到 [`dioxus-shadcn`](https://github.com/yuxuetr/dioxus-ui) 的过程中（计划见 [`DIOXUS_UI_MIGRATION.md`](DIOXUS_UI_MIGRATION.md)），
> 记录组件库本身的问题与改进点，之后整理成 dioxus-ui 的 issue / RFC。

## 记录规则

- **只记组件库的问题**：bug、API 难用、文档缺失、SSR/hydration 行为、样式可移植性。本项目自己的问题不记在这里。
- **先最小复现，再记录**：尽量给出能在 dioxus-ui 的 `examples/` 里复现的最小 RSX。
- **临时处理要可追溯**：本项目里为某个问题写的临时处理，代码注释标注编号（`// dioxus-ui FB-03`），上游修复后搜索编号删除。
- **状态**：`open`（已记录）→ `reported`（已提 issue，附链接）→ `fixed x.y`（上游已发版）→ `removed`（本项目临时处理已删）。也可能是 `wontfix`（评估后认为是本项目的用法问题）。

## 条目模板

```markdown
### FB-NN 一句话标题

- 组件 / 版本：
- 发现于：U? — 文件:行号
- 现象：
- 复现：
- 影响：
- 本项目的临时处理：
- 建议的上游修复：
- 状态：open
```

## 条目

### FB-01 `@source` 绝对路径在 Docker / CI 构建中不可移植

- 组件 / 版本：`dioxus-shadcn-cli`（`dxui init`）0.6.0
- 发现于：迁移规划阶段（U1 前）
- 现象：`dxui init` 写入 `@source "<本机 ~/.cargo/registry 绝对路径>/dioxus-shadcn-0.6.0/src"`。本项目在 Docker 构建阶段和 CI 中编译 Tailwind，那里的 registry 路径不同或不存在，Tailwind 不报错，只是组件用到的类名不生成，表现为组件**没有样式**。
- 复现：在一台机器上 `dxui init`，把生成的 css 提交，在另一台机器或 Docker 中 `npx @tailwindcss/cli -i … -o …`，产物里没有 `bg-primary` 等组件类名。
- 影响：所有依赖 crate 而非复制源码的用户，只要 CSS 不是在本机编译，都会踩到；错误是静默的。
- 本项目的临时处理：`scripts/tw-sources.mjs` 在构建时用 `cargo metadata` 生成 `@source` 行（见迁移计划 D3）。
- 建议的上游修复：提供 `dxui css-sources`（只打印或写出当前解析到的 `@source` 行，供构建脚本调用），并在 README 的「Depend on the crate」一节说明 CI / Docker 场景；或者随 crate 附带一份预生成的类名清单（safelist）。
- 状态：open

### FB-02 交互组件依赖 `document::eval`，在不允许 `'unsafe-eval'` 的 CSP 下失效并触发 panic

- 组件 / 版本：`dioxus-shadcn` 0.6.0（`modal_focus`、`anchored_overlay`、`listbox`、`roving_group`、`navigation_menu`、`menubar`、`hover_open`、`dismiss_timer`、`media_query`、`checkbox`、`slider`、`resizable`、`sidebar`、`input_otp`、`theme_controller`，共 15 个文件）
- 发现于：U1 — 基线检查
- 现象：Dioxus web 端的 `document::eval` 用 `new Function` 执行 JS。站点 CSP 是 `script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'`（不含 `'unsafe-eval'`），`new Function` 被拒绝，wasm-bindgen 报「imported JS function that was not marked as `catch` threw an error」，随后在 `js-sys …/futures/task/singlethread.rs:142` **panic**。
- 复现：任意 Dioxus 0.7 web 应用，响应头加 `Content-Security-Policy: script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'`，触发一次 `document::eval`（本站的暗色模式切换按钮即可复现；组件库中 Dialog 打开时的焦点管理、Popover/Dropdown 定位、Tabs 的方向键导航都会走这条路径）。
- 影响：不允许 `'unsafe-eval'` 是常见的安全基线，在这种站点上弹层、菜单、键盘导航都不可用，且 panic 后 wasm 运行时状态不可预期。dioxus-ui 仓库里没有任何 CSP 配置或测试（`grep -ri content-security-policy` 无结果），浏览器测试（含 axe 审计）都在无 CSP 环境下跑，所以没发现。
- 本项目的临时处理：无。决定（2026-10-08）：CSP 保持不含 `'unsafe-eval'`，等上游修复；依赖 eval 的组件推迟迁移。本站自身的 `document::eval`（暗色切换、搜索模态 Escape 等）同样受影响，自 S1（2026-07-21）起即已失效（站点尚未上线，未影响用户），这是本项目自己的问题，见 SECURITY_REMEDIATION.md B1。
- 建议的上游修复：web 端把这些 JS 交互改为 `web-sys` / `wasm-bindgen` 直接调用（焦点、`getBoundingClientRect`、事件监听都有对应 API），`document::eval` 只留给 desktop / mobile；浏览器测试矩阵加一组「带严格 CSP」的运行；README 写明 CSP 要求。
- 状态：open

---

以下 FB-03 ~ FB-12 来自 2026-10-08 对 dioxus-shadcn 0.6.0 的只读安全审计（静态阅读代码，标「未运行验证」的条目修复前先复现）。审计结论：**未发现脚本注入路径**——所有 `document::eval` 脚本是编译期常量，运行时数据经 `eval.send()` 以 JSON 传入；组件与 CLI 模板中没有 `dangerous_inner_html`；`style` 只拼数值。下面是需要改进的地方。整体整改计划见 [`SECURITY_REMEDIATION.md`](SECURITY_REMEDIATION.md) 阶段 E。

### FB-03 接收 URL 的组件不检查协议，`javascript:` 链接可执行

- 组件 / 版本：0.6.0 — `avatar.rs:37-50`、`breadcrumb.rs:96-110`、`hover_card.rs:102-115`、`navigation_menu.rs:451-470`、`dock.rs:56-73`、`menu.rs:85-106`、`sidebar.rs:435-460`、`pagination.rs:203-238`
- 发现于：安全审计
- 现象：`href` / `src` 原样输出到 `a` / `img`。
- 复现：`BreadcrumbLink { href: "javascript:alert(document.cookie)" }`，点击即执行；`AvatarImage { src }` 传外部地址会泄露浏览者 IP。
- 影响：中。应用把用户资料里的链接、头像传给组件是常见用法，组件本身成为 XSS 入口。
- 本项目的临时处理：迁移时不把用户提交的 URL 传给这些组件，直到上游修复。
- 建议的上游修复：提供 `SafeUrl` 校验（trim + 小写后拒绝 `javascript:` / `vbscript:` / `data:`，`src` 允许 `data:image/*`），组件默认使用；不安全时不输出 `href`。文档说明。
- 状态：open

### FB-04 `theme_init_script` 用 Rust `{:?}` 把值拼进内联脚本

- 组件 / 版本：0.6.0 — `theme_controller.rs:113-117`（文档示例 `docs/components/theme-controller.md:78` 用于 `<head>` 内联脚本）
- 现象：`storage_key` 用 Debug 格式拼进 JS，这不是 JS / HTML 转义，`</script>` 可原样通过。
- 影响：低（`storage_key` 通常是常量），但 API 没有约束它必须是常量；另外内联脚本需要 CSP `'unsafe-inline'` 或 nonce / hash，文档未说明。
- 建议的上游修复：用 `serde_json::to_string` 并把 `<` 转成 `\u003c`，或限制 key 为 `[A-Za-z0-9_-]`；文档给出 CSP hash 用法。
- 状态：open

### FB-05 主题控制器把 localStorage 中任意值当作主题名

- 组件 / 版本：0.6.0 — `theme_controller.rs:58-85`、`Theme::parse`
- 现象：存储的任意字符串变成 `data-theme`，并以 `Theme::Preset(任意字符串)` 传给 `on_theme_change`。
- 影响：低。若应用用主题名拼样式表路径或 URL，就成了注入点。
- 建议的上游修复：按已知主题列表校验，或在文档中标明该值是不可信输入。
- 状态：open

### FB-06 禁用的 `NavigationMenuLink` 仍保留 `href`

- 组件 / 版本：0.6.0 — `navigation_menu.rs:451-470`
- 现象：禁用只靠 CSS `pointer-events:none` 与 `aria-disabled`；键盘回车、鼠标中键仍可跳转。Menu / Sidebar / Pagination 禁用时会去掉 `href`，行为不一致。
- 影响：低。应用若用禁用状态挡住未付费功能或无权访问的路由，会被绕过。
- 建议的上游修复：`href: (!disabled).then_some(href)`，与其他组件一致。
- 状态：open

### FB-07 Escape / 外部点击监听挂在 document 上，没有层级判断

- 组件 / 版本：0.6.0 — `anchored_overlay.rs:77-87`、`dialog.rs:135`
- 现象：没有「最上层才处理」的判断，也不阻止冒泡。
- 影响：低（未运行验证）。一次 Escape 可能同时关闭弹层和外层对话框，确认类对话框可能被意外关闭。
- 建议的上游修复：维护弹层栈，只有最上层响应关闭。
- 状态：open

### FB-08 Sidebar 快捷键在输入框里也会拦截

- 组件 / 版本：0.6.0 — `sidebar.rs:26-31`
- 现象：`window` 上的 keydown 对 Ctrl/Cmd+键 调用 `preventDefault()`，焦点在输入框 / contenteditable 时也会。
- 影响：低。编辑器里 Ctrl+B（加粗）被抢走；多个 Sidebar 会同时切换。
- 建议的上游修复：目标是可编辑元素时跳过；文档写明快捷键。
- 状态：open

### FB-09 `Button` 默认没有 `type`，在表单里会提交

- 组件 / 版本：0.6.0 — `button.rs:96-120`（文档已说明）
- 影响：信息。表单里的「取消」按钮会提交表单，例如删除确认表单。
- 建议的上游修复：考虑默认 `type="button"`（破坏性变更，需版本说明）。
- 状态：open

### FB-10 文档没有安全说明

- 组件 / 版本：0.6.0 — README、`docs/`、crate 文档均无安全 / CSP 章节（只有 `bubble.md:40` 提到富内容净化由应用负责）
- 建议补充的内容：
  1. CSP 要求：哪些组件需要什么指令、缺少时的表现、哪些组件不依赖 eval（FB-02 修复后改写）；`theme_init_script` 需要 nonce / hash；SSR 输出的 `style` 属性需要 `style-src 'unsafe-inline'` 或 `style-src-attr`。
  2. URL：`href` / `src` 的处理方式（FB-03 修复前：原样透传，不可信 URL 须自行校验）。
  3. 属性透传：使用者传入的属性会覆盖组件自身的 `type` / `href` / `role` / `aria-*`；自行加 `target="_blank"` 时要加 `rel="noopener noreferrer"`。
  4. 输入校验：FileInput 的 `accept`、OTP 过滤、NumberInput 解析只是 UX，服务端必须校验。
  5. 主题存储值是不可信输入，`storage_key` 必须是常量。
  6. 全局行为：document / window 监听器、`data-dxui-*` 属性、`<html>` 上的滚动锁属性（`modal_focus.rs:14-36`）、Sidebar 快捷键。
  7. 防点击劫持（`frame-ancestors` / `X-Frame-Options`）由应用负责。
- 状态：open

### FB-11 `dxui` CLI 写文件时跟随符号链接

- 组件 / 版本：dioxus-shadcn-cli 0.6.0 — `main.rs:742-750,843-870`
- 现象：组件名必须在内置清单中、目标路径来自内置 JSON，无路径穿越；不加 `--overwrite` 不覆盖。但 `fs::write` 跟随符号链接。
- 影响：信息。仓库里被放置的符号链接可以把写入重定向到别处。
- 建议的上游修复：目标是符号链接时拒绝写入。
- 状态：open

### FB-12 crate 与 CLI 模板靠手工保持同步

- 组件 / 版本：0.6.0 — 源码中的「Keep in sync」注释
- 现象：`dioxus-shadcn` 源码与 CLI 模板副本手工同步；已经用 `dxui add` 复制了模板的应用拿不到安全修复。
- 影响：信息。安全修复的传播依赖使用者自己 `dxui diff`。
- 建议的上游修复：CI 加漂移检查；安全修复在 changelog 单独标注，提示复制源码的用户更新。
- 状态：open
