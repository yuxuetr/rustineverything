//! 纯文本工具函数（前后端共享，可单元测试）。

/// 去除 Markdown frontmatter (`---\n...\n---\n`)。
pub fn strip_frontmatter(raw: &str) -> &str {
  if !raw.starts_with("---") {
    return raw;
  }
  let after = &raw[3..];
  if let Some(end) = after.find("\n---") {
    let rest_start = end + 4;
    let rest = &after[rest_start..];
    return rest.strip_prefix('\n').unwrap_or(rest);
  }
  raw
}

/// 简单 Markdown -> 纯文本转换:
/// - 去除常见的 Markdown 标记符号(#, *, _, `, !\[\]\(\), \[\]\(\), >)
/// - 多空行折叠为一行
/// - 保留中英文标点
pub fn markdown_to_plain(md: &str) -> String {
  let body = strip_frontmatter(md);
  let mut out = String::with_capacity(body.len());
  let mut prev_blank = false;
  for line in body.lines() {
    let cleaned = clean_line(line);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
      if !prev_blank && !out.is_empty() {
        out.push('\n');
      }
      prev_blank = true;
    } else {
      out.push_str(trimmed);
      out.push('\n');
      prev_blank = false;
    }
  }
  out.trim().to_string()
}

fn clean_line(line: &str) -> String {
  let mut s = line.to_string();

  // 去除代码块围栏(` ``` `开头)
  if s.trim_start().starts_with("```") {
    return String::new();
  }
  // 去除常见前缀符号
  let trimmed = s.trim_start();
  if let Some(rest) = trimmed
    .strip_prefix("# ")
    .or_else(|| trimmed.strip_prefix("## "))
    .or_else(|| trimmed.strip_prefix("### "))
    .or_else(|| trimmed.strip_prefix("#### "))
    .or_else(|| trimmed.strip_prefix("##### "))
    .or_else(|| trimmed.strip_prefix("###### "))
    .or_else(|| trimmed.strip_prefix("> "))
    .or_else(|| trimmed.strip_prefix("- "))
    .or_else(|| trimmed.strip_prefix("* "))
    .or_else(|| trimmed.strip_prefix("+ "))
  {
    s = rest.to_string();
  }

  // 移除 inline 标记
  let cleaned = s.replace("**", "").replace("__", "").replace('`', "").replace("~~", "");

  // 处理 Markdown 链接 [text](url) 和图片 ![alt](url) -> 仅保留文字
  strip_md_links(&cleaned)
}

fn strip_md_links(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let mut pos = 0;
  while pos < bytes.len() {
    // 图片 ![alt](url)
    if bytes[pos] == b'!' && pos + 1 < bytes.len() && bytes[pos + 1] == b'[' {
      if let Some((text, consumed)) = parse_full_link(&s[pos + 1..]) {
        out.push_str(&text);
        pos += 1 + consumed;
        continue;
      }
    }
    // 链接 [text](url)
    if bytes[pos] == b'[' {
      if let Some((text, consumed)) = parse_full_link(&s[pos..]) {
        out.push_str(&text);
        pos += consumed;
        continue;
      }
    }
    // 拷贝当前 UTF-8 字符
    let next = next_char_boundary(s, pos);
    out.push_str(&s[pos..next]);
    pos = next;
  }
  out
}

/// 从 `[` 开始解析完整的 `[text](url)` 结构。
/// 返回(text, 消耗的字节数含头尾括号)。
fn parse_full_link(s: &str) -> Option<(String, usize)> {
  if !s.starts_with('[') {
    return None;
  }
  let close_bracket = s.find(']')?;
  if close_bracket < 1 {
    return None;
  }
  let after_close = &s[close_bracket + 1..];
  if !after_close.starts_with('(') {
    return None;
  }
  let close_paren = after_close.find(')')?;
  let text = s[1..close_bracket].to_string();
  // 字节消耗: '[' + text + ']' + '(' + url + ')' = close_bracket + 1 + close_paren + 1
  let consumed = close_bracket + 1 + close_paren + 1;
  Some((text, consumed))
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
  let mut i = pos + 1;
  while i < s.len() && !s.is_char_boundary(i) {
    i += 1;
  }
  i
}

/// 取前 N 个字符,过长截断并加 ellipsis。
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
  let mut end = s.len();
  for (count, (idx, _)) in s.char_indices().enumerate() {
    if count == max_chars {
      end = idx;
      break;
    }
  }
  if end >= s.len() {
    s.to_string()
  } else {
    let mut out = s[..end].to_string();
    out.push('…');
    out
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn strip_frontmatter_basic() {
    let raw = "---\ntitle: Hello\n---\n\n# Body";
    assert_eq!(strip_frontmatter(raw), "\n# Body");
  }

  #[test]
  fn strip_frontmatter_none() {
    let raw = "# Heading\n\nBody";
    assert_eq!(strip_frontmatter(raw), "# Heading\n\nBody");
  }

  #[test]
  fn strip_frontmatter_unterminated_kept() {
    let raw = "---\nincomplete\n";
    assert_eq!(strip_frontmatter(raw), raw);
  }

  #[test]
  fn markdown_to_plain_strips_marks() {
    let md = "# Title\n\n**bold** and *italic* and `code`\n\n> quote\n";
    let plain = markdown_to_plain(md);
    assert!(plain.contains("Title"));
    assert!(plain.contains("bold"));
    assert!(plain.contains("italic"));
    assert!(plain.contains("code"));
    assert!(plain.contains("quote"));
    assert!(!plain.contains('#'));
    assert!(!plain.contains("**"));
  }

  #[test]
  fn markdown_to_plain_strips_code_fence() {
    let md = "intro\n\n```rust\nfn foo() {}\n```\n\noutro";
    let plain = markdown_to_plain(md);
    assert!(plain.contains("intro"));
    assert!(plain.contains("outro"));
    // 围栏行被 drop,代码内容会保留(简化策略)
    // 但围栏符号本身不应出现
    assert!(!plain.contains("```"));
  }

  #[test]
  fn markdown_to_plain_handles_links() {
    let md = "see [Rust](https://rust-lang.org) for more.";
    let plain = markdown_to_plain(md);
    assert!(plain.contains("Rust"));
    assert!(!plain.contains("https"));
    assert!(!plain.contains('('));
  }

  #[test]
  fn markdown_to_plain_handles_images() {
    let md = "![logo](https://example.com/a.png) caption";
    let plain = markdown_to_plain(md);
    assert!(plain.contains("logo"));
    assert!(plain.contains("caption"));
    assert!(!plain.contains("https"));
  }

  #[test]
  fn markdown_to_plain_chinese() {
    let md = "# 你好世界\n\n这是一段 **加粗** 的 *中文* 内容。";
    let plain = markdown_to_plain(md);
    assert!(plain.contains("你好世界"));
    assert!(plain.contains("加粗"));
    assert!(plain.contains("中文"));
    assert!(!plain.contains('#'));
  }

  #[test]
  fn truncate_chars_short_unchanged() {
    assert_eq!(truncate_chars("abc", 10), "abc");
  }

  #[test]
  fn truncate_chars_ascii() {
    assert_eq!(truncate_chars("abcdefg", 3), "abc…");
  }

  #[test]
  fn truncate_chars_chinese_safe() {
    let s = "你好世界搜索";
    let t = truncate_chars(s, 2);
    assert_eq!(t, "你好…");
  }

  #[test]
  fn truncate_chars_zero() {
    assert_eq!(truncate_chars("abc", 0), "…");
  }
}
