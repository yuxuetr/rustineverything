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

### 2.1 颜色映射

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

### 2.2 WASM 主题插件

主题色由 WASM 插件在运行时通过 CSS 变量覆盖（`--color-primary`、`--color-bg` 等）。`tailwind-input.css` 中的 `@theme` 块定义了默认值。

### 2.3 深色模式

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

### 3.2 动态类名与 @source

Tailwind v4 通过扫描源文件提取类名。当类名在 Rust 的 `match` 或函数返回值中动态拼接时，Tailwind 默认扫描路径可能找不到它们。

**解决方案：** 在 `tailwind-input.css` 中添加 `@source` 指令：

```css
@source "../../crates/modules/cases/src/";
```

这告诉 Tailwind CLI 额外扫描 cases 模块的 `.rs` 文件。

**规则：** 如果你新增了一个模块并在其中使用了项目其他地方没出现过的 Tailwind 类名（如新的颜色 `rose-100`），必须在 `tailwind-input.css` 中添加对应的 `@source` 行，然后重新 `npm run build`。

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

| 场景 | 推荐色系 | 说明 |
|------|---------|------|
| 品牌主色 / CTA 按钮 | `blue-*`（映射为 orange） | Rust 品牌橙色 |
| 成功 / 开源标签 | `emerald-*` | |
| 警告 | `amber-*` | |
| 错误 / 精选标记 | `rose-*` | |
| 中性文字 / 背景 | `slate-*`（映射为 stone） | 暖灰色调 |
| 真正的蓝色（非橙色） | `sky-*` / `indigo-*` / `cyan-*` | 绕过 blue→orange 映射 |

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
2. 如果是新模块，在 `crates/app/tailwind-input.css` 中添加 `@source` 指令
3. 重新运行 `cd crates/app && npm run build`
4. 确认输出的 `assets/tailwind.css` 中包含该类名：`grep 'your-class' assets/tailwind.css`

### Q2: `blue-600` 为什么渲染成橙色？

这是主题映射，见 [2.1 颜色映射](#21-颜色映射)。如果需要真正的蓝色，使用 `sky-*`、`indigo-*` 或 `cyan-*`。

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

1. 在 `crates/app/tailwind-input.css` 中添加：
   ```css
   @source "../../crates/modules/your-module/src/";
   ```
2. 确保 `.rs` 文件中的类名是**完整的字符串字面量**（不要用 `format!` 拼接类名片段）
3. 运行 `cd crates/app && npm run build`
4. 验证：`grep 'your-new-class' crates/app/assets/tailwind.css`

### Q7: Watch 模式下修改 `.rs` 文件会自动重编译 Tailwind 吗？

`npm run dev` 会 watch `tailwind-input.css` 中 `@source` 指定的路径。如果你修改了被 `@source` 涵盖的 `.rs` 文件中的类名，Tailwind CLI 会自动重编译。

但如果修改的模块不在 `@source` 列表中，需要先添加 `@source` 指令。

### Q8: 为什么不把 Tailwind 配置放在项目根目录？

因为 Dioxus 的 `asset!` 宏固定解析 `crates/app/assets/` 路径下的文件。Tailwind 的输出必须直接落在这里，所以源文件和 npm 工具链与输出在同一目录下是最短路径，避免跨目录同步的复杂性。

---

## 7. 参考资料

- [Tailwind CSS v4 官方文档](https://tailwindcss.com/docs)
- [v4 升级指南](https://tailwindcss.com/docs/upgrade-guide)
- [v4 博客公告](https://tailwindcss.com/blog/tailwindcss-v4)
- 项目内 Tailwind 语法规范：`crates/app/tailwind.md`
