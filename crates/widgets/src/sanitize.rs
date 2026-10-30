//! 用户内容的 URL 白名单（Phase 4.2 / 8.6）。
//!
//! 用户 markdown 里的原始 HTML 不靠这里处理：`crate::mdx::Markdown` 在
//! `untrusted` 时把 `Event::Html` / `Event::InlineHtml` 一律当文本输出（Dioxus 转义），
//! 只有注册过的 MDX 组件按组件渲染，组件只读取自己认识的属性。测试见
//! `mdx::tests::untrusted_raw_html_never_becomes_markup`。
//!
//! 链接与图片的地址在渲染层（cmark `Tag::Link` / `Tag::Image` 拿到的 `dest_url`
//! 已解码）用下面的 allowlist 校验：先解码 HTML 实体、去掉控制字符，再比对 scheme，
//! `j&#x61;vascript:`、`j\tavascript:` 这类变体都过不去。

/// 允许出现在 `<a href>` 的 URL scheme allowlist。
///
/// 检查策略：
/// 1. 先用 [`decode_html_entities`] 解码所有 `&#xNN;` / `&#NN;` / `&name;` 实体
/// 2. trim 前后 ASCII 空白 + 内部 ASCII 控制字符（包括 TAB / LF / CR / 零宽）
/// 3. lowercase 后取 `:` 之前的 scheme 部分，与白名单（`http` / `https` /
///    `mailto` / `tel`）比对
/// 4. 没有 `:` 视为相对路径或 `/` 起始的绝对路径，统一放行
pub fn is_safe_link_url(url: &str) -> bool {
  scheme_is_in(url, &["http", "https", "mailto", "tel"])
}

/// `<img src>` 的 allowlist：除了 link 的几个 scheme，再加 `data:image/`
/// 子集（base64 图片在 markdown 里有合法用途，但禁用 `data:text/html` 等）。
pub fn is_safe_image_url(url: &str) -> bool {
  // 图片专属：相对路径 / `/` 直接 OK
  let normalized = normalize_url_for_scheme_check(url);
  let lower = normalized.to_ascii_lowercase();
  if lower.starts_with("data:") {
    // data: 仅放 image/ 子型；data:text/html、data:application/* 都禁
    return lower.starts_with("data:image/");
  }
  scheme_is_in(&normalized, &["http", "https"])
}

/// 不可信内容（评论 / 话题等）里的图片只允许本站上传的文件（SEC-09）。
///
/// 形状必须与 uploads 模块生成的文件名一致：`/uploads/` + 单个
/// `[A-Za-z0-9_-]+` 段 + 图片扩展名。不放行 `/`、`.`（扩展名分隔除外）、`%`、`\`，
/// 所以 `..` / `%2e%2e` / `\..\` 这类会被浏览器规范化到别处的写法都过不去。
pub fn is_upload_image_url(url: &str) -> bool {
  let Some((stem, ext)) = url.strip_prefix("/uploads/").and_then(|name| name.rsplit_once('.'))
  else {
    return false;
  };
  !stem.is_empty()
    && stem.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    && matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "webp")
}

/// 解码 HTML 实体 + 剥除内嵌的空白 / 控制字符后，再做 scheme 比对。
fn scheme_is_in(url: &str, allowed: &[&str]) -> bool {
  let normalized = normalize_url_for_scheme_check(url);
  // 没有 `:` → 相对 URL / fragment / query → 放行
  let Some(colon) = normalized.find(':') else {
    return true;
  };
  let scheme = normalized[..colon].to_ascii_lowercase();
  allowed.contains(&scheme.as_str())
}

/// 规范化 URL：解码 HTML 实体，剥除嵌入的 ASCII 控制字符 + 空白
/// （`javascript:` 攻击常用的 `j\tavascript:` / `J A V A SCRIPT:` 等变体被这里抹平）。
fn normalize_url_for_scheme_check(url: &str) -> String {
  let decoded = decode_html_entities(url);
  decoded
    .trim()
    .chars()
    .filter(|c| {
      // 保留 ASCII visible + 非 ASCII；剔除控制字符 / 空白
      let v = *c;
      !(v.is_ascii_whitespace() || v.is_ascii_control())
    })
    .collect()
}

/// 简易 HTML 实体解码器：处理 `&#NN;` / `&#xNN;` / 常见命名实体。
///
/// 设计取舍：本函数只需要识别得了 `javascript:` 等危险 URL 的常见编码变体，
/// 不追求 WHATWG 全名实体表。未知实体保持原样，避免误伤合法链接。
fn decode_html_entities(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'&' {
      // 找 `;`，最大扫描 8 字节
      let end = (i + 1..bytes.len().min(i + 10)).find(|&j| bytes[j] == b';');
      if let Some(semi) = end {
        let entity = &input[i + 1..semi];
        if let Some(decoded) = decode_entity_body(entity) {
          out.push(decoded);
          i = semi + 1;
          continue;
        }
      }
    }
    // 安全 push：可能是多字节 UTF-8 起首；取当前字符整段
    let ch_len = utf8_char_len(bytes[i]);
    out.push_str(&input[i..i + ch_len]);
    i += ch_len;
  }
  out
}

fn utf8_char_len(b: u8) -> usize {
  // continuation byte alone (0x80..0xC0) shouldn't occur on valid UTF-8；
  // 与 ASCII 同样返回 1 是防御性兜底，确保不会无限循环。
  match b {
    0x00..=0xBF => 1,
    0xC0..=0xDF => 2,
    0xE0..=0xEF => 3,
    _ => 4,
  }
}

