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

/// 获取翻译文本的 Hook
/// 返回一个 Signal<String> 以确保 UI 能够响应翻译加载完成
pub fn use_t(key: &str) -> Signal<String> {
    let lang = use_i18n();
    let key_str = key.to_string();
    
    let res = use_resource(move || {
        let key_str = key_str.clone();
        async move {
            let lang_str = if lang() == Language::En { "en" } else { "zh" };
            translate_server(key_str.clone(), lang_str.to_string()).await.unwrap_or(key_str)
        }
    });

    let mut output = use_signal(|| key.to_string());

    // 当资源加载完成时，更新输出信号
    if let Some(val) = res.read().as_ref() {
        if *val != *output.read() {
            output.set(val.clone());
        }
    }

    output
}

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
