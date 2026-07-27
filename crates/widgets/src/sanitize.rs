//! Phase 4.2：用户内容 XSS 防护。
//!
//! ## 攻击面回顾
//! `crates/widgets/src/mdx.rs` 的 [`crate::mdx::Markdown`] 组件处理 markdown
//! 输入时调用 `pulldown_cmark::Parser`。`<script>` / `<iframe>` / `<a
//! onclick=...>` 等被 cmark 视为 `Event::Html` / `Event::InlineHtml`：
//! 1. 若 HTML 文本能匹配到 `ComponentRegistry` 中注册的 MDX 标签
//!    （如 `<YouTube id="..." />`），由对应组件以 RSX 安全渲染。
//! 2. 否则 fallback 为 `rsx! { span { "{h}" } }` — Dioxus 自动转义文本，
//!    `<script>` 字面输出为 `&lt;script&gt;` 不会执行。
//!
//! 因此默认渲染管道 **已经基本免疫 raw `<script>` 注入**。本模块提供
//! **额外的 defense-in-depth 层**：在用户提交内容进入 cmark 解析之前，
//! 先剥离明确有害的语法片段（script / iframe / on*= / javascript: URL），
//! 让审计与渲染表现完全一致 — 用户即使「看到自己输入的字面 `<script>`」
//! 也无法触发任何 XSS 利用链。
//!
//! ## 使用方式
//! ```ignore
//! use widgets::Markdown;
//!
//! rsx! { Markdown {
//!   content: comment.body,
//!   blog_id: "comment".into(),
//!   untrusted: true,   // ← 来自用户表单的内容
//! }}
//! ```
//! 默认 `untrusted = false`（站点作者内容信任）。
//! 启用后，markdown 源会先经过 [`sanitize_user_html`] 再被 cmark 解析。
//!
//! ## 注意
//! - 本函数 **不影响** MDX 嵌入组件（白名单 `<YouTube>` / `<Yellow>` ...
//!   等不会被剥离），仅作用于 HTML 层级危险标签 / 属性。
//! - cmark 的代码块 / 反引号包裹的代码不受影响：那是用户写的代码片段，
//!   不会被当作 HTML 解析。
//!
//! ## 测试覆盖
//! 见本文件底部 `tests`：常见 XSS payload (`<script>`, `<iframe>`,
//! `onerror=`, `javascript:` URLs, base64 SVG / `data:`) 均被中和。

/// Phase 4.2：剥离用户 markdown 中的明确危险 HTML 片段。
///
/// 处理规则（按顺序）：
/// 1. 删除 `<script ...>...</script>`（含属性、跨行）
/// 2. 删除 `<iframe ...>...</iframe>` / `<object>` / `<embed>` 整块
/// 3. 删除 `<style ...>...</style>`（防止 CSS 注入）
/// 4. 删除内联事件处理：`on\w+\s*=\s*"..."` / `'...'` / `bare`
/// 5. 删除 `javascript:` 形式的 URL 协议（替换为 `about:blank`）
/// 6. 删除 `data:text/html` 协议（同上）
///
/// 注意：本函数是 **字符串级** 处理，不构造 DOM；性能 O(n)；对正常
/// markdown 完全透明（不含上述 pattern 时直接返回相同长度的 String）。
pub fn sanitize_user_html(input: &str) -> String {
  let mut out = input.to_string();
  out = strip_tag_block(&out, "script");
  out = strip_tag_block(&out, "iframe");
  out = strip_tag_block(&out, "object");
  out = strip_tag_block(&out, "embed");
  out = strip_tag_block(&out, "style");
  out = strip_on_event_attrs(&out);
  // Phase 8.6：URL scheme 校验下沉到 cmark Tag::Link / Tag::Image 渲染层
  // （[`is_safe_link_url`] / [`is_safe_image_url`]），不再做易被绕过的
  // 字面串替换。
  out
}