fn decode_entity_body(body: &str) -> Option<char> {
  if let Some(rest) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
    let n = u32::from_str_radix(rest, 16).ok()?;
    return char::from_u32(n);
  }
  if let Some(rest) = body.strip_prefix('#') {
    let n: u32 = rest.parse().ok()?;
    return char::from_u32(n);
  }
  match body {
    "amp" => Some('&'),
    "lt" => Some('<'),
    "gt" => Some('>'),
    "quot" => Some('"'),
    "apos" => Some('\''),
    "nbsp" => Some('\u{00A0}'),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Phase 8.6：原 `neutralizes_javascript_url` / `neutralizes_data_text_html`
  // 测试覆盖的「字面串黑名单」已删除，校验逻辑迁到 cmark Tag::Link / Tag::Image
  // 渲染层（[`is_safe_link_url`] / [`is_safe_image_url`]）。本组测试改成直接
  // 验证两个 allowlist helper，覆盖 HTML 实体 / 大小写 / 控制字符等绕过姿势。

  #[test]
  fn link_allowlist_accepts_safe_schemes() {
    assert!(is_safe_link_url("https://example.com/page"));
    assert!(is_safe_link_url("http://example.com"));
    assert!(is_safe_link_url("mailto:hi@example.com"));
    assert!(is_safe_link_url("tel:+8613800138000"));
    // 相对 / 锚点 / 查询 → 允许
    assert!(is_safe_link_url("/blog/welcome"));
    assert!(is_safe_link_url("#section-2"));
    assert!(is_safe_link_url("?page=2"));
    assert!(is_safe_link_url("relative/page.html"));
  }

  #[test]
  fn link_allowlist_rejects_dangerous_schemes() {
    assert!(!is_safe_link_url("javascript:alert(1)"));
    assert!(!is_safe_link_url("data:text/html,<script>alert(1)</script>"));
    assert!(!is_safe_link_url("file:///etc/passwd"));
    assert!(!is_safe_link_url("vbscript:msgbox(1)"));
  }

  /// Phase 8.6 关键测试：HTML 实体 + 大小写 + 制表符 / 空白嵌入绕过姿势。
  /// 任何一项打穿都意味着 XSS。
  #[test]
  fn link_allowlist_resists_known_bypasses() {
    // HTML 数字实体（十六进制）
    assert!(!is_safe_link_url("j&#x61;vascript:alert(1)"));
    assert!(!is_safe_link_url("&#x6A;avascript:alert(1)"));
    // HTML 数字实体（十进制）
    assert!(!is_safe_link_url("&#106;avascript:alert(1)"));
    // 大小写混合
    assert!(!is_safe_link_url("JaVaScRiPt:alert(1)"));
    assert!(!is_safe_link_url("JAVASCRIPT:alert(1)"));
    // TAB / CR / LF 嵌入
    assert!(!is_safe_link_url("j\tavascript:alert(1)"));
    assert!(!is_safe_link_url("j\navascript:alert(1)"));
    assert!(!is_safe_link_url("j\ra\tvascript:alert(1)"));
    // 前后空白
    assert!(!is_safe_link_url("   javascript:alert(1)"));
    assert!(!is_safe_link_url("javascript :alert(1)"));
    // 命名实体绕过 colon
    assert!(!is_safe_link_url("javascript&#58;alert(1)"));
  }

  #[test]
  fn image_allowlist_allows_relative_and_data_image() {
    // 相对路径 / 绝对路径 → 允许（覆盖 markdown 内嵌图片场景）
    assert!(is_safe_image_url("/uploads/x.png"));
    assert!(is_safe_image_url("welcome/hero.jpg"));
    // https → 允许
    assert!(is_safe_image_url("https://cdn.example.com/x.png"));
    // data:image/* → 允许；data:text/html → 禁
    assert!(is_safe_image_url("data:image/png;base64,iVBORw0KGgo="));
    assert!(is_safe_image_url("data:image/svg+xml;base64,PHN2Zw=="));
    assert!(!is_safe_image_url("data:text/html,<script>alert(1)</script>"));
    assert!(!is_safe_image_url("javascript:alert(1)"));
  }

  #[test]
  fn upload_image_url_only_accepts_uploaded_file_names() {
    assert!(is_upload_image_url("/uploads/1700000000_cat_Ab12Cd.png"));
    assert!(is_upload_image_url("/uploads/a-b_c.jpg"));
    assert!(is_upload_image_url("/uploads/x.jpeg"));
    assert!(is_upload_image_url("/uploads/x.gif"));
    assert!(is_upload_image_url("/uploads/x.webp"));
    for bad in [
      "/api/auth/logout",
      "/uploads/../api/auth/logout",
      "/uploads/..%2fapi/auth/logout",
      "/uploads/%2e%2e/api/x.png",
      "/uploads\\..\\api\\x.png",
      "/uploads/a/b.png",
      "/uploads/.png",
      "/uploads/x.svg",
      "/uploads/x.png?x=1",
      "/uploads/x.png#a",
      "/uploads/",
      "uploads/x.png",
      "//evil.example/uploads/x.png",
      "https://evil.example/x.png",
      "data:image/png;base64,iVBORw0KGgo=",
      "welcome/hero.jpg",
    ] {
      assert!(!is_upload_image_url(bad), "{bad}");
    }
  }

  /// HTML 实体解码：覆盖三种形式。
  #[test]
  fn decode_html_entities_basic() {
    assert_eq!(decode_html_entities("a&amp;b"), "a&b");
    assert_eq!(decode_html_entities("a&lt;b&gt;c"), "a<b>c");
    assert_eq!(decode_html_entities("&#x6A;"), "j"); // hex
    assert_eq!(decode_html_entities("&#106;"), "j"); // dec
                                                     // 未知实体保持原样
    assert_eq!(decode_html_entities("&unknown;"), "&unknown;");
    // 没有 `;` 不解码
    assert_eq!(decode_html_entities("a&amp"), "a&amp");
  }
}
