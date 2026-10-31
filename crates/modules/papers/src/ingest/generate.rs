//! 调模型：全文 → 英文解读，英文解读 → 其他语言。不合格时把问题告诉模型重写一次。

use llm::{LlmClient, LlmMessage, LlmUsage};

use super::check::{check_digest, check_translation, Checked};
use crate::digest::PaperDigest;

/// 提示词版本，记进 `paper_summaries.prompt_version`；改提示词时加一。
pub const PROMPT_VERSION: i32 = 1;

const SCHEMA: &str = r#"{
  "tldr": "one sentence",
  "problem": "2-4 sentences",
  "method": "3-6 sentences",
  "results": [{"claim": "...", "numbers": ["exact number strings copied from the paper"], "section": "4.2"}],
  "limitations": ["..."],
  "key_points": [{"text": "...", "section": "3.1"}],
  "mindmap": {"label": "root", "children": [{"label": "...", "children": []}]}
}"#;

fn extract_system() -> String {
  format!(
    "You write faithful technical digests of research papers for engineers.
Read the paper text and return ONLY a JSON object with this shape:
{SCHEMA}
Rules:
- Use only facts stated in the paper. Do not add outside knowledge or praise.
- \"section\" must be a section label exactly as it appears in a [§...] marker of the text (e.g. \"3\", \"4.2\", \"A.1\", \"Limitations\"), the section where the claim is stated.
- \"numbers\" are copied character-for-character from the paper text (e.g. \"73.4\", \"2.1×\", \"15%\"). Use [] if none.
- 3-6 results, 5-8 key_points, 2-4 limitations (only limitations the paper itself states or that its experiments clearly leave open; say which).
- mindmap: at most 3 levels below the root, at most 6 children per node, labels under 60 characters, no quotes or brackets."
  )
}

const TRANSLATE_SYSTEM: &str = "Translate the string values of this JSON object into Simplified Chinese for engineers.
Keep the exact same JSON structure, keys and array lengths. Do not translate or change \"section\" values or
\"numbers\" entries. Keep model names, dataset names, metric names and code identifiers in English.
Mindmap labels stay under 30 Chinese characters. Return ONLY the JSON object.";

/// 模型有时会包一层 ```json 代码块或在前后加话：取第一个 `{` 到最后一个 `}`。
pub fn parse_digest(reply: &str) -> Result<PaperDigest, String> {
  let start = reply.find('{').ok_or("reply has no JSON object")?;
  let end = reply.rfind('}').ok_or("reply has no JSON object")?;
  if end < start {
    return Err("reply has no JSON object".into());
  }
  serde_json::from_str(&reply[start..=end]).map_err(|e| format!("invalid JSON: {e}"))
}

fn add(total: &mut LlmUsage, u: LlmUsage) {
  total.input_tokens += u.input_tokens;
  total.output_tokens += u.output_tokens;
}

/// 先问一次，`validate` 不过就带上问题再问一次。两次都不过返回最后的问题。
async fn ask_validated<T>(
  llm: &dyn LlmClient,
  stage: &'static str,
  system: &str,
  user: String,
  usage: &mut LlmUsage,
  validate: impl Fn(&PaperDigest) -> Result<T, Vec<String>>,
) -> Result<(PaperDigest, T), String> {
  let mut messages = vec![LlmMessage::system(system), LlmMessage::user(user)];
  let mut last_problems = String::new();
  for attempt in 0..2 {
    let input_chars: usize = messages.iter().map(|m| m.text_only().chars().count()).sum();
    let started = std::time::Instant::now();
    let reply = llm.complete(messages.clone()).await.map_err(|e| format!("模型调用失败：{e}"))?;
    // 单次调用的长度：自托管负载估算要用。output_tokens 远大于 reply_chars 时，多出来的是推理 token。
    tracing::info!(
      stage,
      attempt,
      input_chars,
      reply_chars = reply.text.chars().count(),
      input_tokens = reply.usage.input_tokens,
      output_tokens = reply.usage.output_tokens,
      elapsed_ms = started.elapsed().as_millis() as u64,
      "papers: llm call"
    );
    add(usage, reply.usage);
    let problems = match parse_digest(&reply.text) {
      Ok(d) => match validate(&d) {
        Ok(extra) => return Ok((d, extra)),
        Err(p) => p,
      },
      Err(e) => vec![e],
    };
    last_problems = problems.join("; ");
    tracing::info!(attempt, problems = %last_problems, "papers: reply rejected");
    messages.push(LlmMessage::assistant(reply.text));
    messages.push(LlmMessage::user(format!(
      "Your JSON has these problems: {last_problems}. Return the corrected JSON object only."
    )));
  }
  Err(format!("两次都没通过校验：{last_problems}"))
}

