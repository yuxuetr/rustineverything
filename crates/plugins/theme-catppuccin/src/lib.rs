//! Phase 3.2 主题插件：Catppuccin。
//!
//! Light: Catppuccin Latte。Dark: Catppuccin Macchiato（Todos.md 既定）。
//! 仅提取所需的 6 个 CSS 变量，避免引入完整调色板字典。
//!
//! 调色板原始值参考 https://catppuccin.com/palette。

use std::slice;

use rustineverything_sdk::{alloc, capabilities, dealloc, pack_json, PluginManifest};

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
:root {
  --color-primary: #1e66f5;            /* Latte blue */
  --color-bg: #eff1f5;                  /* Latte base */
  --color-surface: #e6e9ef;             /* Latte mantle */
  --color-text: #4c4f69;                /* Latte text */
  --color-text-muted: #6c6f85;          /* Latte subtext0 */
  --color-border: #ccd0da;              /* Latte surface0 */
}

.dark {
  --color-primary: #8aadf4;            /* Macchiato blue */
  --color-bg: #24273a;                  /* Macchiato base */
  --color-surface: #1e2030;             /* Macchiato mantle */
  --color-text: #cad3f5;                /* Macchiato text */
  --color-text-muted: #a5adcb;          /* Macchiato subtext0 */
  --color-border: #363a4f;              /* Macchiato surface0 */
}

/* 强制 body 背景跟随变量 */
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
