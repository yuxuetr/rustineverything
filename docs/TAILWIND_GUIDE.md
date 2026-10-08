# Tailwind CSS 使用指南

本文档是 **Rust in Everything** 项目的 Tailwind CSS 开发指南，涵盖构建流程、主题系统、在 Dioxus/Rust 中使用 Tailwind 的注意事项，以及常见问题解答。

通用的 Tailwind v4 语法规范请参考 `crates/app/tailwind.md`。

---

## 1. 构建流程

### 1.1 文件布局

```
crates/app/
├── tailwind-input.css       ← Tailwind 源配置（@import + @theme + @source）
├── package.json             ← npm 脚本（build / dev）
├── node_modules/            ← npm 依赖（gitignored）
├── tailwind.md              ← Tailwind v4 通用规范参考
└── assets/
    └── tailwind.css          ← 编译输出（Dioxus asset! 引用）

assets/
└── tailwind.css              ← 根目录副本（SoT，git 跟踪）
```

### 1.2 构建命令

```bash
# 进入 app crate 目录
cd crates/app

# 首次安装依赖
npm install

# 一次性编译
npm run build

# 开发 Watch 模式（修改 .rs 或 tailwind-input.css 自动重编译）
npm run dev
```

`npm run build` 等价于：

```bash
npx @tailwindcss/cli -i tailwind-input.css -o assets/tailwind.css
```

### 1.3 数据流

```
tailwind-input.css
    ↓  (npx @tailwindcss/cli v4)
crates/app/assets/tailwind.css     ← Dioxus asset!("/assets/tailwind.css") 引用
    ↓  (build.rs 反向同步，mtime 比较)
assets/tailwind.css                ← git 跟踪的 SoT
```

- `build.rs` 在 `cargo build` 时自动从 `assets/` → `crates/app/assets/` 同步所有静态资源
- 同时，如果 `crates/app/assets/tailwind.css` 比 `assets/tailwind.css` 更新，会反向回写
- **无需手动拷贝**

### 1.4 Release 构建

`dx build --release --package app` 打包 `crates/app/assets/` 下的文件。`tailwind-input.css`、`node_modules/` 不会进入 release 产物。

---

## 2. 主题系统

### 2.1 颜色来源：shadcn token 优先

颜色的唯一来源是 dioxus-shadcn 的语义 token（`--background`、`--foreground`、`--primary`、`--muted`、`--muted-foreground`、`--card`、`--popover`、`--border`、`--input`、`--ring`、`--accent`、`--secondary`、`--destructive`、`--success`、`--warning` 等）。默认值在 `tailwind-input.css`，主题插件运行时覆盖（亮色写在 `:root`，暗色写在 `.dark`）。新代码用 token 类名：

```rust
rsx! {
    div { class: "rounded-xl border border-border bg-card text-card-foreground",
        p { class: "text-sm text-muted-foreground", "说明文字" }
        a { class: "text-primary hover:underline", href: "/docs", "链接" }
    }
}
```

token 类名自动跟随主题与暗色模式，不需要写 `dark:` 变体。

### 2.2 旧色阶映射（存量代码）

在 `tailwind-input.css` 的 `@theme` 块中，项目做了两组颜色重映射：

```css
/* slate → stone（暖色调灰阶） */
--color-slate-*: var(--color-stone-*);

/* blue → orange（Rust 品牌色） */
--color-blue-*: var(--color-orange-*);
```

**意味着：**
- 代码中写 `bg-blue-600` 实际渲染为 **橙色**（orange-600）
- 代码中写 `text-slate-900` 实际渲染为 **石灰色**（stone-900）
- 如果需要真正的蓝色，使用 `sky-*`、`indigo-*` 或 `cyan-*`

存量代码里还有约 1100 处 `slate-*`。映射只改色阶，**不跟随主题**：主题的中性色 token 若取冷色，组件会是冷色，旁边手写的 `dark:bg-slate-900` 却是暖色 stone。所以内置主题的中性色都取 stone 色阶（ocean 自 2026-10-08 起，`crates/core/src/engines/theme.rs` 有测试守住）。新代码不要再写 `slate-*` / `blue-*`，改用 2.1 的 token。

