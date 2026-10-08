#![allow(clippy::missing_safety_doc)] // WASM ABI exports: 安全契约见 docs/PLUGIN_ABI.md
use sdk::{alloc, capabilities, dealloc, pack_json, PluginManifest};
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
/* shadcn 语义 token（docs/THEME_SPEC.md §12「Token 契约」）：组件与旧 --color-* 别名都读这些变量。 */
:root {
  --background: #ffffff;
  --foreground: #0f172a;
  --card: #ffffff;
  --card-foreground: #0f172a;
  --popover: #ffffff;
  --popover-foreground: #0f172a;
  --primary: oklch(64.6% 0.222 41.116);
  --primary-foreground: #ffffff;
  --secondary: #f8fafc;
  --secondary-foreground: #0f172a;
  --muted: #f8fafc;
  --muted-foreground: #475569;
  --accent: #f8fafc;
  --accent-foreground: #0f172a;
  --border: #e2e8f0;
  --input: #e2e8f0;
  --ring: oklch(70.5% 0.213 47.604);
}

.dark {
  --background: #020617;
  --foreground: #f8fafc;
  --card: #0f172a;
  --card-foreground: #f8fafc;
  --popover: #0f172a;
  --popover-foreground: #f8fafc;
  --primary: oklch(64.6% 0.222 41.116);
  --primary-foreground: #ffffff;
  --secondary: #1e293b;
  --secondary-foreground: #f8fafc;
  --muted: #1e293b;
  --muted-foreground: #94a3b8;
  --accent: #1e293b;
  --accent-foreground: #f8fafc;
  --border: #1e293b;
  --input: #1e293b;
  --ring: oklch(70.5% 0.213 47.604);
}

/* 强制 Body 背景跟随变量 */
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
