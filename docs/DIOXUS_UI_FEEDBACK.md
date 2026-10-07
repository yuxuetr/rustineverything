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
- 本项目的临时处理：待定（需要决定：CSP 放开 `'unsafe-eval'`，或等上游改造）。本站自身的 `document::eval`（暗色切换、搜索模态 Escape 等）同样受影响，自 S1（2026-07-21）起在生产环境失效，这是本项目自己的问题，另行处理。
- 建议的上游修复：web 端把这些 JS 交互改为 `web-sys` / `wasm-bindgen` 直接调用（焦点、`getBoundingClientRect`、事件监听都有对应 API），`document::eval` 只留给 desktop / mobile；浏览器测试矩阵加一组「带严格 CSP」的运行；README 写明 CSP 要求。
- 状态：open
