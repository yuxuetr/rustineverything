//! 内置默认 MDX 嵌入组件（Phase 2.2）。
//!
//! 这些组件不依赖任何业务模块，可在 widgets 内自治：
//! - 视频嵌入：`<YouTube id="..." />` / `<Bilibili id="..." />`
//! - 文字样式：`<Yellow|Green|Blue|Pink|Purple text="..." />`
//! - 文本装饰：`<Underline text="..." />` / `<Strikethrough text="..." />`
//!
//! 调用 [`register_default_components`] 注册到全局注册表（`crates/app/src/main.rs`
//! 启动期一次）。注册顺序内部已固定为字典序，便于诊断。

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::registry::{register, MdxComponent};

/// 一次性注册全部内置 MDX 组件。
///
/// 多次调用是幂等的：每次都对同名组件重新覆盖（[`register`] 已经
/// 用 `bool` 表达覆盖语义，本函数忽略其返回值）。
pub fn register_default_components() {
  register(Box::new(YouTubeComponent));
  register(Box::new(BilibiliComponent));
  register(Box::new(UnderlineComponent));
  register(Box::new(StrikethroughComponent));
  register(Box::new(AudioPlayerComponent));

  // 5 种 Mac Preview 风格高亮色。
  for (name, color) in COLOR_COMPONENTS {
    register(Box::new(ColorComponent { name, color }));
  }
}

// ────────────────────────────────────────────────────────────
// 视频嵌入：YouTube / Bilibili
// ────────────────────────────────────────────────────────────

/// `<YouTube id="abc123" />`
struct YouTubeComponent;
impl MdxComponent for YouTubeComponent {
  fn name(&self) -> &'static str {
    "YouTube"
  }
  fn render(&self, attrs: &HashMap<String, String>) -> Element {
    let Some(id) = embed_id(attrs) else {
      return rsx! {};
    };
    rsx! {
        div { class: "not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800",
            iframe { class: "w-full h-full", src: "https://www.youtube.com/embed/{id}", allowfullscreen: true }
        }
    }
  }
}

/// `<Bilibili id="BV1xx" />`
struct BilibiliComponent;
impl MdxComponent for BilibiliComponent {
  fn name(&self) -> &'static str {
    "Bilibili"
  }
  fn render(&self, attrs: &HashMap<String, String>) -> Element {
    let Some(id) = embed_id(attrs) else {
      return rsx! {};
    };
    rsx! {
        div { class: "not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800",
            iframe { class: "w-full h-full border-0", src: bilibili_embed_url(&id), allowfullscreen: true }
        }
    }
  }
}

/// 视频编号（YouTube id / Bilibili BV 号）只由字母、数字、`_`、`-` 组成。
/// id 来自用户内容并直接拼进 iframe 地址（H7），其它字符一律不渲染，
/// 免得借 `?` / `&` / `/` 改写嵌入页的路径和参数。
fn embed_id(attrs: &HashMap<String, String>) -> Option<String> {
  attrs
    .get("id")
    .filter(|id| {
      !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
    .cloned()
}

/// 写死 https：协议相对地址在 http 页面上会变成 http，被 CSP 的
/// `frame-src https://player.bilibili.com` 拦掉。
fn bilibili_embed_url(id: &str) -> String {
  format!("https://player.bilibili.com/player.html?bvid={id}&page=1&high_quality=1")
}

// ────────────────────────────────────────────────────────────
// 文字高亮：5 色 + 下划线 + 删除线
// ────────────────────────────────────────────────────────────

/// `<Yellow text="..." />` 等系列。
const COLOR_COMPONENTS: &[(&str, &str)] = &[
  ("Yellow", "#EAB308"), // yellow-500
  ("Green", "#22C55E"),  // green-500
  ("Blue", "#3B82F6"),   // blue-500
  ("Pink", "#EC4899"),   // pink-500
  ("Purple", "#A855F7"), // purple-500
];

struct ColorComponent {
  name: &'static str,
  color: &'static str,
}
impl MdxComponent for ColorComponent {
  fn name(&self) -> &'static str {
    self.name
  }
  fn render(&self, attrs: &HashMap<String, String>) -> Element {
    let text = attrs.get("text").cloned().unwrap_or_default();
    let style = format!("color: {}; font-weight: 600", self.color);
    rsx! {
        span { style: "{style}", "{text}" }
    }
  }
}

/// `<Underline text="..." />`
struct UnderlineComponent;
impl MdxComponent for UnderlineComponent {
  fn name(&self) -> &'static str {
    "Underline"
  }
  fn render(&self, attrs: &HashMap<String, String>) -> Element {
    let text = attrs.get("text").cloned().unwrap_or_default();
    rsx! {
        span { style: "text-decoration: underline; text-decoration-thickness: 2px; text-underline-offset: 3px", "{text}" }
    }
  }
}

