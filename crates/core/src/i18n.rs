use serde::{Deserialize, Serialize};

/// Shared language enum used across crates (app, forum modules, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
  En,
  Zh,
}

impl Default for Language {
  fn default() -> Self {
    Language::Zh
  }
}