### 2.3 主题

内置主题输出 2.1 的 token（规范见 `docs/THEME_SPEC.md`）。旧变量 `--color-primary` / `--color-bg` / `--color-surface` / `--color-text` / `--color-text-muted` / `--color-border` 在 `assets/css/main.css` 中是 token 的别名，保留给 `var(--color-*)` 写法。

### 2.4 深色模式

使用 class 策略（非 media query）：

```css
@variant dark (&:where(.dark, .dark *));
```

代码中始终先写 light 样式，再加 `dark:` 前缀：

```rust
rsx! {
    div { class: "bg-white text-slate-900 dark:bg-slate-950 dark:text-white", ... }
}
```

---

## 3. 在 Dioxus/Rust 中使用 Tailwind

### 3.1 基本用法

```rust
rsx! {
    div { class: "flex items-center gap-4 rounded-xl p-6 bg-white dark:bg-slate-900",
        h2 { class: "text-lg font-bold", "标题" }
    }
}
```

### 3.2 扫描范围（@source）

Tailwind v4 只生成它在源文件里扫描到的类名。`tailwind-input.css` 扫描三处：

```css
@import "./tailwind-sources.css";      /* dioxus-shadcn 组件源码，scripts/tw-sources.mjs 构建时生成 */
@source "../../crates/modules/";       /* 所有 module crate */
@source "../../crates/app/src/";
```

- 新增 module crate 放在 `crates/modules/` 下即可，**不用**再加 `@source` 行。此前只列了 6 个 crate，docs、search 与 5 个内容板块独有的类名一直没有生成（2026-10-08 修复）。
- 升级 dioxus-shadcn 后要重新 `npm run build`：`tailwind-sources.css` 里是 registry 中带版本号的源码路径。
- 在 `crates/` 之外（例如 `widgets`、`core`）写类名时，要在 `tailwind-input.css` 加对应的 `@source`。

### 3.3 动态类拼接的正确写法

```rust
// ✅ 正确：完整类名作为字符串字面量，Tailwind 能扫描到
fn badge_class(kind: &str) -> &'static str {
    match kind {
        "frontend" => "bg-violet-100 text-violet-700",
        "backend"  => "bg-sky-100 text-sky-700",
        _          => "bg-slate-100 text-slate-600",
    }
}

// ❌ 错误：拼接类名片段，Tailwind 扫描不到完整类名
fn badge_class(color: &str) -> String {
    format!("bg-{}-100 text-{}-700", color, color)
}
```

### 3.4 format_args! 与条件样式

Dioxus 中条件样式的常见模式：

```rust
rsx! {
    div {
        class: format_args!("px-4 py-2 rounded-lg {}",
            if active { "bg-blue-600 text-white" } else { "bg-slate-100 text-slate-600" }
        ),
        "按钮"
    }
}
```

### 3.5 内联动态属性

```rust
rsx! {
    div {
        class: "aspect-[16/9] {gradient} overflow-hidden",  // {gradient} 是变量插值
        ...
    }
}
```

Dioxus 的 `rsx!` 支持 `{variable}` 直接插入到 class 字符串中。确保变量值包含的是完整的 Tailwind 类名。

---

### 3.6 使用 dioxus-shadcn 组件

站点依赖 crates.io 上的 `dioxus-shadcn`（各 crate 在 `Cargo.toml` 按需开 feature）。迁移记录见 `docs/DIOXUS_UI_MIGRATION.md`，组件库问题见 `docs/DIOXUS_UI_FEEDBACK.md`（FB-NN）。

