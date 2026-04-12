use std::slice;
use rustineverything_sdk::{alloc, dealloc};

/// Tailwind V4 主题配色变量
const THEME_CSS: &str = "
:root {
  --color-primary: oklch(65% 0.15 250); /* 海洋蓝 */
  --color-secondary: oklch(80% 0.1 200); /* 浅绿 */
  --color-accent: oklch(75% 0.2 180); /* 碧绿 */
  --font-sans: 'Inter', system-ui, sans-serif;
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
