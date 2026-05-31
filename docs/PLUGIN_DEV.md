# Plugin Development Guide

> 适用阶段：Phase 5（v2.1 Todos.md）。
> 30 分钟入门：从零写一个 WASM 插件、构建、部署到站点。
> ABI 规范见 [PLUGIN_ABI.md](PLUGIN_ABI.md)。

## 1. 准备

```bash
# Rust 工具链 + wasm32 target
rustup target add wasm32-unknown-unknown
```

如果你已经构建过本仓库的内置插件（`./scripts/build_themes.sh`），
target 已经就绪。

## 2. 项目骨架

新建 `crates/plugins/<your-name>/`：

```text
crates/plugins/theme-purple/
├── Cargo.toml
└── src/
    └── lib.rs
```

### 2.1 `Cargo.toml`

```toml
[package]
name = "theme-purple-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
sdk = { path = "../../sdk" }
```

关键点：
- `name` 以 `-plugin` 结尾（与 build script 约定一致）。
- `crate-type = ["cdylib"]` 必填，否则不会输出 `.wasm` 产物。
- 依赖 `sdk` 拿到 `alloc/dealloc`、`pack_output`、`PluginManifest` 等辅助。

### 2.2 把 crate 加入 workspace

编辑根 `Cargo.toml`：

```toml
[workspace]
members = [
    # ... 其他 crates
    "crates/plugins/theme-purple",
]
```

## 3. 最小可行实现（主题插件）

`crates/plugins/theme-purple/src/lib.rs`：

```rust
use sdk::{alloc, capabilities, pack_json, PluginManifest};
use std::slice;

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
    let m = PluginManifest::new(
        "theme-purple",
        "Theme Purple",
        env!("CARGO_PKG_VERSION"),
    )
    .with_capability(capabilities::THEME)
    .with_description("紫罗兰主题（示例）")
    .with_author("yuxuetr");
    pack_json(&m)
}

const THEME_CSS: &str = r#"
:root {
  --color-primary: #7c3aed;        /* violet-600 */
  --color-bg: #faf5ff;             /* violet-50 */
  --color-surface: #f3e8ff;        /* violet-100 */
  --color-text: #1e1b4b;
  --color-text-muted: #4c1d95;
  --color-border: #ddd6fe;         /* violet-200 */
}
.dark {
  --color-primary: #a78bfa;
  --color-bg: #1e1b4b;
  --color-surface: #312e81;
  --color-text: #ede9fe;
  --color-text-muted: #c4b5fd;
  --color-border: #4338ca;
}
body { background-color: var(--color-bg) !important; color: var(--color-text) !important; }
"#;

#[no_mangle]
pub unsafe extern "C" fn get_theme_css(_ptr: *mut u8, _len: usize) -> u64 {
    let bytes = THEME_CSS.as_bytes();
    let ptr = alloc(bytes.len());
    let dst = slice::from_raw_parts_mut(ptr, bytes.len());
    dst.copy_from_slice(bytes);
    ((ptr as u64) << 32) | (bytes.len() as u64)
}
```

就这样，~25 行实现一个完整主题。

## 4. 构建

```bash
CARGO_TARGET_DIR=/Users/hal/.target cargo build \
    -p theme-purple-plugin \
    --target wasm32-unknown-unknown \
    --release
```

产物在：
```
/Users/hal/.target/wasm32-unknown-unknown/release/theme_purple_plugin.wasm
```

注意 crate 名 `theme-purple-plugin` 在产物中变为 `theme_purple_plugin.wasm`
（下划线）— wasm 工具链统一约定。

## 5. 部署到站点

```bash
cp /Users/hal/.target/wasm32-unknown-unknown/release/theme_purple_plugin.wasm \
   assets/plugins/
```

然后编辑 `assets/site.json` 启用：

