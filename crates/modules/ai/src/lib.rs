//! Rust AI 内容板块。页面与文章扫描由共享的 [`board`] crate 提供，
//! 这里只持有本板块的 [`BOARD`] 常量；文章在 `assets/topics/ai/`。

use board::{Board, BoardArticle, BoardIndex, FeaturedCrate};
use dioxus::prelude::*;
use sdk::AppModule;

pub const BOARD: Board = Board {
  id: "ai",
  route: "/ai",
  subtopics: &["tensors", "inference", "llm", "tokenizers", "training", "embeddings"],
  featured_crates: &[
    FeaturedCrate { name: "candle", url: "https://github.com/huggingface/candle" },
    FeaturedCrate { name: "burn", url: "https://burn.dev" },
    FeaturedCrate { name: "tch", url: "https://github.com/LaurentMazare/tch-rs" },
    FeaturedCrate { name: "tokenizers", url: "https://github.com/huggingface/tokenizers" },
    FeaturedCrate { name: "ort", url: "https://ort.pyke.io" },
    FeaturedCrate { name: "safetensors", url: "https://github.com/huggingface/safetensors" },
  ],
};

#[component]
pub fn AiIndexPage() -> Element {
  rsx! { BoardIndex { board: BOARD } }
}

#[component]
pub fn AiArticlePage(slug: String) -> Element {
  rsx! { BoardArticle { board: BOARD, slug } }
}

pub struct AiModule;

impl AppModule for AiModule {
  fn name(&self) -> &'static str {
    "AI"
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn board_config_is_valid() {
    assert_eq!(super::BOARD.problems(), Vec::<String>::new());
  }
}
