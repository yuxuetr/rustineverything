use dioxus::prelude::*;
use crate::server::translate_server;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Language {
  En,
  Zh,
}

pub fn use_i18n() -> Signal<Language> {
  use_context::<Signal<Language>>()
}

pub fn init_i18n() {
  use_context_provider(|| Signal::new(Language::Zh));
}

/// A hook that provides a reactive translation for a key
pub fn use_t(key: &str) -> ReadOnlySignal<String> {
    let lang = use_i18n();
    let key_str = key.to_string();

    let res = use_resource(move || {
        let key_str = key_str.clone();
        async move {
            let lang_str = if lang() == Language::En { "en" } else { "zh" };
            translate_server(key_str.clone(), lang_str.to_string()).await.unwrap_or(key_str)
        }
    });

    let val = match &*res.read() {
        Some(val) => val.clone(),
        None => key.to_string(),
    };

    Signal::new(val).into()
}


// Keep legacy t for fallback or non-reactive uses
pub fn t(lang: Language, key: &str) -> String {
  match (lang, key) {
    (Language::Zh, "nav.blog") => "博客".to_string(),
    (Language::En, "nav.blog") => "Blog".to_string(),
    (Language::Zh, "nav.podcast") => "播客".to_string(),
    (Language::En, "nav.podcast") => "Podcast".to_string(),
    (Language::Zh, "nav.start") => "开始学习".to_string(),
    (Language::En, "nav.start") => "Get Started".to_string(),
    (_, k) => k.to_string(),
  }
}