/// `<Strikethrough text="..." />`
struct StrikethroughComponent;
impl MdxComponent for StrikethroughComponent {
  fn name(&self) -> &'static str {
    "Strikethrough"
  }
  fn render(&self, attrs: &HashMap<String, String>) -> Element {
    let text = attrs.get("text").cloned().unwrap_or_default();
    rsx! {
        span { style: "text-decoration: line-through; text-decoration-thickness: 2px", "{text}" }
    }
  }
}

/// `<AudioPlayer src="https://….m4a" title="..." artist="..." />`（旧博客的音频组件，M3）。
/// 只放行 https 地址：CSP `media-src` 允许 https，其余协议不渲染。
struct AudioPlayerComponent;
impl MdxComponent for AudioPlayerComponent {
  fn name(&self) -> &'static str {
    "AudioPlayer"
  }
  fn render(&self, attrs: &HashMap<String, String>) -> Element {
    let Some(src) = attrs.get("src").filter(|s| s.starts_with("https://")).cloned() else {
      return rsx! {};
    };
    let title = attrs.get("title").cloned().unwrap_or_default();
    let artist = attrs.get("artist").cloned().unwrap_or_default();
    rsx! {
        figure { class: "not-prose my-8 rounded-2xl border border-slate-200 dark:border-slate-800 p-4",
            if !title.is_empty() {
                figcaption { class: "text-sm font-semibold text-slate-800 dark:text-slate-200", "{title}" }
            }
            if !artist.is_empty() {
                p { class: "text-xs text-slate-500 dark:text-slate-400", "{artist}" }
            }
            audio { class: "w-full mt-3", controls: true, preload: "none", src: "{src}" }
        }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::registry::{clear_for_tests, list_registered, registered_count};

  /// H7：id 来自用户内容，只能是视频编号，不能带出路径或查询参数。
  #[test]
  fn embed_id_rejects_url_syntax() {
    let id = |v: &str| embed_id(&HashMap::from([("id".to_string(), v.to_string())]));
    assert_eq!(id("dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
    assert_eq!(id("BV1xx411c7mD").as_deref(), Some("BV1xx411c7mD"));
    assert_eq!(id("a_b-c").as_deref(), Some("a_b-c"));
    for bad in ["", "x?autoplay=1", "x&page=2", "../watch", "x y", "x#t", "%3F"] {
      assert_eq!(id(bad), None, "{bad}");
    }
    assert_eq!(embed_id(&HashMap::new()), None);
  }

  /// 注：本 mod 操作全局单例 registry，所有测试共享同一进程状态。
  /// 同一测试进程内单跑 / 顺序跑均可。Cargo 默认开 -j 并行，所以
  /// 这里我们用一个粗粒度策略：每个测试开头先 `clear_for_tests()`，
  /// 然后注册并断言。配合 `--test-threads=1`（项目脚本约定）即可。
  #[test]
  fn default_components_register_all_expected_names() {
    clear_for_tests();
    register_default_components();
    let names = list_registered();
    for expected in &[
      "AudioPlayer",
      "Bilibili",
      "Blue",
      "Green",
      "Pink",
      "Purple",
      "Strikethrough",
      "Underline",
      "Yellow",
      "YouTube",
    ] {
      assert!(
        names.iter().any(|n| n == expected),
        "default registry missing {}: actual = {:?}",
        expected,
        names
      );
    }
    // 至少 10 个：5 颜色 + 2 视频 + 2 装饰 + 音频
    assert!(registered_count() >= 10);
  }

  #[test]
  fn register_default_components_is_idempotent() {
    clear_for_tests();
    register_default_components();
    let first = registered_count();
    register_default_components();
    // 重复注册 → 覆盖同名条目，组件总数应保持不变
    assert_eq!(registered_count(), first);
  }

  #[test]
  fn bilibili_embed_url_is_https() {
    assert_eq!(
      bilibili_embed_url("BV1xx411c7mD"),
      "https://player.bilibili.com/player.html?bvid=BV1xx411c7mD&page=1&high_quality=1"
    );
  }

  #[test]
  fn unknown_component_lookup_returns_none() {
    clear_for_tests();
    register_default_components();
    // 任意未注册的名字
    assert!(crate::registry::render("NonexistentTag", &HashMap::new()).is_none());
  }
}
