//! 示例审核插件：DeepSeek（或任意 OpenAI / Anthropic 兼容 LLM）做评论审核。
//!
//! 本插件只管 **policy**（怎么问 LLM、怎么读懂回答），LLM 端点与协议由
//! 宿主（`crates/llm`）按 env 变量选择。详情见 docs/MODERATION_SPEC.md。
//!
//! ## 导出函数
//! - `get_manifest`：声明 capability=`moderation-provider`
//! - `moderation_build_prompt(submission_json) → Vec<LlmMessage> JSON`
//! - `moderation_parse_verdict(llm_text) → ModerationVerdict JSON`
//!
//! ## 构建
//! ```sh
//! CARGO_TARGET_DIR=/Users/hal/.target cargo build \
//!   -p plugin-moderation-deepseek \
//!   --target wasm32-unknown-unknown \
//!   --release
//! cp /Users/hal/.target/wasm32-unknown-unknown/release/plugin_moderation_deepseek.wasm \
//!    assets/plugins/
//! ```
//!
//! ## 启用
//! ```jsonc
//! // assets/site.json
//! {
//!   "moderation": {
//!     "enabled": true,
//!     "plugins": ["plugin_moderation_deepseek.wasm"]
//!   }
//! }
//! ```

use rustineverything_sdk::{capabilities, pack_json, read_input, PluginManifest};
use serde::{Deserialize, Serialize};

// ────────────────────────────────────────────────────────────
// Manifest
// ────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let m = PluginManifest::new(
    "moderation-deepseek",
    "Moderation (DeepSeek-compatible)",
    env!("CARGO_PKG_VERSION"),
  )
  .with_capability(capabilities::MODERATION_PROVIDER)
  .with_description("LLM 审核示例：提示模型输出 JSON 形式的 score/label/reason")
  .with_author("yuxuetr");
  pack_json(&m)
}

// ────────────────────────────────────────────────────────────
// 协议：与 SDK 中 ModerationSubmission / ModerationVerdict 同步。
// 不直接 use SDK 中的类型以保持插件最小依赖（避免 wasm 体积膨胀）。
// 字段对齐由两侧 JSON 字面值保证。
// ────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct Submission {
  #[serde(default)]
  content: String,
  #[serde(default)]
  kind: String,
  #[serde(default)]
  ref_path: String,
  /// 多模态：评论中夹带的图片（绝对 URL 或 data URL）。
  /// 老宿主不传该字段时默认空 → 行为退化为纯文本审核。
  #[serde(default)]
  images: Vec<ImageRef>,
}

#[derive(Deserialize, Default)]
struct ImageRef {
  #[serde(default)]
  url: String,
  // 接受但不用：plugin 不需要知道 media_type；它由宿主 (`crates/llm`) 在
  // 序列化到 LLM 时用（Anthropic 需要 media_type，OpenAI 从 URL 推断）。
  #[serde(default)]
  #[allow(dead_code)]
  media_type: String,
}

#[derive(Serialize)]
struct WireMessage<'a> {
  role: &'a str,
  /// 走 LlmContentBlock 数组：宿主 (`crates/llm`) 的 LlmMessage Deserializer
  /// 兼容字符串和数组两种形态，但有图片时必须用数组。
  /// 没有图片时统一也用数组（让宿主自己决定是否折叠回字符串）。
  content: Vec<WireContent<'a>>,
}

/// 与 `rustineverything_llm::LlmContentBlock` 字面对齐的内容块。
/// 插件 crate 不依赖 llm crate（避免 wasm 体积爆炸），手写同字面 JSON。
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContent<'a> {
  Text { text: &'a str },
  ImageUrl { url: &'a str },
}

#[derive(Serialize)]
struct Verdict<'a> {
  score: f32,
  label: &'a str,
  reason: String,
}

// ────────────────────────────────────────────────────────────
// build_prompt：把待审内容包装成两条 LLM 消息
// ────────────────────────────────────────────────────────────

/// 系统提示词：让模型严格输出 JSON。多模态维度只在 user message 含图时由模型考虑。
const SYSTEM_PROMPT: &str = r#"你是一个评论审核员，负责判断用户提交的内容是否合规。
判断维度（任意命中即视为有问题）：
1. 谩骂、人身攻击、歧视、骚扰
2. 色情、低俗、暴力血腥描写（**含图片**：色情、血腥、政治符号、令人不适的视觉元素）
3. 政治敏感、违法犯罪、煽动性内容
4. 垃圾广告、营销链接、无意义刷屏
5. 文本与图片不匹配的诱导（标题党 / 钓鱼）

**只输出一行 JSON**，不要 markdown 围栏、不要解释。字段：
{"score": 0.0-1.0, "label": "allow"|"flag"|"block", "reason": "≤30 字理由"}

评分约定：
- 0.0 ~ 0.49 → label="allow"（正常内容）
- 0.5 ~ 0.89 → label="flag"（可疑，需要复核）
- 0.9 ~ 1.0  → label="block"（明显违规）"#;

