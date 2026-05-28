use rustineverything_sdk::{alloc, capabilities, dealloc, pack_json, PluginManifest};
use std::slice;

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let manifest = PluginManifest::new("theme-ocean", "Theme Ocean", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::THEME)
    .with_description("海色主题插件 (不同于 ocean breeze)")
    .with_author("yuxuetr");
  pack_json(&manifest)
}

const THEME_CSS: &str = "
:root {
  --color-primary: oklch(65% 0.15 250);
  --color-bg: #ffffff;
  --color-surface: #f8fafc;
  --color-text: #0f172a;
  --color-text-muted: #475569;
  --color-border: #e2e8f0;
}

.dark {
  --color-primary: oklch(75% 0.15 250);
  --color-bg: #020617;
  --color-surface: #0f172a;
  --color-text: #f8fafc;
  --color-text-muted: #94a3b8;
  --color-border: #1e293b;
}

/* 强制 Body 背景跟随变量 */
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
