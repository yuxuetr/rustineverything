//! 内置主题（R2）：每个主题是一份写死在代码里的 CSS（shadcn 语义 token，见
//! `docs/THEME_SPEC.md`）。
//!
//! 站点默认主题来自 site.json 的 `theme`；访客可用 `site_theme` cookie 覆盖。两者都
//! 只认 [`THEMES`] 里的 id——cookie 由浏览器提交、可被任意改写（SEC-11），表外的值一律
//! 忽略。主题 CSS 是作者写的受信内容，原样输出到页面 `<style>`。

/// 一个内置主题。
#[derive(Debug, PartialEq, Eq)]
pub struct Theme {
  /// site.json、cookie 与 `/api/theme/set` 用的 id。
  pub id: &'static str,
  /// 主题切换菜单里显示的名字。
  pub label: &'static str,
  pub css: &'static str,
}

/// 全部内置主题，顺序即切换菜单的顺序。第一个是 site.json 未配置 / 配错时的回退。
pub const THEMES: &[Theme] = &[
  Theme { id: "ocean", label: "Ocean", css: include_str!("themes/ocean.css") },
  Theme { id: "sunset", label: "Sunset", css: include_str!("themes/sunset.css") },
  Theme { id: "catppuccin", label: "Catppuccin", css: include_str!("themes/catppuccin.css") },
];

pub fn find_theme(id: &str) -> Option<&'static Theme> {
  THEMES.iter().find(|t| t.id == id)
}

/// 本次请求生效的主题：合法的 cookie 覆盖 > site.json 默认 > 第一个内置主题。
pub fn resolve_theme(site_default: &str, cookie: Option<&str>) -> &'static Theme {
  if let Some(raw) = cookie.map(str::trim).filter(|s| !s.is_empty()) {
    match find_theme(raw) {
      Some(theme) => return theme,
      None => tracing::warn!(cookie = %raw.escape_debug(), "theme: unknown cookie theme, ignoring"),
    }
  }
  find_theme(site_default).unwrap_or_else(|| {
    tracing::warn!(theme = %site_default, "theme: unknown site.json theme, using default");
    &THEMES[0]
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cookie_overrides_site_default_only_with_a_builtin_id() {
    assert_eq!(resolve_theme("ocean", None).id, "ocean");
    assert_eq!(resolve_theme("ocean", Some("")).id, "ocean");
    assert_eq!(resolve_theme("ocean", Some(" sunset ")).id, "sunset");
    // SEC-11：路径、旧的插件文件名、其他插件、大小写变体都不是主题。
    for bad in
      ["../plugins/x", "theme_sunset_plugin.wasm", "i18n_fluent_plugin.wasm", "Sunset", "sun set"]
    {
      assert_eq!(resolve_theme("ocean", Some(bad)).id, "ocean", "{bad:?}");
    }
  }

  #[test]
  fn unknown_site_default_falls_back_to_first_theme() {
    assert_eq!(resolve_theme("theme_ocean_plugin.wasm", None).id, THEMES[0].id);
    assert_eq!(resolve_theme("", Some("catppuccin")).id, "catppuccin");
  }

  #[test]
  fn theme_ids_are_unique_and_every_theme_sets_light_and_dark_tokens() {
    for (i, theme) in THEMES.iter().enumerate() {
      assert!(THEMES[..i].iter().all(|t| t.id != theme.id), "重复 id {}", theme.id);
      assert!(theme.css.contains(":root {"), "{} 缺少亮色 token", theme.id);
      assert!(theme.css.contains(".dark {"), "{} 缺少暗色 token", theme.id);
    }
  }

  /// Tailwind v4 的 stone 色阶。站点把手写的 `slate-*` 类映射到 stone，
  /// 默认主题的中性色 token 也取 stone，组件和手写区域才不会冷暖不一。
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
  fn ocean_neutral_tokens_use_the_stone_scale() {
    let css = find_theme("ocean").map(|t| t.css).unwrap_or_default();
    let (light, dark) = css.split_once(".dark {").expect(".dark block");
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
  fn ocean_primary_stays_brand_orange() {
    let css = find_theme("ocean").map(|t| t.css).unwrap_or_default();
    assert_eq!(css.matches("--primary: oklch(64.6% 0.222 41.116);").count(), 2);
  }
}
