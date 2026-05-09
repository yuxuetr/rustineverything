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
        let id = attrs.get("id").cloned().unwrap_or_default();
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
        let id = attrs.get("id").cloned().unwrap_or_default();
        rsx! {
            div { class: "not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800",
                iframe { class: "w-full h-full border-0", src: "//player.bilibili.com/player.html?bvid={id}&page=1&high_quality=1", allowfullscreen: true }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{clear_for_tests, list_registered, registered_count};

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
        // 至少 9 个：5 颜色 + 2 视频 + 2 装饰
        assert!(registered_count() >= 9);
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
    fn unknown_component_lookup_returns_none() {
        clear_for_tests();
        register_default_components();
        // 任意未注册的名字
        assert!(crate::registry::render(
            "NonexistentTag",
            &HashMap::new()
        )
        .is_none());
    }
}