```jsonc
{
  "themes": ["theme_purple_plugin.wasm"],
  "_themes_available": [
    "theme_ocean_plugin.wasm",
    "theme_sunset_plugin.wasm",
    "theme_catppuccin_plugin.wasm",
    "theme_purple_plugin.wasm"     // ← 新增
  ]
}
```

重启 dev server 或刷新页面 — 主题立即生效。
ThemePicker 会自动通过 `list_available_themes` 扫到新插件
（manifest capability=`theme` 匹配）。

## 6. 其他 Capability 模板

### 6.1 i18n 插件

`get_manifest` 返回 capability=`i18n`，并实现：

```rust
#[no_mangle]
pub unsafe extern "C" fn translate(ptr: *mut u8, len: usize) -> u64 {
    use sdk::{pack_output, read_input};

    let input = read_input(ptr, len);
    let req: serde_json::Value = serde_json::from_slice(input).unwrap_or_default();
    let key = req.get("key").and_then(|v| v.as_str()).unwrap_or("");
    let lang = req.get("lang").and_then(|v| v.as_str()).unwrap_or("zh");

    let translation = match (key, lang) {
        ("nav-blog", "en") => "Blog",
        ("nav-blog", _)    => "博客",
        _ => key,
    };
    pack_output(translation.as_bytes().to_vec())
}
```

### 6.2 Auth 插件

完整模板看 `crates/plugins/github-auth/src/lib.rs`。核心导出：

- `get_config` → `AuthProviderConfig` JSON（OAuth endpoints + scopes + PKCE flag）
- `get_display_info` → `AuthProviderDisplay` JSON（名字 + 图标 + 品牌色）
- `exchange_code` → 把 `code` 换 access_token（插件内 raw HTTP 形态序列化为 JSON 让宿主代发请求）
- `fetch_profile` → 解析 profile 返回 `StandardUser` JSON

宿主负责真实 HTTP 调用、cookie、JWT 等；插件只做数据 mapping。

### 6.3 Moderation 插件（Phase 4.3 计划）

ABI 待定。预期导出：

- `get_endpoint` → API URL + headers
- `map_request(content)` → API request body JSON
- `map_verdict(api_response)` → `Verdict` JSON

详见 [MODERATION_SPEC.md](MODERATION_SPEC.md)。

## 7. 调试技巧

### 7.1 本地测试导出函数

WASM 是 sandbox 的，传统调试器不可用。常见做法：

1. **把核心逻辑提取为普通 fn**（不带 `#[no_mangle]`），用 `cargo test` 跑单测。
2. WASM 入口（`extern "C"`）只做「解析输入 → 调核心 fn → 打包输出」三件事。

示例：

```rust
fn translate_core(key: &str, lang: &str) -> &'static str { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translate_nav_blog_en() {
        assert_eq!(translate_core("nav-blog", "en"), "Blog");
    }
}

#[no_mangle]
pub unsafe extern "C" fn translate(ptr: *mut u8, len: usize) -> u64 {
    // ... 解析 input ...
    let out = translate_core(key, lang);
    pack_output(out.as_bytes().to_vec())
}
```

### 7.2 查看 wasm 体积

```bash
ls -lh assets/plugins/your_plugin.wasm
```

参考：内置主题 ~26 KB；i18n ~80 KB；auth ~50 KB。
超过 200 KB 通常意味着不必要的依赖被打包，可用 `cargo bloat --target wasm32-unknown-unknown -p your-plugin` 排查。

### 7.3 错误：宿主拒绝加载

| 错误信息 | 原因 | 修复 |
| --- | --- | --- |
| `ABI not compatible` | `abi_version != 1` | 用 `PluginManifest::new(...)` 而非手填 |
| `plugin output exceeds limit` | 返回 > 8 MiB | 削减输出 / 调 `with_output_limit` |
| `module has no memory export` | 缺 `crate-type = ["cdylib"]` | 检查 Cargo.toml |
| `function "X" not found` | 函数名拼错 / 缺 `#[no_mangle]` | 加 `#[no_mangle] pub extern "C"` |

