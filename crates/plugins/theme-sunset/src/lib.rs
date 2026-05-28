//! Phase 3.2 主题插件：Sunset。
//!
//! 暖色调（橙 / 桃 / 琥珀）；同时提供 light 与 `.dark` 两套 CSS 变量。
//! 与 `theme-ocean` 接口完全对齐：导出 `alloc` / `dealloc` /
//! `get_manifest` / `get_theme_css`。

use std::slice;

use rustineverything_sdk::{alloc, capabilities, dealloc, pack_json, PluginManifest};

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let manifest = PluginManifest::new("theme-sunset", "Theme Sunset", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::THEME)
    .with_description("Sunset 暖色调主题（橙 / 琥珀），含 light + dark 双模")
    .with_author("yuxuetr");
  pack_json(&manifest)
}

const THEME_CSS: &str = "
:root {
  --color-primary: oklch(70% 0.16 45);
  --color-bg: #fffaf3;
  --color-surface: #fff1e0;
  --color-text: #3b1f10;
  --color-text-muted: #8a5a3a;
  --color-border: #f5d6b3;
}

.dark {
  --color-primary: oklch(78% 0.18 50);
  --color-bg: #1c0d05;
  --color-surface: #2a160a;
  --color-text: #fff1e0;
  --color-text-muted: #d8a780;
  --color-border: #4a2a18;
}

/* 强制 Body 背景跟随变量（与 ocean 主题保持一致的硬约束） */
body {
  background-color: var(--color-bg) !important;
  color: var(--color-text) !important;
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
  let _ = dealloc(std::ptr::null_mut(), 0);
}