- **`class` 是合并，不是追加**：同一类 utility（如 `px-*`、`bg-*`）用户值替换组件默认值，其余保留。
- **链接做成按钮的样子**：用导出的类名函数，例如 `a { class: button_class(ButtonVariant::Outline, ButtonSize::Sm, UiDensity::Comfortable, "") }`；还有 `badge_class`、`card_class`、`tabs_*_class` 等。
- **密度**：默认 Comfortable 会给控件加最小高度；后台页包在 `DensityProvider { density: UiDensity::Compact }` 里。
- **边框颜色要显式写**：Tailwind v4 默认边框色是 `currentColor`，手写边框加 `border-border`。
- **严格 CSP**：站点不允许 `'unsafe-eval'`。不要用 `document::eval` 与 `document::Title`（`crates/app/tests/csp_no_eval.rs` 会拦下），浏览器操作走 `widgets::browser`，页面标题用 `PageTitle`。
- **弹层放在哪里**：带 `backdrop-filter` / `transform` / `filter` 的祖先会成为 `fixed` 子元素的定位容器。全屏弹层（Dialog、Sheet）不要渲染在导航栏 header 里，放在它之外。
- **已知的组件库问题与站点绕法**：
  - Dropdown 内容加 `fixed`（FB-19：在 flex 行里定位偏移）。
  - Dialog 里的 Command 只在打开时挂载：`if open() { Command { … } }`（FB-17）。
  - ToggleGroup 单选再点已选项会报空值，需要「总有一项选中」时受控并忽略空值。
  - 导航下拉用纯 CSS 的 `group-hover` / `group-focus-within`，不用 NavigationMenu（FB-20：hydration 前打不开）。

## 4. Tailwind v4 速查

本项目使用 **Tailwind CSS v4.1**，以下是最常踩的 v3 → v4 变更：

### 渐变

```rust
// ❌ v3 写法
"bg-gradient-to-br from-blue-500 to-indigo-600"

// ✅ v4 写法
"bg-linear-to-br from-blue-500 to-indigo-600"
```

### 阴影

| v3 | v4 |
|----|-----|
| `shadow-sm` | `shadow-xs` |
| `shadow` | `shadow-sm` |
| `shadow-md` | `shadow-md`（不变）|
| `shadow-lg` | `shadow-lg`（不变）|

### 圆角

| v3 | v4 |
|-----|------|
| `rounded-sm` | `rounded-xs` |
| `rounded` | `rounded-sm` |
| `rounded-md` | `rounded-md`（不变）|

### 其他

| v3 | v4 |
|-----|------|
| `outline-none` | `outline-hidden` |
| `ring` | `ring-3` |
| `blur-sm` | `blur-xs` |

完整列表见 `crates/app/tailwind.md`。

---

## 5. 项目约定

### 5.1 颜色使用约定

| 场景 | 新代码 | 存量写法 |
|------|-------|---------|
| 品牌主色 / CTA | `bg-primary` / `text-primary`，或 `Button` | `blue-*`（映射为 orange） |
| 成功 / 开源标签 | `text-success`、`Badge { variant: Success }` | `emerald-*` |
| 警告 | `text-warning`、`Badge { variant: Warning }` | `amber-*` |
| 错误 / 危险操作 | `text-destructive`、`ButtonVariant::Destructive` | `rose-*` / `red-*` |
| 中性文字 / 背景 / 边框 | `text-foreground`、`text-muted-foreground`、`bg-background`、`bg-card`、`bg-muted`、`border-border` | `slate-*`（映射为 stone） |
| 分类配色（真正的蓝等） | `sky-*` / `indigo-*` / `cyan-*` | 同左，绕过 blue→orange 映射 |

### 5.2 间距与布局

- 使用 `gap-*` 代替 `space-x-*` / `space-y-*`
- 使用 `aspect-video` 或 `aspect-[16/9]` 代替手动计算 padding hack
- 容器宽度用 `max-w-7xl` + `mx-auto` + `px-4 sm:px-6 lg:px-8`

### 5.3 响应式断点

| 前缀 | 最小宽度 |
|------|---------|
| `sm:` | 640px |
| `md:` | 768px |
| `lg:` | 1024px |
| `xl:` | 1280px |

