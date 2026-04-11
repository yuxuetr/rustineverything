use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
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

pub fn t(lang: Language, key: &str) -> String {
  match (lang, key) {
    (Language::Zh, "nav.blog") => "博客".to_string(),
    (Language::En, "nav.blog") => "Blog".to_string(),
    (Language::Zh, "nav.podcast") => "播客".to_string(),
    (Language::En, "nav.podcast") => "Podcast".to_string(),
    (Language::Zh, "nav.courses") => "课程".to_string(),
    (Language::En, "nav.courses") => "Courses".to_string(),
    (Language::Zh, "nav.docs") => "文档".to_string(),
    (Language::En, "nav.docs") => "Docs".to_string(),
    (Language::Zh, "nav.cases") => "案例".to_string(),
    (Language::En, "nav.cases") => "Showcase".to_string(),
    (Language::Zh, "nav.ai") => "AI".to_string(),
    (Language::En, "nav.ai") => "AI".to_string(),
    (Language::Zh, "nav.web3") => "Web3".to_string(),
    (Language::En, "nav.web3") => "Web3".to_string(),
    (Language::Zh, "nav.start") => "开始学习".to_string(),
    (Language::En, "nav.start") => "Get Started".to_string(),
    (Language::Zh, "footer.slogan") => "专注 Rust 技术栈：文档 / 博客 / 课程 / 案例".to_string(),
    (Language::En, "footer.slogan") => {
      "Focusing on Rust Stack: Docs / Blog / Courses / Showcase".to_string()
    }
    (_, k) => k.to_string(),
  }
}
