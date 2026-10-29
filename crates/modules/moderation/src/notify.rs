//! 有内容进审核队列时给管理员发邮件。
//!
//! 配置全在环境变量里，缺任何一项就只记 warn、不发：
//! - `MODERATION_NOTIFY_EMAIL`：收件人
//! - `SMTP_HOST` / `SMTP_USERNAME` / `SMTP_PASSWORD`
//! - `SMTP_PORT`：默认 465（隐式 TLS）；其他端口走 STARTTLS
//! - `SMTP_FROM`：发件人，默认同 `SMTP_USERNAME`
//!
//! 发信在后台任务里做，失败只记日志：通知丢了不影响提交，内容仍在队列里。

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// 邮件里正文快照最多带多少字符。
const CONTENT_PREVIEW_CHARS: usize = 2000;

struct SmtpConfig {
  host: String,
  port: u16,
  username: String,
  password: String,
  from: String,
  to: String,
}

impl SmtpConfig {
  fn from_env() -> Result<Self, String> {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let need = |k: &str| var(k).ok_or_else(|| format!("{k} 未设置"));
    let username = need("SMTP_USERNAME")?;
    let port = match var("SMTP_PORT") {
      Some(p) => p.parse().map_err(|_| format!("SMTP_PORT 不是端口号：{p}"))?,
      None => 465,
    };
    Ok(Self {
      host: need("SMTP_HOST")?,
      port,
      password: need("SMTP_PASSWORD")?,
      from: var("SMTP_FROM").unwrap_or_else(|| username.clone()),
      to: need("MODERATION_NOTIFY_EMAIL")?,
      username,
    })
  }
}

/// 一条待审内容的摘要，邮件只用这些字段。
pub struct PendingItem {
  pub kind: String,
  pub ref_path: String,
  pub reason: String,
  pub score: f32,
  pub content: String,
  pub image_count: usize,
}

fn render(item: &PendingItem, base_url: &str) -> (String, String) {
  let subject = format!("[待审核] {} {}", item.kind, item.ref_path);
  let preview: String = item.content.chars().take(CONTENT_PREVIEW_CHARS).collect();
  let truncated = if preview.len() < item.content.len() { "\n…（已截断）" } else { "" };
  let body = format!(
    "有一条内容等待人工审核，通过之前不会公开显示。\n\n\
     类型：{}\n位置：{}\n原因：{}\n分数：{:.2}\n图片：{} 张\n\n\
     审核队列：{}/admin/moderation\n\n---- 内容 ----\n{}{}\n",
    item.kind,
    item.ref_path,
    item.reason,
    item.score,
    item.image_count,
    base_url.trim_end_matches('/'),
    preview,
    truncated,
  );
  (subject, body)
}

async fn send(item: PendingItem) -> Result<(), String> {
  let cfg = SmtpConfig::from_env()?;
  let base_url = std::env::var("BASE_URL").unwrap_or_default();
  let (subject, body) = render(&item, &base_url);
  let message = Message::builder()
    .from(cfg.from.parse().map_err(|e| format!("SMTP_FROM: {e}"))?)
    .to(cfg.to.parse().map_err(|e| format!("MODERATION_NOTIFY_EMAIL: {e}"))?)
    .subject(subject)
    .header(ContentType::TEXT_PLAIN)
    .body(body)
    .map_err(|e| e.to_string())?;
  let builder = if cfg.port == 465 {
    AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
  } else {
    AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
  }
  .map_err(|e| e.to_string())?;
  let mailer =
    builder.port(cfg.port).credentials(Credentials::new(cfg.username, cfg.password)).build();
  mailer.send(message).await.map_err(|e| e.to_string())?;
  Ok(())
}

/// 后台发通知邮件；不等结果。
pub fn notify_admin(item: PendingItem) {
  let kind = item.kind.clone();
  let ref_path = item.ref_path.clone();
  tokio::spawn(async move {
    match send(item).await {
      Ok(()) => tracing::info!(kind = %kind, ref_path = %ref_path, "moderation: admin notified"),
      Err(e) => tracing::warn!(
        error = %e,
        kind = %kind,
        ref_path = %ref_path,
        "moderation: admin notification not sent"
      ),
    }
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item(content: &str) -> PendingItem {
    PendingItem {
      kind: "comment".into(),
      ref_path: "blog:welcome".into(),
      reason: "LLM 审核失败，待人工复核".into(),
      score: 0.0,
      content: content.into(),
      image_count: 1,
    }
  }

  #[test]
  fn render_links_queue_and_includes_content() {
    let (subject, body) = render(&item("你好"), "https://my.site/");
    assert_eq!(subject, "[待审核] comment blog:welcome");
    assert!(body.contains("https://my.site/admin/moderation"));
    assert!(body.contains("LLM 审核失败"));
    assert!(body.ends_with("你好\n"));
  }

  #[test]
  fn render_truncates_long_content_by_chars() {
    let long = "审".repeat(CONTENT_PREVIEW_CHARS + 10);
    let (_, body) = render(&item(&long), "");
    assert!(body.contains("（已截断）"));
    let shown = body.split("---- 内容 ----\n").nth(1).unwrap_or_default();
    assert_eq!(shown.matches('审').count(), CONTENT_PREVIEW_CHARS);
  }
}
