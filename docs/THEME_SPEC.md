# Theme Spec

> 站点外观由「主题」（颜色 token）与「布局」（页面骨架）描述，二者正交。
> 主题内置在宿主代码里（R2，2026-10-08 起不再是 WASM 插件）；默认值来自
> `site.json`，访客可用 cookie 覆盖。

## 1. 总体架构

```text
site.json "theme": "ocean"          访客 cookie site_theme=sunset（可选）
          │                                   │
          └──────────────┬────────────────────┘
                         ▼
   engines::theme::resolve_theme(default, cookie)
   （crates/core/src/engines/theme.rs；只认 THEMES 表内的 id）
                         │
                         ▼
   server fn get_aggregated_theme_css → Theme.css（include_str! 的 .css 文件）
                         │
                         ▼
   crates/app/src/main.rs：<style id="wasm-theme-style">{css}</style>
```

## 2. 内置主题

`crates/core/src/engines/theme.rs` 的 `THEMES` 表，顺序即切换菜单顺序，第一项是回退主题：

| id | CSS 文件 | 颜色基调 |
| --- | --- | --- |
| `ocean` | `crates/core/src/engines/themes/ocean.css` | 默认：stone 中性色 + 橙色主色 |
| `sunset` | `crates/core/src/engines/themes/sunset.css` | 暖色 |
| `catppuccin` | `crates/core/src/engines/themes/catppuccin.css` | Latte / Macchiato |

CSS 在编译期嵌入（`include_str!`），属于作者写的受信内容，原样输出，不经过白名单检查。

## 3. 访客 Cookie 覆盖

ThemePicker 选中主题后，`/api/theme/set` 写：

```
Cookie 名:  site_theme
值:         内置主题 id（如 sunset）
属性:       Path=/; Max-Age=31536000; SameSite=Lax（非 HttpOnly：前端也会写同一 cookie）
生产 HTTPS: 自动追加 Secure
```

`resolve_theme` 的规则：cookie 是表内 id → 用它；否则（缺失、空、未知值、旧的
`theme_*_plugin.wasm` 文件名、路径）记 warn 后忽略，用 site.json 的 `theme`；后者也不在表内时用
`THEMES[0]`。cookie 由浏览器提交、可被任意改写（SEC-11），设置与读取两侧都只认表内 id。

清空 cookie 等价于回到 site.json 默认。

## 4. ThemePicker 组件

`crates/app/src/components/theme_picker.rs`：

- server fn `list_available_themes` 返回 `THEMES` 全表，`is_active` 标出本次请求生效的主题。
- 用户切换 → `set_user_theme(id)` 写 cookie，前端同时写 `document.cookie`，再 bump `ThemeVersion` Signal。
- `crates/app/src/main.rs::App` 的 `theme_css` `use_resource` 依赖 `ThemeVersion`，切换后重新拉 CSS 并替换 `<style id="wasm-theme-style">`。

## 5. 新增主题

1. 在 `crates/core/src/engines/themes/` 新建 `<id>.css`，按 §8「Token 契约」在 `:root` 与 `.dark` 各给一套完整的 token；
2. 在 `THEMES` 表加一项 `Theme { id, label, css: include_str!("themes/<id>.css") }`；
3. `theme_ids_are_unique_and_every_theme_sets_light_and_dark_tokens` 测试会检查 id 唯一与两套 token 存在。

不要引用外部资源（字体、图片 URL）：CSP 的 `font-src` / `img-src` 不放行外部主机，主题对所有访客全局生效。

## 6. 布局 (Layout)

布局是「壳」（shell）级别的页面骨架，与主题（颜色变量）正交。

`SiteConfig.active_layout` 控制当前布局，默认 `"classic"`：

| 布局 | 文件 | 形态 |
| --- | --- | --- |
| `classic` | `crates/app/src/components/layouts/classic.rs::ClassicShell` | Navbar + 主导航 + 工具区 + Footer |
| `minimal` | `crates/app/src/components/layouts/minimal.rs::MinimalShell` | 紧凑顶部条 (无主导航 / 无 Footer)，写作 / 阅读优先 |

切换流程：

```text
site.json::active_layout = "minimal"
        │
        ▼
server fn get_active_layout() 返回 "minimal"
        │
        ▼
Navbar 分发组件 use_resource → 选择 MinimalShell
        │
        ▼
RSX 渲染对应壳，Outlet::<Route> 嵌入主内容
```

`Navbar` 在 `crates/app/src/components/nav.rs` 是 Routable layout 入口
（`#[layout(Navbar)]`），内部根据 server fn 动态选择 shell。
等待 server 返回前默认渲染 `ClassicShell` 避免闪烁。

## 7. 与 site.json 的集成

```jsonc
{
  "theme": "ocean",            // 内置主题 id，缺省为 "ocean"
  "active_layout": "classic"   // 或 "minimal"
}
```

- 改默认主题只需改 `theme`；访客的 cookie 选择优先于它。
- 切换布局**只需**改 `active_layout` 字段；运行时 server fn 立即生效。
- 旧字段 `themes` / `active_theme`（插件文件名）已不再读取。

## 8. Token 契约

主题输出 [shadcn/ui](https://ui.shadcn.com/docs/theming) 语义 token，站点组件（dioxus-shadcn）与手写类名都从这些变量取色。默认值定义在 `crates/app/tailwind-input.css`（站点品牌：stone 中性色 + orange 主色），主题 CSS 以 `<style id="wasm-theme-style">` 在其后注入，同名变量覆盖默认值。

| token | 用途 |
| --- | --- |
| `--background` / `--foreground` | 页面底色与正文 |
| `--card` / `--card-foreground` | 卡片、面板 |
| `--popover` / `--popover-foreground` | 弹层、下拉、对话框 |
| `--primary` / `--primary-foreground` | 主按钮、强调、当前导航项 |
| `--secondary` / `--secondary-foreground` | 次按钮 |
| `--muted` / `--muted-foreground` | 弱化底色与次要文字 |
| `--accent` / `--accent-foreground` | 悬停、选中项 |
| `--border` / `--input` / `--ring` | 边框、输入框边框、焦点环 |

主题应在 `:root` 与 `.dark` 各给一套完整的值（`.dark` 加在 `<html>` 上）。`--destructive` / `--success` / `--warning` / `--info`、`--chart-*`、`--sidebar-*` 可省略，沿用默认值。

**旧变量（兼容）**：`--color-primary` / `--color-bg` / `--color-surface` / `--color-text` / `--color-text-muted` / `--color-border` 在 `assets/css/main.css` 中定义为上表 token 的别名（`--color-bg: var(--background)` 等）。只覆盖这些别名的主题只影响使用 `var(--color-*)` 的手写类名，**组件不会跟随**；主题请输出 token。
