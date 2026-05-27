# Theme Spec

> 适用阶段：Phase 3.1 ~ 3.3（v2.1 Todos.md）。
> 站点的视觉外观完全由「主题栈」与「布局」描述，二者均可通过 `site.json`
> 配置切换，且支持用户级 cookie 覆盖。

## 1. 总体架构

```text
┌─────────────────────────────────────────────────────────┐
│                site.json (themes / active_theme /        │
│                  active_layout)                          │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              ThemeEngine + LayoutEngine                  │
│  (crates/core/src/engines/theme.rs / layout.rs)         │
│                                                          │
│  · theme_stack() = [base, ocean, sunset, ...]            │
│  · 末项为最高优先级 (覆盖前面)                            │
│  · theme_with_override(stack, cookie) 应用用户覆盖        │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│            PluginEngine (wasmi)                          │
│  · 调用 theme_*_plugin.wasm 的 get_theme_css()           │
│  · 串接所有 CSS 文本 → 注入 <style id="wasm-theme-style">│
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│   crates/app/src/main.rs (Dioxus RSX)                    │
│   document::Style { id: "wasm-theme-style", "{css}" }    │
└─────────────────────────────────────────────────────────┘
```

## 2. 主题栈

`SiteConfig.themes` 是一个有序数组：

```json
{
  "themes": ["theme_ocean_plugin.wasm", "theme_sunset_plugin.wasm"]
}
```

语义：
- 数组靠后 = 优先级越高（CSS 后写覆盖前面同名变量）。
- 元素为相对 `assets/plugins/` 的 wasm 文件名。
- 当数组为空时回退至 `active_theme` 单插件字段（向后兼容老 site.json）。

读取入口：

```rust
let stack: Vec<String> = config.theme_stack();
```

定义在 `crates/core/src/settings.rs::SiteConfig::theme_stack`。

## 3. ThemeEngine

`crates/core/src/engines/theme.rs`。封装 [`PluginEngine`]，提供：

| API | 说明 |
| --- | --- |
| `register_theme(path)` | 注册单个主题插件路径（init 阶段填充） |
| `set_themes(paths)` | 批量替换栈 |
| `apply_site_config(&SiteConfig)` | 从 `theme_stack()` 装载栈 |
| `aggregate_css()` | 顺序调用所有主题的 `get_theme_css` 并串接 |

`Engine::init` 阶段 ThemeEngine 自动 `apply_site_config`，无需手动调用。

## 4. 用户级 Cookie 覆盖

每个用户可以在 Navbar 的 ThemePicker 下拉中切换主题，选中后写
`Set-Cookie: site_theme=<filename>.wasm`：

```
Cookie 名:  site_theme
属性:       HttpOnly; Path=/; Max-Age=31536000; SameSite=Lax
生产 HTTPS: 自动追加 Secure
```

覆盖规则由纯函数 [`theme_with_override`](../crates/core/src/engines/theme.rs) 实现：

1. 取主题栈
2. 若 cookie 存在且对应 wasm 文件在 `assets/plugins/` 中存在
3. 用 cookie 主题**替换栈最后一项**（不破坏前置基础主题）

清空 cookie 等价于回到 site.json 默认。

## 5. ThemePicker 组件

`crates/app/src/components/theme_picker.rs`。下拉列表数据来源：

- server fn `list_available_themes` 扫 `assets/plugins/*.wasm`，读 manifest（capability=`THEME`）。
- 不可读 manifest 的老插件以「文件名包含 `theme`」启发式包括。
- 用户切换 → 调 `set_user_theme(filename)` 写 cookie + bump `ThemeVersion` Signal。
- `crates/app/src/main.rs::App` 的 `theme_css` `use_resource` 依赖 `ThemeVersion`，因此切换会重新拉聚合 CSS 并替换 `<style id="wasm-theme-style">`。

## 6. 主题插件实现

每个主题插件是一个 `cdylib`，导出至少：

| 导出函数 | 签名 | 说明 |
| --- | --- | --- |
| `alloc / dealloc` | `(i32) -> i32` / `(i32, i32)` | SDK 提供，标准内存分配 |
| `get_theme_css` | `(i32, i32) -> u64` | 返回主题 CSS 字符串（任意输入） |
| `get_manifest`  | `(i32, i32) -> u64` | 返回插件 manifest JSON，capability 必含 `"theme"` |

最小可行结构：

```rust
use rustineverything_sdk::{pack_output, capabilities, PluginManifest};

#[no_mangle]
pub extern "C" fn get_theme_css(_ptr: i32, _len: i32) -> u64 {
    let css = r#"
        :root {
            --color-primary: #ff8a00;
            --color-bg: #fff8f3;
            --color-text: #1a1a1a;
        }
        .dark {
            --color-bg: #1a0c00;
            --color-text: #fff8f0;
        }
    "#;
    pack_output(css)
}

#[no_mangle]
pub extern "C" fn get_manifest(_ptr: i32, _len: i32) -> u64 {
    let m = PluginManifest::new("theme-sunset", "Sunset", "0.1.0")
        .with_capability(capabilities::THEME);
    rustineverything_sdk::pack_json(&m)
}
```

已交付的主题：

| ID | Path | 颜色基调 |
| --- | --- | --- |
| `theme-ocean` | `assets/plugins/theme_ocean_plugin.wasm` | 蓝绿主色 |
| `theme-sunset` | `assets/plugins/theme_sunset_plugin.wasm` | 暖色 (light/dark) |
| `theme-catppuccin` | `assets/plugins/theme_catppuccin_plugin.wasm` | Latte/Macchiato |

## 7. 构建脚本

```bash
./scripts/build_themes.sh             # 构建全部 3 个主题
./scripts/build_themes.sh sunset      # 仅构建 sunset
```

脚本会：

1. 自动安装 `wasm32-unknown-unknown` target（如缺失）
2. 用 `CARGO_TARGET_DIR=/Users/hal/.target` 编译
3. 拷贝 `*.wasm` 到 `assets/plugins/theme_<id>_plugin.wasm`

## 8. 布局 (Layout)

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

## 9. 与 site.json 的集成示例

```jsonc
{
  "themes": ["theme_ocean_plugin.wasm", "theme_sunset_plugin.wasm"],
  "active_layout": "classic",   // 或 "minimal"

  // 老字段（向后兼容）
  "active_theme": "theme_ocean_plugin.wasm"
}
```

- 当 `themes` 非空时，`active_theme` 字段被忽略。
- 切换布局**只需**改 `active_layout` 字段；运行时 server fn 立即生效。
- 切换主题**栈**改 `themes` 字段；用户级覆盖由 cookie 完成，不破坏栈基础项。

## 10. 测试覆盖

- `crates/core/src/settings.rs`：5 个 SiteConfig 单测（theme_stack 优先级 / 默认 / 向后兼容）
- `crates/core/src/engines/theme.rs`：4 个 ThemeEngine 单测 + theme_with_override 11 个纯函数单测
- `crates/core/src/engines/layout.rs`：4 个 LayoutEngine 单测（注册 / 当前 / 默认）

全部 `cargo test --features server -p rustineverything-core` 绿。

## 11. 后续阶段

- **Phase 3.4**：模块开关（[`MODULE_SPEC.md`](MODULE_SPEC.md)）已落地，与主题正交。
- **Phase 5.2**：示例 `examples/plugin-theme-purple` 演示从零开发主题。
- **Phase 5.1**：主题 Hot Reload（admin 上传 wasm，PluginEngine 失效缓存）。
