#![allow(clippy::missing_safety_doc)] // WASM ABI exports: 安全契约见 docs/PLUGIN_ABI.md
//! Phase 5.2.1：示例主题插件 — 紫罗兰（Violet / Purple）。
//!
//! 本插件是 [PLUGIN_DEV.md](../../docs/PLUGIN_DEV.md) 中演示的「30 分钟做
//! 一个主题」实现。**不会**被 `assets/site.json` 默认引用，需要手工构建
//! 并复制 wasm 才能启用。
//!
//! 构建：
//! ```sh
//! CARGO_TARGET_DIR=/Users/hal/.target cargo build \
//!   -p plugin-theme-purple --target wasm32-unknown-unknown --release
//! cp /Users/hal/.target/wasm32-unknown-unknown/release/plugin_theme_purple.wasm \
//!    assets/plugins/
//! ```
//!
//! 在 `assets/site.json::themes` 中加入 `"plugin_theme_purple.wasm"` 即生效。
//!
//! 与正式插件（`crates/plugins/theme-*`）相比，这里：
//! - 体积/可读性优先，CSS 直接内联在 const 中
//! - 只导出最少 3 个函数：`get_manifest` / `get_theme_css` / `alloc / dealloc`（后者由 SDK 提供）
//! - 含纯 Rust 单测（[`palette_has_violet_primary`]），与 wasm runtime 解耦

use sdk::{alloc, capabilities, pack_json, PluginManifest};
use std::slice;

/// Manifest：声明 capability=theme，复用当前 SDK ABI 版本。
#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let m = PluginManifest::new("theme-purple", "Theme Purple (Example)", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::THEME)
    .with_description("紫罗兰示例主题，演示 Phase 5 插件开发流程")
    .with_author("yuxuetr");
  pack_json(&m)
}

/// 主题 CSS。语义：把核心变量映射到 Tailwind `violet-*` 调色板。
///
/// 6 个变量是 site 主题协定的最小集（与 theme-ocean / theme-sunset 对齐）：
/// `--color-primary` / `--color-bg` / `--color-surface` /
/// `--color-text` / `--color-text-muted` / `--color-border`
const THEME_CSS: &str = r#"
:root {
  --color-primary: #7c3aed;      /* violet-600 */
  --color-bg: #faf5ff;            /* violet-50 */
  --color-surface: #f3e8ff;       /* violet-100 */
  --color-text: #1e1b4b;          /* indigo-950 */
  --color-text-muted: #4c1d95;    /* violet-900 */
  --color-border: #ddd6fe;        /* violet-200 */
}

.dark {
  --color-primary: #a78bfa;       /* violet-400 */
  --color-bg: #1e1b4b;            /* indigo-950 */
  --color-surface: #312e81;       /* indigo-900 */
  --color-text: #ede9fe;          /* violet-100 */
  --color-text-muted: #c4b5fd;    /* violet-300 */
  --color-border: #4338ca;        /* indigo-700 */
}

/* 强制 body 跟随变量；与既有内置主题保持一致行为。 */
body {
  background-color: var(--color-bg) !important;
  color: var(--color-text) !important;
}
"#;

/// 导出主题 CSS。返回值高 32 位 = ptr，低 32 位 = len。
#[no_mangle]
pub unsafe extern "C" fn get_theme_css(_ptr: *mut u8, _len: usize) -> u64 {
  let bytes = THEME_CSS.as_bytes();
  let ptr = alloc(bytes.len());
  let dst = slice::from_raw_parts_mut(ptr, bytes.len());
  dst.copy_from_slice(bytes);
  ((ptr as u64) << 32) | (bytes.len() as u64)
}

// ────────────────────────────────────────────────────────────
// 单测：在 host 环境跑，验证 manifest / CSS 内容，不依赖 wasm runtime
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  /// manifest_core 提取出可在 host 环境构造的 PluginManifest，与
  /// `get_manifest` 的逻辑一致；让单测覆盖最小可证文本。
  fn manifest_core() -> PluginManifest {
    PluginManifest::new("theme-purple", "Theme Purple (Example)", env!("CARGO_PKG_VERSION"))
      .with_capability(capabilities::THEME)
      .with_description("紫罗兰示例主题，演示 Phase 5 插件开发流程")
      .with_author("yuxuetr")
  }

  #[test]
  fn manifest_declares_theme_capability() {
    let m = manifest_core();
    assert_eq!(m.id, "theme-purple");
    assert!(m.has_capability(capabilities::THEME));
    assert!(m.is_compatible(), "应与当前 SDK ABI 兼容");
  }

  #[test]
  fn palette_has_violet_primary() {
    // 防止颜色被误改回非紫色（回归保护）。
    assert!(THEME_CSS.contains("#7c3aed"), "light primary should be violet-600");
    assert!(THEME_CSS.contains("#a78bfa"), "dark primary should be violet-400");
  }

  #[test]
  fn declares_all_six_required_vars() {
    for v in &[
      "--color-primary",
      "--color-bg",
      "--color-surface",
      "--color-text",
      "--color-text-muted",
      "--color-border",
    ] {
      assert!(THEME_CSS.contains(v), "missing required CSS var: {}", v);
    }
  }

  #[test]
  fn body_rule_overrides_default_bg() {
    assert!(THEME_CSS.contains("body {"));
    assert!(THEME_CSS.contains("var(--color-bg) !important"));
  }
}
