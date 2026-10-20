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

  /// Tailwind v4 的 zinc 色阶。站点把手写的 `slate-*` 类映射到 zinc，
  /// 默认主题的中性色 token 也取 zinc，组件和手写区域才不会冷暖不一。
  const ZINC_50: &str = "oklch(98.5% 0 0)";
  const ZINC_200: &str = "oklch(92% 0.004 286.32)";
  const ZINC_400: &str = "oklch(70.5% 0.015 286.067)";
  const ZINC_600: &str = "oklch(44.2% 0.017 285.786)";
  const ZINC_800: &str = "oklch(27.4% 0.006 286.033)";
  const ZINC_900: &str = "oklch(21% 0.006 285.885)";
  const ZINC_950: &str = "oklch(14.1% 0.005 285.823)";
  const CYAN_400: &str = "oklch(78.9% 0.154 211.53)";
  const CYAN_700: &str = "oklch(52% 0.105 223.128)";

  fn value_of(block: &str, token: &str) -> String {
    let decl = format!("{token}:");
    block
      .lines()
      .find_map(|line| line.trim().strip_prefix(decl.as_str()))
      .map(|v| v.trim().trim_end_matches(';').to_string())
      .unwrap_or_else(|| panic!("missing {token}"))
  }

  #[test]
  fn ocean_neutral_tokens_use_the_zinc_scale() {
    let css = find_theme("ocean").map(|t| t.css).unwrap_or_default();
    let (light, dark) = css.split_once(".dark {").expect(".dark block");
    let light_expected = [
      ("--foreground", ZINC_900),
      ("--card-foreground", ZINC_900),
      ("--popover-foreground", ZINC_900),
      ("--secondary", ZINC_50),
      ("--secondary-foreground", ZINC_900),
      ("--muted", ZINC_50),
      ("--muted-foreground", ZINC_600),
      ("--accent", ZINC_50),
      ("--accent-foreground", ZINC_900),
      ("--border", ZINC_200),
      ("--input", ZINC_200),
    ];
    let dark_expected = [
      ("--background", ZINC_950),
      ("--foreground", ZINC_50),
      ("--card", ZINC_900),
      ("--card-foreground", ZINC_50),
      ("--popover", ZINC_900),
      ("--popover-foreground", ZINC_50),
      ("--secondary", ZINC_800),
      ("--secondary-foreground", ZINC_50),
      ("--muted", ZINC_800),
      ("--muted-foreground", ZINC_400),
      ("--accent", ZINC_800),
      ("--accent-foreground", ZINC_50),
      ("--border", ZINC_800),
      ("--input", ZINC_800),
    ];
    for (token, want) in light_expected {
      assert_eq!(value_of(light, token), want, "light {token}");
    }
    for (token, want) in dark_expected {
      assert_eq!(value_of(dark, token), want, "dark {token}");
    }
  }

  #[test]
  fn ocean_primary_is_cyan() {
    let css = find_theme("ocean").map(|t| t.css).unwrap_or_default();
    let (light, dark) = css.split_once(".dark {").expect(".dark block");
    assert_eq!(value_of(light, "--primary"), CYAN_700);
    assert_eq!(value_of(light, "--primary-foreground"), "#ffffff");
    assert_eq!(value_of(dark, "--primary"), CYAN_400);
    assert_eq!(value_of(dark, "--primary-foreground"), ZINC_950);
  }
}
