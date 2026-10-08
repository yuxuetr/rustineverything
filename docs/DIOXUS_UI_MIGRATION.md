# dioxus-ui 迁移计划

> 分支：`feat/dioxus-ui-migration`。组件库：[`dioxus-shadcn`](https://github.com/yuxuetr/dioxus-ui) 0.6（自研，crates.io 已发布）。
> 迁移中遇到的组件库问题记入 [`DIOXUS_UI_FEEDBACK.md`](DIOXUS_UI_FEEDBACK.md)，作为 dioxus-ui 的实践检验回流上游。

## 1. 目标与非目标

**目标**

1. 交互控件（按钮、表单、表格、弹层、菜单）改用 dioxus-shadcn 组件，去掉手写的 Escape / 遮罩点击 / 焦点处理与重复类名串。
2. 颜色由 shadcn 语义 token（`--background` / `--primary` / `--border` …）统一驱动，主题插件改写 token，组件与手写类名同步换肤。
3. 用一个真实的全栈 SSR 站点检验 dioxus-ui：API 是否顺手、SSR/hydration 是否一致、样式是否可移植。

**非目标**

- 不改视觉设计：迁移后页面应与迁移前**看起来基本一致**（允许统一圆角、焦点环这类组件自带的细节）。
- 不迁移正文渲染：Markdown/MDX 正文继续用 `@tailwindcss/typography` 的 `prose`；`widgets/src/mdx.rs` 内嵌组件只在有对应控件时替换。
- 不顺带重构：5 个内容板块 crate（ai/cli/embedded/wasm/web3）的合并去重仍不在范围内。
- 不做纯装饰替换：`hero`、`home_sections` 的版式类名、`.btn-flow` / `.text-flow` 特效保留。

## 2. 现状盘点（2026-10-08）

含 `rsx!` 的文件 36 个，约 1.2 万行，`class:` 约 1400 处。原生控件：`button` 84、`input` 22、`textarea` 5、`select` 1、`table` 6；手写模态 3 处、手写下拉 4 处、`animate-spin` 加载圈约 40 处。

| 区域 | 文件 | 主要可替换项 |
| --- | --- | --- |
| admin | `modules/admin/src/admin.rs`（1377 行） | 11 button、6 input、2 textarea、1 select、`tab_btn` 手写页签 |
| admin（app 层） | `app/src/components/admin_entitlements.rs` | 5 button、4 input、3 table |
| course | `modules/course/src/course.rs`、`pay_ui.rs` | 12 button、1 table、支付模态（`fixed inset-0`） |
| forum / 评论 | `modules/forum/src/forum.rs`、`app/src/components/comment.rs` | 10 button、3 input、3 textarea |
| cases / docs / podcast / search | 各 1–2 文件 | 按钮、加载圈；search 的搜索模态（手写 Escape） |
| 5 个内容板块 | `modules/{ai,cli,embedded,wasm,web3}` 各 238 行，结构几乎相同 | 2 button、1 input、3 加载圈 / 每个 |
| 全局外壳 | `layouts/classic.rs`、`minimal.rs`、`theme_picker.rs`、`lang_picker.rs`、`auth_modal.rs`、`ecosystem_menu.rs` | 用户菜单、主题/语言下拉、移动端抽屉、登录模态、生态 mega 菜单 |

## 3. 关键决策

### D1 引入方式：crate 依赖，不复制源码

- 根 `Cargo.toml` 的 `[workspace.dependencies]` 加 `dioxus-shadcn = "0.6"`（不开 feature）；各 crate 用 `dioxus-shadcn = { workspace = true, features = [...] }` 只开自己用到的组件。
- 理由：组件库是自研的，走依赖时这个项目同时在检验库的公开 API，问题在上游修，不在副本里打补丁。
- `dioxus-shadcn` 依赖 `dioxus = "0.7"`，与本仓库锁定的 0.7.9 兼容，**不需要升级 dioxus**，Dockerfile 里 dx CLI 与 Cargo.lock 版本一致的约束不受影响。
- 需要验证上游未发布的修复时，临时用 `[patch.crates-io] dioxus-shadcn = { path = "../../arch/dioxus-ui/crates/dioxus-shadcn" }`，**不提交**这段 patch（Docker/CI 里没有该路径）。

### D2 Token 体系：shadcn token 是唯一来源，旧变量变别名

现状：主题插件运行时写 `:root { --color-primary; --color-bg; --color-surface; --color-text; --color-text-muted; --color-border }`，`tailwind-input.css` 的 `@theme` 也定义了同名变量。dioxus-shadcn 的样式用 `@theme inline { --color-primary: var(--primary); --color-border: var(--border) … }`。

冲突点：

- `--color-primary` / `--color-border` 两边同名，但语义不同（一个是值，一个是对 `--primary` 的引用）。
- `@theme inline` 生成的工具类直接内联 `var(--primary)`，插件改 `--color-primary` **不会**影响 `bg-primary`；插件也不提供 `--primary-foreground`、`--ring`、`--muted` 等组件需要的 token。

做法：

1. `tailwind-input.css` 引入完整的 shadcn token（`:root` / `.dark` + `@theme inline`），**默认值取站点现有配色**：中性色取 stone（现在 slate→stone 的映射），主色取 orange（现在 blue→orange 的映射），保证不装主题插件时外观不变。
2. 旧变量改成别名：`--color-bg: var(--background)`、`--color-surface: var(--card)`、`--color-text: var(--foreground)`、`--color-text-muted: var(--muted-foreground)`；`--color-primary` / `--color-border` 由 `@theme inline` 统一指向 `var(--primary)` / `var(--border)`，现有 `text-[var(--color-primary)]` 写法继续有效。
3. 三个主题插件（ocean / sunset / catppuccin）改为输出 shadcn token（至少 `--background --foreground --card --primary --primary-foreground --muted --muted-foreground --accent --border --input --ring`，亮暗两套），重编 wasm。`plugin_security` 是黑名单过滤，新变量名不受影响。
4. slate→stone、blue→orange 的色阶映射暂时保留，服务尚未迁移的手写类名；收尾阶段（U10）再评估能否删除。

**实验结论（U1）**：`@theme inline` 下 `bg-primary` 直接编译成 `background-color: var(--primary)`；`--color-primary: var(--primary)` 仍会输出，但位于 `@layer theme` 的 `:root` 中。分层规则输给未分层的 `main.css` 与插件 `<style>`，所以 U1 里 49 处 `var(--color-primary)` 继续取插件值（ocean 蓝），外观不变。U1 因此**只加 token、不改别名**；别名切换随 U2 插件改造一起做。站点原 `@theme` 里的 `--color-primary` / `--color-border` 默认值已删除（与组件库映射重名），其余旧变量的默认值仍在 `main.css`。

**U2 落地**：插件输出 token；旧变量在 `main.css` 里变为别名。决定（2026-10-08）：ocean 主色保持品牌橙，全站一个主色；sunset / catppuccin 保留各自主色。

### D7 CSP 与 `document::eval`（2026-10-08 决定）

CSP 保持不含 `'unsafe-eval'`。组件库在 web 端改用 web-sys（FB-02，上游修复）；站点自身的 `document::eval` 同样改掉（E1）。依赖 eval 的组件（Dialog、Dropdown、Popover、Command、Sheet、NavigationMenu，以及 Tabs / ToggleGroup 的键盘导航）要等上游发版后再迁移：U4 的支付模态、U8、U9 排在上游修复之后；U3 若用 Tabs，先验证点击路径不触发 eval。

### D3 Tailwind 扫描组件库源码：构建时生成，不写死路径

`dxui init` 生成的 `@source "/Users/…/.cargo/registry/src/…/dioxus-shadcn-0.6.0/src"` 是本机绝对路径；本项目的 Tailwind 在 Docker 构建阶段和 CI 里编译，写死路径会让组件类名**静默丢失**（不报错，只是没样式）。

做法：新增 `scripts/tw-sources.mjs`（Node 脚本：Docker builder 里已有 node，不必再装 jq），用 `cargo metadata` 查出 `dioxus-shadcn` 的 `manifest_path`，生成 `crates/app/tailwind-sources.css`（gitignore）写入 `@source` 行；`tailwind-input.css` 引入该文件；`package.json` 的 `build` / `dev` 先跑脚本。Dockerfile 在 `npm run build` 之前已 COPY `scripts/` 和全部 manifest，无需改动；CI 不编译 Tailwind。反向检查（U1 已验证）：依赖图里找不到 crate 时脚本退出 1；生成文件缺失时 Tailwind CLI 退出 1。

（这是 dioxus-ui 的第一条反馈，见 FB-01。）

### D4 路由 `Link` 用类名函数，不包组件

按钮外观的跳转链接（`Link { class: "…bg-orange-600…" }`）用 `button_class(variant, size, UiDensity::Comfortable, extra)` / `badge_class(...)` 生成类名，不为 `Link` 再写一层包装组件。如果发现这种写法需要的样板过多（例如每处都要显式给 density），记为组件库反馈。

### D5 纯 CSS 的生态菜单先评估再换

`ecosystem_menu.rs` 是有意做成纯 CSS（`group-hover` / `group-focus-within`），没有 signal，hydration 之前就能展开。`NavigationMenu` 是受控组件，hydration 前可能无法展开。U9 先对比两者在 **SSR 首屏 + JS 未加载** 时的行为，没有优势就保留现状，结论写进 FB 记录。

### D6 SSR / hydration 一致性是每个任务的硬验收

本站是 fullstack SSR。每迁一个页面都要检查：SSR 首屏 HTML 含组件结构，浏览器控制台没有 hydration mismatch 警告，弹层组件首屏默认关闭且不闪烁。

## 4. 任务拆分

一个任务一次提交（`refactor(ui): …` / `feat(ui): …`），提交后勾选 Todos.md。顺序：先基础设施，再低风险的 admin 表单，然后逐步扩大到面向读者的页面，全局外壳最后动。

| 任务 | 范围 | 用到的组件 | 验收要点 |
| --- | --- | --- | --- |
| **U0** | 本计划 + 反馈记录 + Todos.md | — | 文档评审通过 |
| **U1** | 依赖接入、token、D3 的 `@source` 生成、Dockerfile/CI 同步 | — | 不装主题插件时首页、课程页、admin 亮/暗截图与迁移前一致；Docker 镜像内 `tailwind.css` 含 `bg-primary`；D2 待实验结论回写 |
| **U2** | 三个主题插件 token 化 + `build_themes.sh` 重编 + THEME_SPEC 更新 | — | 三主题亮/暗切换，组件主色、边框、正文颜色都跟随；`plugin_security` 测试通过 |
| **U3** | admin：`admin.rs` + `admin_entitlements.rs` | Button、Input、Textarea、NativeSelect、Tabs（或 ToggleGroup）、Table、Spinner、Badge | admin 各页操作可用（审核、授权、退款按钮）；表单提交行为不变 |
| **U4** | course + 支付：`course.rs`、`pay_ui.rs` | Card、Button、Badge、Table、**Dialog**（支付模态，第一个弹层） | 支付模态 Escape / 遮罩 / 焦点回归正确；SSR 首屏无模态闪烁；`--no-default-features` 编译通过 |
| **U5** | forum + 评论：`forum.rs`、`comment.rs` | Button、Input、Textarea、Card、Empty、Alert | 发帖、回复、评论流程可用 |
| **U6** | cases、docs、podcast、search 列表部分 | Card、Badge、Button、Spinner、Empty、Pagination（如适用） | 列表与详情页 SSR 首屏含正文 |
| **U7** | 5 个内容板块：先迁 `ai.rs`，确认后机械套用到其余 4 个 | Button、Input、Spinner、Card | 5 个页面截图一致 |
| **U8** | 全局弹层：`auth_modal` → Dialog；search 模态 → Dialog + Command；`theme_picker` / `lang_picker` / 用户菜单 → Dropdown；移动端菜单 → Sheet | Dialog、Command、Dropdown、Sheet | 删除手写 Escape JS（`search.rs:77`）；键盘全程可操作；移动宽度（375px）抽屉正常 |
| **U9** | 生态 mega 菜单（D5 评估） | NavigationMenu（视评估） | 评估结论入 FB；不劣于现状才替换 |
| **U10** | 收尾：删除无用类名与 CSS、评估移除色阶映射、更新 TAILWIND_GUIDE、反馈汇总 | — | 全量校验命令通过；FB 汇总可直接转成 dioxus-ui 的 issue |

### 每个任务的校验

- `cargo fmt`；`CARGO_TARGET_DIR=/Users/hal/.target cargo clippy --workspace --features server --all-targets -- -D warnings`
- `CARGO_TARGET_DIR=/Users/hal/.target cargo test --features server --workspace -- --test-threads=1`
- `cd crates/app && npm run build`（新类名需要重新编译 CSS）后 `dx serve`；改了 `.ftl` 或 SSR 缓存异常时重启
- 浏览器（Playwright）：涉及页面的亮/暗截图与迁移前对比；控制台无 hydration 警告；弹层与表单做一次键盘操作
- SSR 烟测：`curl -s http://127.0.0.1:8080/<路由>` 断言首屏 HTML 含正文

## 5. 风险

| 风险 | 应对 |
| --- | --- |
| Token 默认值与现有配色有偏差，整站色调变化 | U1 用截图逐页对比；偏差在 token 层调，不在组件上覆盖类名 |
| 组件默认尺寸/间距（density、`min-h-10`）与原设计不同，页面变挤或变松 | 优先用组件的 `size` / `class` 合并能力调整；需要大量覆盖时记 FB |
| 受控弹层在 hydration 前不可用 | 首屏默认关闭即可接受；若是首屏就需要的交互（如生态菜单），按 D5 处理 |
| 上游修复需要发版才能用 | 本地 `[patch]` 验证、不提交；本项目在 FB 记录里注明「待 dioxus-shadcn x.y」并保留临时处理 |
