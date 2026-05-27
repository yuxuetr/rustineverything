# Plugin ABI Spec

> 适用阶段：Phase 1C.2 + Phase 5（v2.1 Todos.md）。
> 本文规范化宿主（`crates/core/src/engines/plugin.rs`）与 WASM 插件
> （`crates/plugins/*` / `crates/sdk`）之间的 **二进制接口**。

## 1. 当前版本

- **ABI 版本**：`SDK_ABI_VERSION = 1` (`crates/sdk/src/lib.rs:9`)
- **WASM 目标**：`wasm32-unknown-unknown`
- **crate-type**：`cdylib`
- **运行时**：[wasmi](https://github.com/wasmi-labs/wasmi) (`Engine::default()`)
- **内存上限**：`PluginEngine::DEFAULT_PLUGIN_OUTPUT_LIMIT = 8 MiB`

升级 ABI（破坏性变更）规则：
1. SDK 中将 `SDK_ABI_VERSION` 加 1
2. 所有插件 manifest 重建（`get_manifest` 重新 pack）
3. 旧 ABI 的插件被宿主以 `AppError::Plugin("ABI不兼容")` 拒绝

## 2. 必备导出函数

每个插件 cdylib 必须导出以下 3 个函数（命名固定，签名固定）：

### 2.1 `alloc` / `dealloc`（内存管理）

```rust
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8;

#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize);
```

由 [`rustineverything-sdk`](../crates/sdk/src/lib.rs) 提供默认实现，
通常无需重写。功能：在 wasm 线性内存中分配 / 释放字节段，让宿主能写入
输入参数 / 读出返回值。

### 2.2 `get_manifest`（自描述）

```rust
#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64;
```

- 输入：`(ptr, len)` 形参保留以匹配统一签名，**不被读取**。
- 输出：`u64`，高 32 位 = JSON 输出指针；低 32 位 = JSON 长度。
- 内容：`PluginManifest` JSON 序列化。

示例：

```rust
use rustineverything_sdk::{capabilities, pack_json, PluginManifest};

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
    let m = PluginManifest::new("my-theme", "My Theme", env!("CARGO_PKG_VERSION"))
        .with_capability(capabilities::THEME)
        .with_description("演示主题")
        .with_author("yuxuetr");
    pack_json(&m)
}
```

### 2.3 能力相关函数（按 capability 而异）

| Capability | 必备函数 | 输入 | 输出 |
| --- | --- | --- | --- |
| `theme` | `get_theme_css` | 忽略 | UTF-8 CSS 文本 |
| `i18n` | `translate` | `{"key": "...", "lang": "..."}` JSON | 翻译文本 |
| `auth-provider` | `get_config` / `exchange_code` / `fetch_profile` / `get_display_info` | OAuth 流程相关 JSON | OAuth 流程相关 JSON |
| `moderation-provider` | `get_endpoint` / `map_request` / `map_verdict` (Phase 4.3) | 见 [MODERATION_SPEC](MODERATION_SPEC.md) | 同上 |
| `mdx-component` | （宿主侧 ComponentRegistry 注册，不通过 wasm） | — | — |
| `layout` | （预留，Phase 5+） | — | — |
| `notification` | （预留） | — | — |

## 3. 数据打包约定

所有 wasm 函数的 **返回类型** 统一为 `u64`，高 32 位为指针，低 32 位为长度。

```text
                ┌────────────────┐
                │   u64 返回值    │
                ├────────────────┤
        高 32 位│  ptr (i32 cast)│  ← 在 wasm 线性内存中指向 UTF-8 字节
        低 32 位│  len (i32 cast)│  ← 字节数
                └────────────────┘
```

宿主调用流程（`PluginManager::invoke_module`）：

```text
1. 输入 -> alloc(len) -> input_ptr
2. memory.write(input_ptr, input_bytes)
3. target_fn.call(input_ptr, input_len) -> packed_u64
4. result_ptr = packed_u64 >> 32; result_len = packed_u64 & 0xFFFF_FFFF
5. memory.read(result_ptr, &mut result_buf)
6. dealloc(input_ptr, input_len); dealloc(result_ptr, result_len)
```

输出大小限制：超过 `DEFAULT_PLUGIN_OUTPUT_LIMIT`（默认 8 MiB）的输出
会被宿主拒绝并返回 `AppError::Plugin`，避免恶意 / bug 引起的内存爆炸。
调用方可通过 `PluginEngine::with_output_limit(n)` 调整。

## 4. SDK 辅助 API

`crates/sdk/src/lib.rs`：

| API | 用途 |
| --- | --- |
| `pack_output(Vec<u8>) -> u64` | 把字节串写入 wasm 内存 + 返回打包值 |
| `pack_json<T: Serialize>(&T) -> u64` | 等价于 `pack_output(serde_json::to_vec(v))` |
| `read_input(ptr, len) -> &[u8]` | 安全包装：读取宿主传入的字节串 |
| `PluginManifest::new(id, name, version)` | 构造 manifest，使用当前 ABI 版本 |
| `PluginManifest::with_capability(cap)` | 声明能力 |
| `PluginManifest::is_compatible()` | 宿主：校验 ABI 版本 |
| `capabilities::*` | 能力字符串常量 |

## 5. 能力（Capability）路由

宿主加载插件时：

1. 调 `get_manifest` 读出 `PluginManifest`
2. 校验 `m.is_compatible()`，不兼容 → 拒绝
3. 遍历 `m.capabilities`，把插件挂到对应引擎（ThemeEngine / AuthEngine / ...）

每个引擎只调用属于自己 capability 的函数。例如 ThemeEngine 永远不会调
`exchange_code`；AuthEngine 永远不会调 `get_theme_css`。

`PluginEngine::filter_by_capability(paths, cap)` / `capabilities_of(path)`
给上层统一调度。

## 6. 错误处理

| 错误来源 | 表现 | 后果 |
| --- | --- | --- |
| `alloc` / `dealloc` 缺失 | 实例化失败 | `AppError::Plugin` |
| `get_manifest` 不存在或解析失败 | manifest 拿不到 | `PluginEngine::call` 仍可调用；`strict_call` 拒绝 |
| `m.abi_version != SDK_ABI_VERSION` | 不兼容 | `AppError::Plugin("ABI不兼容")` |
| 函数 panic | wasmi trap | `AppError::Plugin` |
| 返回大小 > 限制 | 输出超限 | `AppError::Plugin("plugin output exceeds limit")` |
| `Memory::read` 越界 | wasmi runtime error | `AppError::Plugin` |

## 7. 安全模型

- WASM 沙箱：插件无法访问宿主文件系统、网络、环境变量（除非宿主显式
  通过 `Linker::define` 暴露 host fn — 当前未暴露任何 host fn，是
  **完全离线** 的纯函数模型）。
- 输入/输出受限于 wasm 线性内存，超限即拒绝。
- 跨调用持久状态：**无**（每次调用创建新 `Store`，旧状态丢弃）。
  Phase 5 Hot Reload 也是基于这一性质 — 替换 `Module` 即可，不用清理状态。

## 8. 版本兼容矩阵

| Plugin ABI | 宿主 ABI | 行为 |
| --- | --- | --- |
| `1` (当前) | `1` | ✅ 兼容 |
| `0` (老插件，未声明) | `1` | ❌ `is_compatible() = false` → 拒绝 |
| `2` (未来) | `1` | ❌ 旧宿主拒绝新插件 |
| `1` | `2` | ❌ 新宿主拒绝旧插件 |

关键约束：ABI 升级 **必须** 重建全部已部署插件并部署。

## 9. 当前内置插件清单（v1）

| 插件 | Capability | 文件 |
| --- | --- | --- |
| `theme-ocean` | theme | `assets/plugins/theme_ocean_plugin.wasm` |
| `theme-sunset` | theme | `assets/plugins/theme_sunset_plugin.wasm` |
| `theme-catppuccin` | theme | `assets/plugins/theme_catppuccin_plugin.wasm` |
| `i18n-fluent` | i18n | `assets/plugins/i18n_fluent_plugin.wasm` |
| `github-auth` | auth-provider | `assets/plugins/github_auth_plugin.wasm` |
| `google-auth` | auth-provider | `assets/plugins/google_auth_plugin.wasm` |
| `discord-auth` | auth-provider | `assets/plugins/discord_auth_plugin.wasm` |
| `twitter-auth` | auth-provider | `assets/plugins/twitter_auth_plugin.wasm` |

## 10. 测试覆盖

- `crates/sdk/src/lib.rs::tests`：10 个单测（manifest 构造 / ABI / 能力 / 序列化 / pack_output / pack_json / read_input）
- `crates/core/src/engines/plugin.rs::tests`：12 个单测（名字 / 限制 / shutdown / init / manifest 检测 / filter_by_capability / 老插件 / 超限 / 3 集成）

## 11. 参考

- [PLUGIN_DEV.md](PLUGIN_DEV.md)：30 分钟开发指南（从零写一个新主题 / Auth 插件）
- [ENGINES_SPEC.md](ENGINES_SPEC.md)：8 大引擎抽象 + 生命周期
- [THEME_SPEC.md](THEME_SPEC.md)：主题栈 + cookie 覆盖
- [MODERATION_SPEC.md](MODERATION_SPEC.md)：审核插件（Phase 4.3+）
