//! 业务模块共享的类型：审核提交 [`ModerationSubmission`] / [`ImageRef`]，
//! 以及模块初始化约定 [`AppModule`]。
use serde::{Deserialize, Serialize};

/// 提交给审核的内容（评论 / 话题 / 标注等业务模块 → `module-moderation`）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModerationSubmission {
  /// 待审正文（评论 / 话题 body / 回复 / 标注 ...）
  pub content: String,
  /// 业务类型：`comment` / `topic` / `reply` / `annotation` / 自定义
  /// 审核提示词会带上它作为场景。
  #[serde(default)]
  pub kind: String,
  /// 业务侧引用路径（如 `blog/welcome` / `topic:42`），可选。
  #[serde(default)]
  pub ref_path: String,
  /// 评论 / 话题中夹带的图片，LLM 审核时随消息发给 vision 模型。
  /// 老 site / 老 hook 不传该字段时默认空，不影响行为。
  #[serde(default)]
  pub images: Vec<ImageRef>,
}

/// 评论 / 话题中夹带的图片引用。
///
/// `url` 字段语义：
/// - **绝对 https URL**（生产）：LLM 厂商可直接 fetch
/// - **`data:image/...;base64,...`**：内联，所有 vision 模型都支持
/// - **相对路径** (`/uploads/xxx.jpg`)：宿主负责在 hook 调用前补全成绝对 URL
///
/// `media_type` 是给 Anthropic 协议提示的（OpenAI 协议从 URL 自动推断）；
/// 留空时 Anthropic 路径会拒收 base64 图片，URL 模式不影响。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImageRef {
  pub url: String,
  #[serde(default)]
  pub media_type: String,
}

impl ImageRef {
  pub fn url(url: impl Into<String>) -> Self {
    Self { url: url.into(), media_type: String::new() }
  }
  pub fn with_media_type(mut self, mt: impl Into<String>) -> Self {
    self.media_type = mt.into();
    self
  }
}

impl ModerationSubmission {
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      content: content.into(),
      kind: String::new(),
      ref_path: String::new(),
      images: Vec::new(),
    }
  }
  pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
    self.kind = kind.into();
    self
  }
  pub fn with_ref_path(mut self, ref_path: impl Into<String>) -> Self {
    self.ref_path = ref_path.into();
    self
  }
  pub fn with_images(mut self, images: impl IntoIterator<Item = ImageRef>) -> Self {
    self.images = images.into_iter().collect();
    self
  }
  pub fn push_image(mut self, image: ImageRef) -> Self {
    self.images.push(image);
    self
  }
}

/// 内置业务模块 Trait
/// 用于规范 博客、播客、论坛等模块的初始化
pub trait AppModule {
  fn name(&self) -> &'static str;

  /// 模块初始化钩子（例如初始化数据库表）
  fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn moderation_submission_serde_round_trip() {
    let s =
      ModerationSubmission::new("骂人内容").with_kind("comment").with_ref_path("blog/welcome");
    let json = serde_json::to_string(&s).unwrap();
    let parsed: ModerationSubmission = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, s);
  }

  #[test]
  fn moderation_submission_defaults_kind_ref() {
    // 只给 content 时其余字段取默认值
    let json = r#"{"content":"x"}"#;
    let s: ModerationSubmission = serde_json::from_str(json).unwrap();
    assert_eq!(s.content, "x");
    assert_eq!(s.kind, "");
    assert_eq!(s.ref_path, "");
    assert!(s.images.is_empty());
  }

  #[test]
  fn moderation_submission_with_images_serde() {
    let s = ModerationSubmission::new("有图")
      .with_kind("comment")
      .push_image(ImageRef::url("https://example.com/a.jpg"))
      .push_image(ImageRef::url("data:image/png;base64,iVBORw0K").with_media_type("image/png"));
    let json = serde_json::to_string(&s).unwrap();
    let parsed: ModerationSubmission = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.images.len(), 2);
    assert_eq!(parsed.images[0].url, "https://example.com/a.jpg");
    assert_eq!(parsed.images[1].media_type, "image/png");
  }

  #[test]
  fn image_ref_default_media_type_empty() {
    let r = ImageRef::url("https://x");
    assert!(r.media_type.is_empty());
  }
}