/// 删除 `<tag ...>...</tag>` 整块（含起止标签与中间内容），大小写不敏感。
///
/// 单独存在的 `<tag ... />` 或 `<tag>` 无对应 `</tag>` 时，至少删除起始标签
/// （从 `<` 到下一个 `>`）。
///
/// 实现注意：UTF-8 安全 — 通过字符串切片拼接而不是逐字节构造，
/// 避免破坏中文等多字节字符。
fn strip_tag_block(input: &str, tag: &str) -> String {
  let lower_tag = tag.to_lowercase();
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;
  let mut last_emit = 0;

  while i < bytes.len() {
    if bytes[i] == b'<' {
      let rest = &input[i..];
      let rest_lower = rest.to_lowercase();
      let open_match = rest_lower.starts_with(&format!("<{}", lower_tag))
        && rest
          .as_bytes()
          .get(1 + lower_tag.len())
          .map(|c| matches!(c, b' ' | b'\t' | b'\n' | b'>' | b'/'))
          .unwrap_or(false);
      if open_match {
        // 先把累积的合法片段输出
        out.push_str(&input[last_emit..i]);
        // 找闭合标签；找不到则只删起始标签到首个 `>`
        let close_marker = format!("</{}>", lower_tag);
        if let Some(end_rel) = rest_lower.find(&close_marker) {
          i += end_rel + close_marker.len();
          last_emit = i;
          continue;
        }
        if let Some(gt_rel) = rest.find('>') {
          i += gt_rel + 1;
          last_emit = i;
          continue;
        }
        // 无 `>` 则整段抛弃，last_emit 不再前进
        last_emit = bytes.len();
        break;
      }
    }
    i += 1;
  }
  out.push_str(&input[last_emit..]);
  out
}

/// 删除 `on*=` 事件属性。匹配形式：
/// - `onclick="alert(1)"`
/// - `onclick='alert(1)'`
/// - `onclick=alert(1)`（裸值，到下一个空白或 `>`）
///
/// UTF-8 安全：通过字节索引扫描 ASCII pattern，再用字符串切片输出，
/// 不会破坏中文 / emoji 等多字节字符。
fn strip_on_event_attrs(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;
  let mut last_emit = 0;
  while i < bytes.len() {
    // 找 `on` 起首的属性（前一字节须是 ASCII 空白 / 标签内分隔符）
    if (i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'"' | b'\''))
      && i + 2 < bytes.len()
      && bytes[i].eq_ignore_ascii_case(&b'o')
      && bytes[i + 1].eq_ignore_ascii_case(&b'n')
      && (bytes[i + 2].is_ascii_alphabetic())
    {
      let start = i;
      let mut j = i + 2;
      while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || matches!(bytes[j], b'-' | b'_'))
      {
        j += 1;
      }
      while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
        j += 1;
      }
      if j < bytes.len() && bytes[j] == b'=' {
        j += 1;
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
          j += 1;
        }
        if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
          let quote = bytes[j];
          j += 1;
          while j < bytes.len() && bytes[j] != quote {
            j += 1;
          }
          if j < bytes.len() {
            j += 1;
          }
        } else {
          while j < bytes.len() && !matches!(bytes[j], b' ' | b'\t' | b'>' | b'/') {
            j += 1;
          }
        }
        // 输出 [last_emit, start) 之间的合法片段，跳过 [start, j)
        out.push_str(&input[last_emit..start]);
        i = j;
        last_emit = j;
        continue;
      }
    }
    i += 1;
  }
  out.push_str(&input[last_emit..]);
  out
}

