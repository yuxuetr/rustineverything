//! [`LlmModerationStage`]：让 LLM 判断一条提交是否合规。
//!
//! 1. [`build_messages`]：系统提示词 + 用户消息（场景前缀、正文、链接提示、图片）
//! 2. `LlmClient::chat` 取模型回复（端点 / 协议由 `crates/llm` 按 env 选择）
//! 3. [`parse_verdict`]：从回复里读出 `{score, label, reason}`
//!
//! LLM 调用失败或回复无法解析时返回 [`Verdict::allow`]（fail-open），并记 warning。

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use app_core::engines::moderation::{ModerationLabel, Verdict};
use llm::{LlmClient, LlmContentBlock, LlmMessage, LlmRole};
use sdk::ModerationSubmission;

use crate::stage::AsyncModerationStage;
use crate::url_blocklist::extract_urls;

/// 系统提示词：让模型严格输出 JSON。多模态维度只在 user message 含图时由模型考虑。
const SYSTEM_PROMPT: &str = r#"你是一个评论审核员，负责判断用户提交的内容是否合规。
判断维度（任意命中即视为有问题）：
1. 谩骂、人身攻击、歧视、骚扰
2. 色情、低俗、暴力血腥描写（**含图片**：色情、血腥、政治符号、令人不适的视觉元素）
3. 政治敏感、违法犯罪、煽动性内容
4. 垃圾广告、营销链接、无意义刷屏
5. 文本与图片不匹配的诱导（标题党 / 钓鱼）
6. **链接风险**：当用户消息中带 `[包含链接: ...]` 标记时，仔细审查这些域名：
   - 域名拼写仿冒知名品牌（如 paypa1.com / amaz0n.shop）→ block
   - 短链 / 跳转中介（bit.ly, t.cn, tinyurl 等）+ 诱导话术 → flag 或 block
   - 评论上下文与链接目的明显不符的诱导（"点这里领奖" 之类）→ block

**只输出一行 JSON**，不要 markdown 围栏、不要解释。字段：
{"score": 0.0-1.0, "label": "allow"|"flag"|"block", "reason": "≤30 字理由"}

评分约定：
- 0.0 ~ 0.49 → label="allow"（正常内容）
- 0.5 ~ 0.89 → label="flag"（可疑，需要复核）
- 0.9 ~ 1.0  → label="block"（明显违规）"#;

pub struct LlmModerationStage {
  llm: Arc<dyn LlmClient>,
}

impl LlmModerationStage {
  pub fn new(llm: Arc<dyn LlmClient>) -> Self {
    Self { llm }
  }
}

