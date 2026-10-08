#![allow(clippy::missing_safety_doc)] // WASM ABI exports: 安全契约见 docs/PLUGIN_ABI.md
//! Phase 3.2 主题插件：Catppuccin。
//!
//! Light: Catppuccin Latte。Dark: Catppuccin Macchiato（Todos.md 既定）。
//! 仅提取所需的 6 个 CSS 变量，避免引入完整调色板字典。
//!
//! 调色板原始值参考 https://catppuccin.com/palette。

use std::slice;

use sdk::{alloc, capabilities, dealloc, pack_json, PluginManifest};

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let manifest =
    PluginManifest::new("theme-catppuccin", "Theme Catppuccin", env!("CARGO_PKG_VERSION"))
      .with_capability(capabilities::THEME)
      .with_description("Catppuccin Latte (light) + Macchiato (dark) 调色板")
      .with_author("yuxuetr");
  pack_json(&manifest)
}

// Latte: base #eff1f5, text #4c4f69, mantle #e6e9ef, surface0 #ccd0da, blue #1e66f5, overlay0 #9ca0b0
// Macchiato: base #24273a, text #cad3f5, mantle #1e2030, surface0 #363a4f, blue #8aadf4, overlay0 #6e738d
const THEME_CSS: &str = "
/* shadcn 语义 token（docs/THEME_SPEC.md §12「Token 契约」）：组件与旧 --color-* 别名都读这些变量。 */
:root {
  --background: #eff1f5;
  --foreground: #4c4f69;
  --card: #eff1f5;
  --card-foreground: #4c4f69;
  --popover: #eff1f5;
  --popover-foreground: #4c4f69;
  --primary: #1e66f5;
  --primary-foreground: #eff1f5;
  --secondary: #e6e9ef;
  --secondary-foreground: #4c4f69;
  --muted: #e6e9ef;
  --muted-foreground: #6c6f85;
  --accent: #e6e9ef;
  --accent-foreground: #4c4f69;
  --border: #ccd0da;
  --input: #ccd0da;
  --ring: #1e66f5;
}

.dark {
  --background: #24273a;
  --foreground: #cad3f5;
  --card: #1e2030;
  --card-foreground: #cad3f5;
  --popover: #1e2030;
  --popover-foreground: #cad3f5;
  --primary: #8aadf4;
  --primary-foreground: #24273a;
  --secondary: #363a4f;
  --secondary-foreground: #cad3f5;
  --muted: #363a4f;
  --muted-foreground: #a5adcb;
  --accent: #363a4f;
  --accent-foreground: #cad3f5;
  --border: #363a4f;
  --input: #363a4f;
  --ring: #8aadf4;
}

/* 强制 body 背景跟随变量 */
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
