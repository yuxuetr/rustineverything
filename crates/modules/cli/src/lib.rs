//! Rust CLI 内容板块。页面与文章扫描由共享的 [`board`] crate 提供，
//! 这里只持有本板块的 [`BOARD`] 常量；文章在 `assets/topics/cli/`。

use board::{Board, BoardArticle, BoardIndex, FeaturedCrate};
use dioxus::prelude::*;
use sdk::AppModule;

pub const BOARD: Board = Board {
  id: "cli",
  route: "/cli",
  subtopics: &["args", "tui", "output", "config", "testing", "distribution"],
  featured_crates: &[
    FeaturedCrate { name: "clap", url: "https://github.com/clap-rs/clap" },
    FeaturedCrate { name: "ratatui", url: "https://ratatui.rs" },
    FeaturedCrate { name: "indicatif", url: "https://github.com/console-rs/indicatif" },
    FeaturedCrate { name: "console", url: "https://github.com/console-rs/console" },
    FeaturedCrate { name: "crossterm", url: "https://github.com/crossterm-rs/crossterm" },
    FeaturedCrate { name: "assert_cmd", url: "https://github.com/assert-rs/assert_cmd" },
  ],
};

#[component]
pub fn CliIndexPage() -> Element {
  rsx! { BoardIndex { board: BOARD } }
}

#[component]
pub fn CliArticlePage(slug: String) -> Element {
  rsx! { BoardArticle { board: BOARD, slug } }
}

pub struct CliModule;

impl AppModule for CliModule {
  fn name(&self) -> &'static str {
    "CLI"
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn board_config_is_valid() {
    assert_eq!(super::BOARD.problems(), Vec::<String>::new());
  }
}
