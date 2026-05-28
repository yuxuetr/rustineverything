//! URL 域名黑名单 stage：从评论文本中扫出 URL，命中黑名单即 Block。
//!
//! 是流水线里的第一道便宜防线：
//! - **不调用 LLM**，不耗 token，µs 级返回
//! - **确定性 + 可解释**：admin 看 site.json 就知道为什么被拒
//! - 命中即 Block（score=1.0），早停后续 LLM stage
//!
//! 没有命中黑名单时返回 Allow，让 LLM stage 接管。
//!
//! ## 配置
//! ```jsonc
//! "moderation": {
//!   "enabled": true,
//!   "url_blocklist": ["scam.com", "*.phishing.example", "bit.ly"]
//! }
//! ```
//!
//! ## 匹配规则
//! - 精确：`"scam.com"` 只匹配 host = `scam.com`（含端口的剥端口后再匹配）
//! - 通配：`"*.phishing.example"` 匹配 `sub.phishing.example` 与
//!   `phishing.example` 本身
//! - 不区分大小写
//!
//! ## URL 提取
//! 手写扫描，不引 regex 依赖（避免编译时间膨胀）。识别 `http://` 与
//! `https://` 起首的 URL，从 scheme 之后扫到首个非 URL 字符为止
//! （空白 / 中文 / 不在 RFC 3986 reserved 集合的）。

use async_trait::async_trait;

use rustineverything_core::engines::moderation::Verdict;
use rustineverything_sdk::ModerationSubmission;

use crate::stage::AsyncModerationStage;

/// 黑名单 stage。默认配置（空模式列表）下永远 Allow。
pub struct UrlBlocklistStage {
  patterns: Vec<String>,
}

