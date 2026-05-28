use serde::{Deserialize, Serialize};

/// Shared language enum used across crates (app, forum modules, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum Language {
  En,
  #[default]
  Zh,
}

