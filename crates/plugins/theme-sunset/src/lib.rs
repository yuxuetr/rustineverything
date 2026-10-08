#![allow(clippy::missing_safety_doc)] // WASM ABI exports: 安全契约见 docs/PLUGIN_ABI.md
//! Phase 3.2 主题插件：Sunset。
//!
//! 暖色调（橙 / 桃 / 琥珀）；同时提供 light 与 `.dark` 两套 CSS 变量。
//! 与 `theme-ocean` 接口完全对齐：导出 `alloc` / `dealloc` /
//! `get_manifest` / `get_theme_css`。

use std::slice;

use sdk::{alloc, capabilities, dealloc, pack_json, PluginManifest};

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let manifest = PluginManifest::new("theme-sunset", "Theme Sunset", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::THEME)
    .with_description("Sunset 暖色调主题（橙 / 琥珀），含 light + dark 双模")
    .with_author("yuxuetr");
  pack_json(&manifest)
}

const THEME_CSS: &str = "
/* shadcn 语义 token（docs/THEME_SPEC.md §12「Token 契约」）：组件与旧 --color-* 别名都读这些变量。 */
:root {
  --background: #fffaf3;
  --foreground: #3b1f10;
  --card: #fffaf3;
  --card-foreground: #3b1f10;
  --popover: #fffaf3;
  --popover-foreground: #3b1f10;
  --primary: oklch(70% 0.16 45);
  --primary-foreground: #3b1f10;
  --secondary: #fff1e0;
  --secondary-foreground: #3b1f10;
  --muted: #fff1e0;
  --muted-foreground: #8a5a3a;
  --accent: #fff1e0;
  --accent-foreground: #3b1f10;
  --border: #f5d6b3;
  --input: #f5d6b3;
  --ring: oklch(70% 0.16 45);
}

.dark {
  --background: #1c0d05;
  --foreground: #fff1e0;
  --card: #2a160a;
  --card-foreground: #fff1e0;
  --popover: #2a160a;
  --popover-foreground: #fff1e0;
  --primary: oklch(78% 0.18 50);
  --primary-foreground: #1c0d05;
  --secondary: #4a2a18;
  --secondary-foreground: #fff1e0;
  --muted: #4a2a18;
  --muted-foreground: #d8a780;
  --accent: #4a2a18;
  --accent-foreground: #fff1e0;
  --border: #4a2a18;
  --input: #4a2a18;
  --ring: oklch(78% 0.18 50);
}

/* 强制 Body 背景跟随变量（与 ocean 主题保持一致的硬约束） */
body {
  background-color: var(--background) !important;
  color: var(--foreground) !important;
}
";

#[no_mangle]
pub unsafe extern "C" fn get_theme_css(_ptr: *mut u8, _len: usize) -> u64 {
  let result_bytes = THEME_CSS.as_bytes();
  let res_len = result_bytes.len();
  let res_ptr = alloc(res_len);
  let res_slice = slice::from_raw_parts_mut(res_ptr, res_len);
  res_slice.copy_from_slice(result_bytes);
  ((res_ptr as u64) << 32) | (res_len as u64)
}

#[no_mangle]
pub unsafe extern "C" fn plugin_unused_fix() {
  dealloc(std::ptr::null_mut(), 0);
}
