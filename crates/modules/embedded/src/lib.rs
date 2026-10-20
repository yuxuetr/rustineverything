//! 嵌入式 Rust 内容板块。页面与文章扫描由共享的 [`board`] crate 提供，
//! 这里只持有本板块的 [`BOARD`] 常量；文章在 `assets/topics/embedded/`。

use board::{Board, BoardArticle, BoardIndex, FeaturedCrate};
use dioxus::prelude::*;
use sdk::AppModule;

pub const BOARD: Board = Board {
  id: "embedded",
  route: "/embedded",
  subtopics: &["no-std", "embassy", "rtic", "hal", "defmt", "platforms"],
  featured_crates: &[
    FeaturedCrate { name: "embassy", url: "https://embassy.dev" },
    FeaturedCrate { name: "rtic", url: "https://rtic.rs" },
    FeaturedCrate { name: "embedded-hal", url: "https://github.com/rust-embedded/embedded-hal" },
    FeaturedCrate { name: "defmt", url: "https://github.com/knurling-rs/defmt" },
    FeaturedCrate { name: "probe-rs", url: "https://probe.rs" },
    FeaturedCrate { name: "heapless", url: "https://github.com/rust-embedded/heapless" },
  ],
};

#[component]
pub fn EmbeddedIndexPage() -> Element {
  rsx! { BoardIndex { board: BOARD } }
}

#[component]
pub fn EmbeddedArticlePage(slug: String) -> Element {
  rsx! { BoardArticle { board: BOARD, slug } }
}

pub struct EmbeddedModule;

impl AppModule for EmbeddedModule {
  fn name(&self) -> &'static str {
    "Embedded"
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn board_config_is_valid() {
    assert_eq!(super::BOARD.problems(), Vec::<String>::new());
  }
}
