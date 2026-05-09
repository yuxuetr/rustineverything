pub mod podcast;
pub mod server;

use rustineverything_sdk::AppModule;

pub struct PodcastModule;

impl AppModule for PodcastModule {
    fn name(&self) -> &'static str {
        "Podcast"
    }
}

/// 把本模块导出的 MDX 嵌入组件注册到 widgets 全局注册表。
///
/// 调用点：`crates/app/src/main.rs` 启动期一次。该函数幂等。
pub fn register_components() {
    // PodcastCard：读入 `id`，调 server fn 拉取详情，同步原本逻辑。
    rustineverything_widgets::register(Box::new(PodcastCardComponent));
}

/// `<PodcastCard id="3" />` 在 MDX 中的渲染实现。
struct PodcastCardComponent;

impl rustineverything_widgets::MdxComponent for PodcastCardComponent {
    fn name(&self) -> &'static str {
        "PodcastCard"
    }

    fn render(
        &self,
        attrs: &std::collections::HashMap<String, String>,
    ) -> dioxus::prelude::Element {
        use dioxus::prelude::*;
        // 解析 id，解析失败走占位提示（文章页仍可渲染，不会 panic）。
        let id_opt = attrs.get("id").and_then(|s| s.parse::<i32>().ok());
        match id_opt {
            Some(id) => rsx! { podcast::PodcastCard { id } },
            None => rsx! {
                div { class: "not-prose my-8 p-4 rounded-lg border border-amber-200 bg-amber-50 text-sm text-amber-800",
                    "PodcastCard 需提供有效的 id。"
                }
            },
        }
    }
}