## 8. CI / 一键脚本

如果是主题插件，可以加进 `scripts/build_themes.sh`：

```bash
ALL_THEMES=(
    "theme-ocean-plugin:theme_ocean_plugin.wasm"
    "theme-sunset-plugin:theme_sunset_plugin.wasm"
    "theme-catppuccin-plugin:theme_catppuccin_plugin.wasm"
    "theme-purple-plugin:theme_purple_plugin.wasm"   # ← 新增
)
```

非主题插件按同样模式写自己的 build 脚本。

## 9. 发布清单

新插件 PR 前自检：

- [ ] `cargo build --target wasm32-unknown-unknown --release -p <plugin>` 通过
- [ ] `cargo test -p <plugin>` 通过（如有单测）
- [ ] manifest 中 capability 正确且 `abi_version == 1`
- [ ] 产物体积合理（主题 < 50 KB / Auth < 100 KB / 其他参考既有插件）
- [ ] 站点 `site.json` 已加入相应字段
- [ ] 在 `docs/` 中补一段 README / 截图（可选）

## 9.1 沙箱约束（Phase 8.1）

宿主对每次 wasm 调用施加四道防线，插件作者编码时需要心里有数：

| 维度 | 默认 | 覆盖 env | 说明 |
| --- | --- | --- | --- |
| Fuel（指令额度） | `100_000_000` ≈ 1s 内核活动 | `WASM_FUEL_LIMIT` | wasmi `Config::consume_fuel(true)` + `Store::set_fuel`，死循环会 trap |
| 线性内存 | 128 页 = 8 MiB | `WASM_MEMORY_PAGES` | `StoreLimitsBuilder::memory_size`；插件 `memory.grow` 超过即 -1 |
| 输出长度上限 | 8 MiB | `WASM_OUTPUT_LIMIT` | host 在 `vec![0u8; len]` 前 clamp；超额直接报错而非截断 |
| 单次调用 timeout | 5s | `WASM_INVOKE_TIMEOUT_SECS` | tokio 兜底；任何 wasmi 调用都跑在 `spawn_blocking` 工作池，timeout 优先释放调度器 |

实践指导：

- 不要在 `start`（构造函数）里跑长循环 —— 加载时就走 fuel
- 输出大概率在 KB 级；若上得了 MB 量级，请评估 `OUTPUT_LIMIT` 是否需要在部署侧上调
- 任何外部 import（host functions）目前**未提供** —— 插件是纯计算 + 字符串 I/O，遇到需要 IO 的场景请回到 host 端实现
- 校验阶段（`admin_upload_plugin`）也同样跑在沙箱内，恶意 `start` 不会卡住上传流程

## 10. 后续阶段（Phase 5+）

- **Hot Reload**：admin 上传 wasm → PluginEngine `invalidate` → 替换。完成后无需重启 server。
- **`/plugins` 浏览页**：从 `assets/plugins/registry.json` 拉清单，提供作者元信息 + 一键启用按钮（Phase 5.5）。
- **签名 / 校验**：上传 wasm 时校验 SHA256 + 可选 Ed25519 签名（Phase 7）。

## 11. 参考

- [PLUGIN_ABI.md](PLUGIN_ABI.md)：完整 ABI 规范
- [crates/sdk/src/lib.rs](../crates/sdk/src/lib.rs)：SDK 源码（~200 行，可一口气读完）
- [crates/plugins/theme-ocean/src/lib.rs](../crates/plugins/theme-ocean/src/lib.rs)：最简单的主题插件实现
- [crates/plugins/github-auth/src/lib.rs](../crates/plugins/github-auth/src/lib.rs)：完整 Auth 插件实现
- [THEME_SPEC.md](THEME_SPEC.md) / [MODERATION_SPEC.md](MODERATION_SPEC.md)：各能力的详细规范
