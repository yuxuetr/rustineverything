//! Rust Web3 内容板块。页面与文章扫描由共享的 [`board`] crate 提供，
//! 这里只持有本板块的 [`BOARD`] 常量；文章在 `assets/topics/web3/`。

use board::{Board, BoardArticle, BoardIndex, FeaturedCrate};
use dioxus::prelude::*;
use sdk::AppModule;

pub const BOARD: Board = Board {
  id: "web3",
  route: "/web3",
  subtopics: &["evm", "solana", "substrate", "contracts", "wallet", "indexing"],
  featured_crates: &[
    FeaturedCrate { name: "alloy", url: "https://github.com/alloy-rs/alloy" },
    FeaturedCrate { name: "revm", url: "https://github.com/bluealloy/revm" },
    FeaturedCrate { name: "solana-sdk", url: "https://github.com/solana-labs/solana" },
    FeaturedCrate { name: "anchor", url: "https://www.anchor-lang.com" },
    FeaturedCrate { name: "polkadot-sdk", url: "https://github.com/paritytech/polkadot-sdk" },
    FeaturedCrate { name: "ink!", url: "https://use.ink" },
  ],
};

#[component]
pub fn Web3IndexPage() -> Element {
  rsx! { BoardIndex { board: BOARD } }
}

#[component]
pub fn Web3ArticlePage(slug: String) -> Element {
  rsx! { BoardArticle { board: BOARD, slug } }
}

pub struct Web3Module;

impl AppModule for Web3Module {
  fn name(&self) -> &'static str {
    "Web3"
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn board_config_is_valid() {
    assert_eq!(super::BOARD.problems(), Vec::<String>::new());
  }
}
