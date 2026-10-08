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
/* shadcn 语义 token（docs/THEME_SPEC.md §12「Token 契约」）：组件与旧 --color-* 别名都读这些变量。
   中性色取 Tailwind stone 色阶，与站点 slate→stone 映射后的手写类名一致。 */
:root {
  --background: #ffffff;
  --foreground: oklch(21.6% 0.006 56.043);
  --card: #ffffff;
  --card-foreground: oklch(21.6% 0.006 56.043);
  --popover: #ffffff;
  --popover-foreground: oklch(21.6% 0.006 56.043);
  --primary: oklch(64.6% 0.222 41.116);
  --primary-foreground: #ffffff;
  --secondary: oklch(98.5% 0.001 106.423);
  --secondary-foreground: oklch(21.6% 0.006 56.043);
  --muted: oklch(98.5% 0.001 106.423);
  --muted-foreground: oklch(44.4% 0.011 73.639);
  --accent: oklch(98.5% 0.001 106.423);
  --accent-foreground: oklch(21.6% 0.006 56.043);
  --border: oklch(92.3% 0.003 48.717);
  --input: oklch(92.3% 0.003 48.717);
  --ring: oklch(70.5% 0.213 47.604);
}

.dark {
  --background: oklch(14.7% 0.004 49.25);
  --foreground: oklch(98.5% 0.001 106.423);
  --card: oklch(21.6% 0.006 56.043);
  --card-foreground: oklch(98.5% 0.001 106.423);
  --popover: oklch(21.6% 0.006 56.043);
  --popover-foreground: oklch(98.5% 0.001 106.423);
  --primary: oklch(64.6% 0.222 41.116);
  --primary-foreground: #ffffff;
  --secondary: oklch(26.8% 0.007 34.298);
  --secondary-foreground: oklch(98.5% 0.001 106.423);
  --muted: oklch(26.8% 0.007 34.298);
  --muted-foreground: oklch(70.9% 0.01 56.259);
  --accent: oklch(26.8% 0.007 34.298);
  --accent-foreground: oklch(98.5% 0.001 106.423);
  --border: oklch(26.8% 0.007 34.298);
  --input: oklch(26.8% 0.007 34.298);
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

#[cfg(test)]
mod tests {
  use super::THEME_CSS;

  /// Tailwind v4 的 stone 色阶。站点把手写的 `slate-*` 类映射到 stone，
  /// 主题的中性色 token 也取 stone，组件和手写区域才不会冷暖不一。
  const STONE_50: &str = "oklch(98.5% 0.001 106.423)";
  const STONE_200: &str = "oklch(92.3% 0.003 48.717)";
  const STONE_400: &str = "oklch(70.9% 0.01 56.259)";
  const STONE_600: &str = "oklch(44.4% 0.011 73.639)";
  const STONE_800: &str = "oklch(26.8% 0.007 34.298)";
  const STONE_900: &str = "oklch(21.6% 0.006 56.043)";
  const STONE_950: &str = "oklch(14.7% 0.004 49.25)";

  fn value_of(block: &str, token: &str) -> String {
    let decl = format!("{token}:");
    block
      .lines()
      .find_map(|line| line.trim().strip_prefix(decl.as_str()))
      .map(|v| v.trim().trim_end_matches(';').to_string())
      .unwrap_or_else(|| panic!("missing {token}"))
  }

  #[test]
  fn neutral_tokens_use_the_stone_scale() {
    let (light, dark) = THEME_CSS.split_once(".dark {").expect(".dark block");
    let light_expected = [
      ("--foreground", STONE_900),
      ("--card-foreground", STONE_900),
      ("--popover-foreground", STONE_900),
      ("--secondary", STONE_50),
      ("--secondary-foreground", STONE_900),
      ("--muted", STONE_50),
      ("--muted-foreground", STONE_600),
      ("--accent", STONE_50),
      ("--accent-foreground", STONE_900),
      ("--border", STONE_200),
      ("--input", STONE_200),
    ];
    let dark_expected = [
      ("--background", STONE_950),
      ("--foreground", STONE_50),
      ("--card", STONE_900),
      ("--card-foreground", STONE_50),
      ("--popover", STONE_900),
      ("--popover-foreground", STONE_50),
      ("--secondary", STONE_800),
      ("--secondary-foreground", STONE_50),
      ("--muted", STONE_800),
      ("--muted-foreground", STONE_400),
      ("--accent", STONE_800),
      ("--accent-foreground", STONE_50),
      ("--border", STONE_800),
      ("--input", STONE_800),
    ];
    for (token, want) in light_expected {
      assert_eq!(value_of(light, token), want, "light {token}");
    }
    for (token, want) in dark_expected {
      assert_eq!(value_of(dark, token), want, "dark {token}");
    }
  }

  #[test]
  fn primary_stays_brand_orange() {
    assert_eq!(THEME_CSS.matches("--primary: oklch(64.6% 0.222 41.116);").count(), 2);
  }
}