impl UrlBlocklistStage {
  /// 用一组域名模式构造。空 patterns 是合法的（永不命中）。
  pub fn new<I, S>(patterns: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    Self {
      patterns: patterns
        .into_iter()
        .map(|s| s.into().trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect(),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.patterns.is_empty()
  }

  pub fn patterns(&self) -> &[String] {
    &self.patterns
  }
}

#[async_trait]
impl AsyncModerationStage for UrlBlocklistStage {
  fn name(&self) -> &str {
    "url-blocklist"
  }

  async fn evaluate(&self, submission: &ModerationSubmission) -> Verdict {
    if self.patterns.is_empty() {
      return Verdict::allow();
    }
    for url in extract_urls(&submission.content) {
      let host = match host_of(url) {
        Some(h) => h,
        None => continue,
      };
      if let Some(pattern) = match_blocklist(host, &self.patterns) {
        tracing::warn!(
          stage = "url-blocklist",
          host = %host,
          pattern = %pattern,
          "moderation: URL blocklist hit"
        );
        return Verdict::block(1.0, format!("命中链接黑名单: {} (规则 {})", host, pattern));
      }
    }
    Verdict::allow()
  }
}

// ────────────────────────────────────────────────────────────
// URL 扫描：从任意文本中提取 http(s) URL
// ────────────────────────────────────────────────────────────

/// 扫文本里所有 http(s) URL。返回原字符串切片，按出现顺序、不去重。
pub fn extract_urls(text: &str) -> Vec<&str> {
  let mut out = Vec::new();
  let bytes = text.as_bytes();
  let len = bytes.len();
  let mut i = 0;
  while i < len {
    // 找到 `http://` 或 `https://`
    let start = match find_scheme(&bytes[i..]) {
      Some(off) => i + off,
      None => break,
    };
    // 从 start 向后扫到首个非 URL 字符
    let url_start_byte = start;
    let mut j = start;
    while j < len && is_url_byte(bytes[j]) {
      j += 1;
    }
    // 去掉常见结尾标点（句号 / 逗号 / 分号 / 引号 / 括号）
    while j > url_start_byte
      && matches!(bytes[j - 1], b'.' | b',' | b';' | b'!' | b'?' | b')' | b']' | b'"' | b'\'')
    {
      j -= 1;
    }
    if j > url_start_byte + 8 {
      // 至少要包含 scheme + `://` + 1 个字符
      // SAFETY: scheme + URL chars 均为 ASCII，切片对齐字符边界
      out.push(&text[url_start_byte..j]);
    }
    i = j.max(url_start_byte + 1);
  }
  out
}

/// 找到 `http://` 或 `https://` 起始的偏移。
fn find_scheme(bytes: &[u8]) -> Option<usize> {
  let needles: &[&[u8]] = &[b"http://", b"https://"];
  let mut best: Option<usize> = None;
  for n in needles {
    if let Some(p) = find_subslice(bytes, n) {
      best = Some(match best {
        Some(b) => b.min(p),
        None => p,
      });
    }
  }
  best
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
  if needle.is_empty() || hay.len() < needle.len() {
    return None;
  }
  (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

/// URL 允许字符集合（RFC 3986 unreserved + reserved + 常见安全字符）。
/// 故意从严：不接受空白、中英文标点、引号等。
fn is_url_byte(b: u8) -> bool {
  matches!(b,
    b'A'..=b'Z'
    | b'a'..=b'z'
    | b'0'..=b'9'
    | b'-' | b'.' | b'_' | b'~'                    // unreserved
    | b':' | b'/' | b'?' | b'#' | b'[' | b']' | b'@'   // gen-delims
    | b'!' | b'$' | b'&' | b'\'' | b'(' | b')'      // sub-delims (部分)
    | b'*' | b'+' | b',' | b';' | b'='
    | b'%'                                          // pct-encoded
  )
}

// ────────────────────────────────────────────────────────────
// host 提取 + 模式匹配
// ────────────────────────────────────────────────────────────

/// 从 URL 提取 host（不含 scheme / 用户名密码 / 端口 / 路径）。
/// 返回 Some 时保证 host 至少 1 个字符。
pub fn host_of(url: &str) -> Option<&str> {
  let after_scheme = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);
  // 跳过 userinfo
  let after_userinfo = match after_scheme.rfind('@') {
    Some(i) if i < after_scheme.len() => &after_scheme[i + 1..],
    _ => after_scheme,
  };
  // 截到 path / query / fragment / port 之前
  let end = after_userinfo
    .find(['/', '?', '#', ':'])
    .unwrap_or(after_userinfo.len());
  let host = &after_userinfo[..end];
  if host.is_empty() {
    None
  } else {
    Some(host)
  }
}

/// 把 host 与黑名单 patterns 匹配。命中返回匹配到的 pattern 字符串。
/// 匹配规则：
/// - `"scam.com"` 精确匹配 host
/// - `"*.evil.com"` 匹配 host 为 `evil.com` 或以 `.evil.com` 结尾
///
/// 不区分大小写；调用方保证 patterns 已 lowercase。
pub fn match_blocklist<'a>(host: &str, patterns: &'a [String]) -> Option<&'a str> {
  let host_lower = host.to_ascii_lowercase();
  for pat in patterns {
    if let Some(suffix) = pat.strip_prefix("*.") {
      if host_lower == suffix || host_lower.ends_with(&format!(".{}", suffix)) {
        return Some(pat);
      }
    } else if host_lower == *pat {
      return Some(pat);
    }
  }
  None
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use rustineverything_core::engines::moderation::ModerationLabel;

  // ── URL 提取 ─────────────────────────────────────────────

  #[test]
  fn extract_single_url() {
    let urls = extract_urls("visit https://example.com/path now");
    assert_eq!(urls, vec!["https://example.com/path"]);
  }

  #[test]
  fn extract_multiple_urls() {
    let urls = extract_urls("see http://a.com and https://b.co/x?y=1 ok");
    assert_eq!(urls, vec!["http://a.com", "https://b.co/x?y=1"]);
  }

  #[test]
  fn extract_strips_trailing_punctuation() {
    let urls = extract_urls("check https://example.com.");
    assert_eq!(urls, vec!["https://example.com"]);

    let urls = extract_urls("(see https://example.com), then go.");
    assert_eq!(urls, vec!["https://example.com"]);
  }

  #[test]
  fn extract_returns_empty_when_no_url() {
    assert!(extract_urls("just plain text 没有链接").is_empty());
  }

  #[test]
  fn extract_handles_chinese_around_url() {
    let urls = extract_urls("点击这里 https://example.com 进入");
    assert_eq!(urls, vec!["https://example.com"]);
  }

  #[test]
  fn extract_handles_uppercase_scheme() {
    let urls = extract_urls("HTTPS://EXAMPLE.COM/path");
    assert_eq!(urls, vec!["HTTPS://EXAMPLE.COM/path"]);
  }

  #[test]
  fn extract_handles_port_and_query() {
    let urls = extract_urls("https://example.com:8080/a?b=1&c=2#frag");
    assert_eq!(urls, vec!["https://example.com:8080/a?b=1&c=2#frag"]);
  }

  #[test]
  fn extract_skips_bare_scheme() {
    // 仅 "http://"（无 host）不应被收
    assert!(extract_urls("see http:// then nothing").is_empty());
  }

  // ── host 提取 ─────────────────────────────────────────────

  #[test]
  fn host_of_basic() {
    assert_eq!(host_of("https://example.com/path"), Some("example.com"));
    assert_eq!(host_of("http://sub.example.com"), Some("sub.example.com"));
  }

  #[test]
  fn host_of_with_port() {
    assert_eq!(host_of("https://example.com:8080/a"), Some("example.com"));
  }

  #[test]
  fn host_of_strips_userinfo() {
    assert_eq!(host_of("https://user:pass@example.com/x"), Some("example.com"));
  }

  #[test]
  fn host_of_no_scheme_uses_raw() {
    // 不是常见路径，但应当不 panic
    assert_eq!(host_of("example.com/path"), Some("example.com"));
  }

  // ── 模式匹配 ─────────────────────────────────────────────

  #[test]
  fn match_exact() {
    let pats = vec!["scam.com".to_string(), "other.org".to_string()];
    assert_eq!(match_blocklist("scam.com", &pats), Some("scam.com"));
    assert_eq!(match_blocklist("other.org", &pats), Some("other.org"));
    assert!(match_blocklist("safe.com", &pats).is_none());
  }

  #[test]
  fn match_wildcard_matches_subdomains_and_apex() {
    let pats = vec!["*.evil.com".to_string()];
    assert_eq!(match_blocklist("evil.com", &pats), Some("*.evil.com"));
    assert_eq!(match_blocklist("a.evil.com", &pats), Some("*.evil.com"));
    assert_eq!(match_blocklist("deep.sub.evil.com", &pats), Some("*.evil.com"));
    assert!(match_blocklist("notevil.com", &pats).is_none());
  }

  #[test]
  fn match_case_insensitive() {
    let pats = vec!["scam.com".to_string()];
    assert_eq!(match_blocklist("SCAM.com", &pats), Some("scam.com"));
    assert_eq!(match_blocklist("ScAm.CoM", &pats), Some("scam.com"));
  }

  // ── stage 集成 ──────────────────────────────────────────

  #[tokio::test]
  async fn stage_blocks_on_blocklist_hit() {
    let stage = UrlBlocklistStage::new(vec!["scam.com"]);
    let v = stage.evaluate(&ModerationSubmission::new("快来 https://scam.com/x 领奖")).await;
    assert_eq!(v.label, ModerationLabel::Block);
    assert!(v.reason.contains("scam.com"));
    assert!((v.score - 1.0).abs() < f32::EPSILON);
  }

  #[tokio::test]
  async fn stage_allows_when_no_url() {
    let stage = UrlBlocklistStage::new(vec!["scam.com"]);
    let v = stage.evaluate(&ModerationSubmission::new("纯文字评论，没有链接")).await;
    assert_eq!(v.label, ModerationLabel::Allow);
  }

  #[tokio::test]
  async fn stage_allows_when_url_not_in_blocklist() {
    let stage = UrlBlocklistStage::new(vec!["scam.com"]);
    let v = stage.evaluate(&ModerationSubmission::new("see https://safe.example/x")).await;
    assert_eq!(v.label, ModerationLabel::Allow);
  }

  #[tokio::test]
  async fn stage_with_empty_patterns_always_allows() {
    let stage = UrlBlocklistStage::new(Vec::<String>::new());
    assert!(stage.is_empty());
    let v = stage.evaluate(&ModerationSubmission::new("https://scam.com 但黑名单空")).await;
    assert_eq!(v.label, ModerationLabel::Allow);
  }

  #[tokio::test]
  async fn stage_wildcard_subdomain_blocks() {
    let stage = UrlBlocklistStage::new(vec!["*.phishing.example"]);
    let v = stage
      .evaluate(&ModerationSubmission::new(
        "尊敬的用户，请访问 https://login.phishing.example/verify",
      ))
      .await;
    assert_eq!(v.label, ModerationLabel::Block);
    assert!(v.reason.contains("login.phishing.example"));
  }

  #[tokio::test]
  async fn stage_blanks_and_empty_patterns_are_filtered() {
    // 用户在 site.json 写了空字符串或空白
    let stage = UrlBlocklistStage::new(vec!["", "  ", "evil.com"]);
    assert_eq!(stage.patterns().len(), 1);
    assert_eq!(stage.patterns()[0], "evil.com");
  }
}
