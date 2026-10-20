//! Rust Wasm 内容板块。页面与文章扫描由共享的 [`board`] crate 提供，
//! 这里只持有本板块的 [`BOARD`] 常量；文章在 `assets/topics/wasm/`。

use board::{Board, BoardArticle, BoardIndex, FeaturedCrate};
use dioxus::prelude::*;
use sdk::AppModule;

pub const BOARD: Board = Board {
  id: "wasm",
  route: "/wasm",
  subtopics: &["bindgen", "wasi", "components", "runtimes", "frontend", "plugins"],
  featured_crates: &[
    FeaturedCrate { name: "wasm-bindgen", url: "https://github.com/rustwasm/wasm-bindgen" },
    FeaturedCrate { name: "wasmtime", url: "https://wasmtime.dev" },
    FeaturedCrate { name: "wasmer", url: "https://wasmer.io" },
    FeaturedCrate { name: "wit-bindgen", url: "https://github.com/bytecodealliance/wit-bindgen" },
    FeaturedCrate { name: "leptos", url: "https://leptos.dev" },
    FeaturedCrate { name: "trunk", url: "https://trunkrs.dev" },
  ],
};

#[component]
pub fn WasmIndexPage() -> Element {
  rsx! { BoardIndex { board: BOARD } }
}

#[component]
pub fn WasmArticlePage(slug: String) -> Element {
  rsx! { BoardArticle { board: BOARD, slug } }
}

pub struct WasmModule;

impl AppModule for WasmModule {
  fn name(&self) -> &'static str {
    "WASM"
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn board_config_is_valid() {
    assert_eq!(super::BOARD.problems(), Vec::<String>::new());
  }
}