/// 构造发给模型的两条消息：系统提示词 + 待审内容。
///
/// 正文里的链接以 `[包含链接: ...]` 附在末尾，让模型判断仿冒域名 / 短链诱导
/// （命中黑名单的链接已由 [`crate::UrlBlocklistStage`] 先行拦下）。
pub fn build_messages(sub: &ModerationSubmission) -> Vec<LlmMessage> {
  let urls = extract_urls(&sub.content);
  let url_hint = if urls.is_empty() {
    String::new()
  } else {
    format!("\n\n[包含链接: {}]", urls.join(", "))
  };

  // kind / ref_path 作为场景上下文，方便模型按场景调整严格度
  let scene_prefix = if sub.kind.is_empty() && sub.ref_path.is_empty() {
    String::new()
  } else {
    let kind = if sub.kind.is_empty() { "comment" } else { &sub.kind };
    format!("[场景: {} {}]\n\n", kind, sub.ref_path)
  };

  let mut content = vec![LlmContentBlock::text(format!("{scene_prefix}{}{url_hint}", sub.content))];
  content.extend(
    sub
      .images
      .iter()
      .filter(|img| !img.url.is_empty())
      .map(|img| LlmContentBlock::image_url(&img.url)),
  );

  vec![LlmMessage::system(SYSTEM_PROMPT), LlmMessage { role: LlmRole::User, content }]
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

/// 从模型回复里读出结论。先整体按 JSON 解析，失败再取第一个 `{...}`
/// （模型偶尔会包 markdown 围栏或加说明）。读不出返回 `None`。
///
/// 未知 label 视为 allow；score 夹到 0.0–1.0，NaN 记为 0。
pub fn parse_verdict(text: &str) -> Option<Verdict> {
  let text = text.trim();
  let raw: RawVerdict = serde_json::from_str(text).ok().or_else(|| {
    extract_first_json_object(text).and_then(|inner| serde_json::from_str(inner).ok())
  })?;
  let label = match raw.label.trim().to_ascii_lowercase().as_str() {
    "block" => ModerationLabel::Block,
    "flag" => ModerationLabel::Flag,
    _ => ModerationLabel::Allow,
  };
  let score = if raw.score.is_nan() { 0.0 } else { raw.score.clamp(0.0, 1.0) };
  Some(Verdict { score, label, reason: raw.reason.unwrap_or_default() })
}

/// 取文本中第一个括号配平的 `{...}`。
fn extract_first_json_object(text: &str) -> Option<&str> {
  let start = text.find('{')?;
  let mut depth = 0i32;
  for (i, ch) in text[start..].char_indices() {
    match ch {
      '{' => depth += 1,
      '}' => {
        depth -= 1;
        if depth == 0 {
          return Some(&text[start..start + i + 1]);
        }
      }
      _ => {}
    }
  }
  None
}

#[async_trait]
impl AsyncModerationStage for LlmModerationStage {
  fn name(&self) -> &str {
    "llm"
  }

  async fn evaluate(&self, submission: &ModerationSubmission) -> Verdict {
    let reply = match self.llm.chat(build_messages(submission)).await {
      Ok(t) => t,
      Err(e) => {
        tracing::warn!(error = %e, "moderation: LLM call failed → allow");
        return Verdict::allow();
      }
    };
    match parse_verdict(&reply) {
      Some(v) => {
        tracing::debug!(score = v.score, label = ?v.label, "moderation: llm verdict");
        v
      }
      None => {
        tracing::warn!(reply = %reply, "moderation: LLM reply has no verdict JSON → allow");
        Verdict::allow()
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use app_core::error::{AppError, AppResult};
  use llm::LlmProvider;
  use sdk::ImageRef;

  struct StubLlm(Option<&'static str>);

  #[async_trait]
  impl LlmClient for StubLlm {
    fn provider(&self) -> LlmProvider {
      LlmProvider::OpenAi
    }
    async fn chat(&self, _messages: Vec<LlmMessage>) -> AppResult<String> {
      self.0.map(str::to_string).ok_or_else(|| AppError::other("stub LLM forced failure"))
    }
  }

  async fn evaluate_with(reply: Option<&'static str>) -> Verdict {
    LlmModerationStage::new(Arc::new(StubLlm(reply)))
      .evaluate(&ModerationSubmission::new("anything"))
      .await
  }

  #[test]
  fn messages_carry_scene_links_and_images() {
    let sub = ModerationSubmission::new("看 https://bit.ly/x 和 http://a.com!")
      .with_kind("topic")
      .with_ref_path("forum/1")
      .with_images([ImageRef::url("https://img.example/1.png"), ImageRef::url("")]);
    let msgs = build_messages(&sub);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, LlmRole::System);
    assert_eq!(msgs[0].text_only(), SYSTEM_PROMPT);
    assert_eq!(msgs[1].role, LlmRole::User);
    assert_eq!(
      msgs[1].text_only(),
      "[场景: topic forum/1]\n\n看 https://bit.ly/x 和 http://a.com!\n\n[包含链接: https://bit.ly/x, http://a.com]"
    );
    // 空 URL 的图片被丢掉
    assert_eq!(msgs[1].content[1..], [LlmContentBlock::image_url("https://img.example/1.png")]);
  }

  #[test]
  fn plain_text_has_no_scene_or_link_hint() {
    let msgs = build_messages(&ModerationSubmission::new("感谢分享"));
    assert_eq!(msgs[1].content, vec![LlmContentBlock::text("感谢分享")]);
  }

  #[test]
  fn scene_defaults_kind_to_comment() {
    let msgs = build_messages(&ModerationSubmission::new("x").with_ref_path("blog/a"));
    assert_eq!(msgs[1].text_only(), "[场景: comment blog/a]\n\nx");
  }

  #[test]
  fn parses_plain_and_fenced_json() {
    let v = parse_verdict(r#"{"score":0.95,"label":"block","reason":"辱骂"}"#).expect("plain");
    assert_eq!((v.label, v.reason.as_str()), (ModerationLabel::Block, "辱骂"));
    let v =
      parse_verdict("好的：```json\n{\"score\":0.8,\"label\":\"FLAG\"}\n```").expect("fenced");
    assert_eq!(v.label, ModerationLabel::Flag);
    assert!((v.score - 0.8).abs() < 1e-5);
    assert_eq!(v.reason, "");
  }

  #[test]
  fn nested_object_is_taken_whole() {
    let s = r#"{"a":{"b":1}, "c":2}"#;
    assert_eq!(extract_first_json_object(s), Some(s));
  }

  #[test]
  fn unknown_label_is_allow_and_score_is_clamped() {
    let v = parse_verdict(r#"{"score":2.5,"label":"garbage"}"#).expect("parse");
    assert_eq!(v.label, ModerationLabel::Allow);
    assert_eq!(v.score, 1.0);
    let v = parse_verdict(r#"{"score":-3,"label":" Block "}"#).expect("parse");
    assert_eq!((v.label, v.score), (ModerationLabel::Block, 0.0));
  }

  #[test]
  fn reply_without_json_is_none() {
    assert!(parse_verdict("plain text").is_none());
    assert!(parse_verdict("{ unclosed").is_none());
    assert!(parse_verdict("").is_none());
  }

  #[tokio::test]
  async fn stage_returns_the_models_verdict() {
    let v = evaluate_with(Some(r#"{"score":0.95,"label":"block","reason":"spam"}"#)).await;
    assert_eq!(v.label, ModerationLabel::Block);
    assert_eq!(v.reason, "spam");
  }

  #[tokio::test]
  async fn llm_failure_and_unparsable_reply_fail_open() {
    assert_eq!(evaluate_with(None).await, Verdict::allow());
    assert_eq!(evaluate_with(Some("我觉得还行")).await, Verdict::allow());
  }
}