/// 英文解读：校验节号与导图，数字找不到的记进 `Checked`。
pub async fn digest_en(
  llm: &dyn LlmClient,
  title: &str,
  text: &str,
  sections: &[String],
  usage: &mut LlmUsage,
) -> Result<(PaperDigest, Checked), String> {
  ask_validated(
    llm,
    "extract",
    &extract_system(),
    format!("Title: {title}\n\n{text}"),
    usage,
    |d| check_digest(d, sections, text),
  )
  .await
}

/// 中文版：只翻译文字，结构、节号、数字不变。
pub async fn translate_zh(
  llm: &dyn LlmClient,
  en: &PaperDigest,
  usage: &mut LlmUsage,
) -> Result<PaperDigest, String> {
  let json = serde_json::to_string(en).map_err(|e| e.to_string())?;
  let (zh, ()) =
    ask_validated(llm, "translate", TRANSLATE_SYSTEM, json, usage, |d| check_translation(en, d))
      .await?;
  Ok(zh)
}

#[cfg(test)]
mod tests {
  use super::*;
  use app_core::error::AppResult;
  use llm::{LlmProvider, LlmReply};
  use std::sync::Mutex;

  /// 依次返回预设的回复。
  struct Scripted(Mutex<Vec<&'static str>>);

  #[async_trait::async_trait]
  impl LlmClient for Scripted {
    fn provider(&self) -> LlmProvider {
      LlmProvider::OpenAi
    }
    async fn complete(&self, _: Vec<LlmMessage>) -> AppResult<LlmReply> {
      let text = self.0.lock().map(|mut v| v.remove(0)).unwrap_or_default();
      Ok(LlmReply { text: text.into(), usage: LlmUsage { input_tokens: 10, output_tokens: 5 } })
    }
  }

  const GOOD: &str = r#"Here you go:
```json
{"tldr": "t", "problem": "p", "method": "m",
 "results": [{"claim": "c", "numbers": ["12.5"], "section": "3"}],
 "limitations": ["l"], "key_points": [{"text": "k", "section": "§3"}],
 "mindmap": {"label": "r", "children": [{"label": "a"}]}}
```"#;
  const BAD_SECTION: &str = r#"{"tldr": "t", "problem": "p", "method": "m",
 "results": [{"claim": "c", "numbers": [], "section": "9.9"}],
 "limitations": [], "key_points": [{"text": "k", "section": "3"}],
 "mindmap": {"label": "r"}}"#;

  #[tokio::test]
  async fn retries_once_then_accepts() {
    let llm = Scripted(Mutex::new(vec![BAD_SECTION, GOOD]));
    let mut usage = LlmUsage::default();
    let (d, checked) =
      digest_en(&llm, "T", "[§3] Method\nscore 12.5", &["3".into()], &mut usage).await.unwrap();
    assert_eq!(d.results[0].numbers, ["12.5"]);
    assert!(checked.unverified_numbers.is_empty());
    assert_eq!(usage, LlmUsage { input_tokens: 20, output_tokens: 10 });
  }

  #[tokio::test]
  async fn gives_up_after_two_bad_replies() {
    let llm = Scripted(Mutex::new(vec![BAD_SECTION, "not json"]));
    let mut usage = LlmUsage::default();
    let err = digest_en(&llm, "T", "x", &["3".into()], &mut usage).await.unwrap_err();
    assert!(err.contains("invalid JSON") || err.contains("no JSON"), "{err}");
  }
}