本项目常见布局模式：

```rust
// 两列：移动端堆叠，桌面端侧边栏 + 主内容
"grid grid-cols-1 lg:grid-cols-[16rem_1fr] gap-8"

// 卡片网格：1列 → 2列 → 3列
"grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6"
```

---

## 6. 常见问题 (FAQ)

### Q1: 我添加了新的 Tailwind 类名但页面没有效果？

**原因：** Tailwind v4 JIT 模式只会生成它在源码中扫描到的类名。如果类名出现在 Tailwind 默认扫描路径之外的文件中，不会被包含进编译输出。

**解决：**
1. 确认类名拼写正确（注意 v4 重命名，如 `bg-linear-*` 非 `bg-gradient-*`）
2. 类名所在文件是否在扫描范围内（见 [3.2](#32-扫描范围source)）；`crates/modules/` 与 `crates/app/src/` 已覆盖
3. 重新运行 `cd crates/app && npm run build`
4. 确认输出的 `assets/tailwind.css` 中包含该类名：`grep 'your-class' assets/tailwind.css`

### Q2: `blue-600` 为什么渲染成橙色？

这是主题映射，见 [2.2 旧色阶映射](#22-旧色阶映射存量代码)。如果需要真正的蓝色，使用 `sky-*`、`indigo-*` 或 `cyan-*`。

### Q3: 深色模式切换不生效？

确认：
1. HTML 根元素上有 `class="dark"`（通过 JS 切换）
2. 使用的是 `dark:` 前缀而不是 `@media (prefers-color-scheme: dark)`
3. 类名中 light 样式在前，dark 在后

### Q4: `npm run build` 报 `command not found: tailwindcss`？

```bash
cd crates/app
npm install    # 确保 node_modules 已安装
npm run build  # 使用 npx 调用本地安装的 CLI
```

### Q5: `crates/app/assets/` 下的文件是什么？需要手动管理吗？

不需要。这个目录是 `build.rs` 从根目录 `assets/` 自动同步的镜像副本，已被 `.gitignore` 忽略。Dioxus 的 `asset!` 宏引用这里的文件。

如果你发现 `crates/app/assets/tailwind.css` 和 `assets/tailwind.css` 不一致，运行一次 `cargo build` 即可，`build.rs` 会自动同步。

### Q6: 如何给新模块的动态类名添加 Tailwind 支持？

1. 模块放在 `crates/modules/` 下即已被扫描；放在别处时在 `crates/app/tailwind-input.css` 加 `@source`
2. 确保 `.rs` 文件中的类名是**完整的字符串字面量**（不要用 `format!` 拼接类名片段）
3. 运行 `cd crates/app && npm run build`
4. 验证：`grep 'your-new-class' crates/app/assets/tailwind.css`

### Q7: Watch 模式下修改 `.rs` 文件会自动重编译 Tailwind 吗？

`npm run dev` 会 watch `tailwind-input.css` 中 `@source` 指定的路径。如果你修改了被 `@source` 涵盖的 `.rs` 文件中的类名，Tailwind CLI 会自动重编译。

但如果修改的文件不在扫描范围内（见 [3.2](#32-扫描范围source)），需要先添加 `@source` 指令。

### Q8: 为什么不把 Tailwind 配置放在项目根目录？

因为 Dioxus 的 `asset!` 宏固定解析 `crates/app/assets/` 路径下的文件。Tailwind 的输出必须直接落在这里，所以源文件和 npm 工具链与输出在同一目录下是最短路径，避免跨目录同步的复杂性。

---

## 7. 参考资料

- [Tailwind CSS v4 官方文档](https://tailwindcss.com/docs)
- [v4 升级指南](https://tailwindcss.com/docs/upgrade-guide)
- [v4 博客公告](https://tailwindcss.com/blog/tailwindcss-v4)
- 项目内 Tailwind 语法规范：`crates/app/tailwind.md`