#[no_mangle]
pub unsafe extern "C" fn moderation_build_prompt(ptr: *mut u8, len: usize) -> u64 {
  let raw = read_input(ptr, len);
  let sub: Submission = serde_json::from_slice(raw).unwrap_or_default();

  // 文本块：把 kind / ref_path 当成附加上下文，方便模型针对场景调整严格度
  let user_text = if sub.kind.is_empty() && sub.ref_path.is_empty() {
    sub.content
  } else {
    format!(
      "[场景: {} {}]\n\n{}",
      if sub.kind.is_empty() { "comment" } else { &sub.kind },
      sub.ref_path,
      sub.content
    )
  };

  // 构造 user message 的多块内容：先文本，再图片。
  let mut user_blocks: Vec<WireContent> = Vec::with_capacity(1 + sub.images.len());
  user_blocks.push(WireContent::Text { text: &user_text });
  // images 字段会在 push 后被 borrow，因此这里需要保留 images 的所有权
  // 直到序列化结束（vec! 通过引用持有）。直接借 sub.images 即可。
  for img in &sub.images {
    if !img.url.is_empty() {
      user_blocks.push(WireContent::ImageUrl { url: &img.url });
    }
  }

  let messages = vec![
    WireMessage {
      role: "system",
      content: vec![WireContent::Text { text: SYSTEM_PROMPT }],
    },
    WireMessage {
      role: "user",
      content: user_blocks,
    },
  ];
  pack_json(&messages)
}

// ────────────────────────────────────────────────────────────
// parse_verdict：从 LLM 文本里抽出 JSON
// ────────────────────────────────────────────────────────────

/// 容错策略：
/// 1. 直接尝试解析整个 LLM 输出为 JSON
/// 2. 失败则用第一个 `{...}` 子串再试一次（应对模型偶尔包了 markdown 围栏）
/// 3. 都失败 → fail-open，返回 allow + 标注「解析失败」
#[no_mangle]
pub unsafe extern "C" fn moderation_parse_verdict(ptr: *mut u8, len: usize) -> u64 {
  let raw = read_input(ptr, len);
  let text = std::str::from_utf8(raw).unwrap_or("").trim();

  let parsed: Option<RawVerdict> = serde_json::from_str(text)
    .ok()
    .or_else(|| extract_first_json_object(text).and_then(|inner| serde_json::from_str(&inner).ok()));

  match parsed {
    Some(v) => {
      let label = normalize_label(&v.label);
      pack_json(&Verdict {
        score: clamp01(v.score),
        label,
        reason: v.reason.unwrap_or_default(),
      })
    }
    None => {
      // 解析失败 → fail-open
      pack_json(&Verdict {
        score: 0.0,
        label: "allow",
        reason: "插件解析失败，按 allow 处理".to_string(),
      })
    }
  }
}

#[derive(Deserialize)]
struct RawVerdict {
  #[serde(default)]
  score: f32,
  #[serde(default)]
  label: String,
  #[serde(default)]
  reason: Option<String>,
}

/// 从文本中抽第一个 `{...}` JSON 对象（处理 LLM 把 JSON 包在 markdown 围栏的情况）
fn extract_first_json_object(text: &str) -> Option<String> {
  let start = text.find('{')?;
  let mut depth = 0i32;
  for (i, ch) in text[start..].char_indices() {
    match ch {
      '{' => depth += 1,
      '}' => {
        depth -= 1;
        if depth == 0 {
          return Some(text[start..start + i + 1].to_string());
        }
      }
      _ => {}
    }
  }
  None
}

fn normalize_label(s: &str) -> &'static str {
  match s.trim().to_ascii_lowercase().as_str() {
    "block" => "block",
    "flag" => "flag",
    _ => "allow",
  }
}

fn clamp01(x: f32) -> f32 {
  if x.is_nan() {
    0.0
  } else {
    x.clamp(0.0, 1.0)
  }
}

// ────────────────────────────────────────────────────────────
// 单测：host 环境验证核心逻辑（与 wasm runtime 解耦）
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_first_json_object_handles_markdown_fence() {
    let s = "好的，结果是：```json\n{\"score\":0.8,\"label\":\"flag\",\"reason\":\"x\"}\n```";
    let inner = extract_first_json_object(s).expect("find");
    let v: RawVerdict = serde_json::from_str(&inner).expect("parse");
    assert_eq!(v.label, "flag");
    assert!((v.score - 0.8).abs() < 1e-5);
  }

  #[test]
  fn extract_first_json_object_handles_nested() {
    let s = r#"{"a":{"b":1}, "c":2}"#;
    let inner = extract_first_json_object(s).expect("find");
    // 应该取完整外层 object，而不是第一个 `}` 就截断
    assert_eq!(inner, s);
  }

  #[test]
  fn extract_first_json_object_returns_none_if_no_brace() {
    assert!(extract_first_json_object("plain text").is_none());
  }

  #[test]
  fn normalize_label_recognizes_all_three() {
    assert_eq!(normalize_label("block"), "block");
    assert_eq!(normalize_label("FLAG"), "flag");
    assert_eq!(normalize_label("  Allow "), "allow");
    assert_eq!(normalize_label("garbage"), "allow"); // 兜底
  }

  #[test]
  fn clamp01_handles_nan_and_overflow() {
    assert_eq!(clamp01(f32::NAN), 0.0);
    assert_eq!(clamp01(-3.0), 0.0);
    assert_eq!(clamp01(2.5), 1.0);
    assert!((clamp01(0.5) - 0.5).abs() < f32::EPSILON);
  }
}