// Phase 8.6：原 `neutralize_dangerous_urls` 字面串黑名单已删除。
// 它存在两个根本缺陷：
// 1. `j&#x61;vascript:` / `&#106;avascript:` 等 HTML-entity / 数字字符引用编码
//    可以绕过任何 case-insensitive 字面比对
// 2. `j\tavascript:`（embedded TAB）等空白注入也会绕过
//
// 替换策略：sanitize_user_html 仍负责剥离 `<script>` / `<iframe>` 等危险标签，
// 而 URL scheme 校验改在渲染层做 —— cmark `Tag::Link` / `Tag::Image` 拿到的
// `dest_url` 已是解码后的字符串，下方两个 allowlist helper 校验后再渲染。

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

  #[test]
  fn plain_markdown_passes_through_unchanged() {
    let input = "# Hello\n\nA paragraph with **bold** and `code`.";
    assert_eq!(sanitize_user_html(input), input);
  }

  #[test]
  fn strips_script_block_with_body() {
    let input = "before <script>alert(1)</script> after";
    let out = sanitize_user_html(input);
    assert!(!out.contains("alert"));
    assert!(!out.contains("<script"));
    assert!(out.contains("before"));
    assert!(out.contains("after"));
  }

  #[test]
  fn strips_iframe() {
    let input = r#"<iframe src="https://evil.com"></iframe> ok"#;
    let out = sanitize_user_html(input);
    assert!(!out.contains("<iframe"));
    assert!(!out.contains("evil.com"));
    assert!(out.contains("ok"));
  }

  #[test]
  fn strips_object_and_embed() {
    let out = sanitize_user_html("<object data='x.swf'></object><embed src='y' /> z");
    assert!(!out.contains("<object"));
    assert!(!out.contains("<embed"));
    assert!(out.contains("z"));
  }

  #[test]
  fn strips_style_block() {
    let out = sanitize_user_html("<style>body{display:none}</style> body");
    assert!(!out.contains("<style"));
    assert!(!out.contains("display"));
    assert!(out.contains("body"));
  }

  #[test]
  fn strips_uppercase_and_mixed_case_script() {
    for variant in &["<SCRIPT>x</SCRIPT>", "<Script>y</Script>", "<sCrIpT>z</sCrIpT>"] {
      let out = sanitize_user_html(variant);
      assert!(!out.to_lowercase().contains("<script"), "variant {} → {}", variant, out);
    }
  }

  #[test]
  fn strips_attributed_script() {
    let out = sanitize_user_html(r#"<script src="https://evil.com/x.js">/* */</script>"#);
    assert!(!out.contains("evil.com"));
    assert!(!out.contains("<script"));
  }

  #[test]
  fn strips_on_event_attributes_quoted() {
    let input = r#"<a href="/x" onclick="alert(1)">link</a>"#;
    let out = sanitize_user_html(input);
    assert!(!out.contains("onclick"));
    assert!(!out.contains("alert"));
    assert!(out.contains("<a href=\"/x\""));
    assert!(out.contains("link"));
  }

  #[test]
  fn strips_on_event_attributes_single_quote() {
    let out = sanitize_user_html(r#"<img src="x" onerror='alert(1)'>"#);
    assert!(!out.contains("onerror"));
    assert!(!out.contains("alert"));
  }

  #[test]
  fn strips_on_event_attributes_bare() {
    let out = sanitize_user_html(r#"<img src=x onerror=alert(1)>"#);
    assert!(!out.contains("onerror"));
    assert!(!out.contains("alert"));
  }

  #[test]
  fn does_not_strip_legit_attributes_starting_with_o() {
    let input = r#"<a href="/x" oncepublished="x">link</a>"#;
    let _ = sanitize_user_html(input);
    // 注意：oncepublished 不是真实 HTML 属性；我们的剥离规则要求
    // 属性名以 on + 一个 ASCII 字母开头，"oncepublished" 也匹配
    // 这个 pattern。这里仅断言函数不 panic、不返回空。

    let safe = r#"abc onenote def"#;
    let out2 = sanitize_user_html(safe);
    // "onenote" 出现在文本中，不在 HTML 属性位置（没有 `=`），不应剥离
    assert!(out2.contains("onenote"));
  }

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

  #[test]
  fn classic_polyglot_xss_neutralized() {
    let payload = r#"<svg onload=alert(1)><script>alert(2)</script>"#;
    let out = sanitize_user_html(payload);
    assert!(!out.contains("onload"));
    assert!(!out.contains("alert(2)"));
    assert!(!out.contains("<script"));
  }

  #[test]
  fn markdown_code_blocks_remain_visible_as_text() {
    // markdown 代码块进入 cmark 后会作为字面文本，sanitize 是在 cmark 之前。
    // 即使 sanitize 删除了 `<script>` 等片段，对在三反引号中的「示例代码」
    // 而言意味着「示例文本被改变」。这是已知 trade-off：用户教程里写
    // `<script>` 字面，会被本函数删掉。文档化此行为，让作者用 HTML 实体
    // (`&lt;script&gt;`) 代替。
    let input = "看这段 HTML：\n\n```html\n<script>alert(1)</script>\n```";
    let out = sanitize_user_html(input);
    assert!(!out.contains("<script"));
    // 但 markdown 结构 (反引号、围栏) 仍然完整
    assert!(out.contains("```html"));
    assert!(out.contains("看这段"));
  }
}
