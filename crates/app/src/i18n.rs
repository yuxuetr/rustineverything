use crate::server::translate_server;
use dioxus::prelude::*;

pub use rustineverything_core::i18n::Language;

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
    (Language::Zh, "nav.cases") => "案例".to_string(),
    (Language::En, "nav.cases") => "Cases".to_string(),
    (Language::Zh, "nav.podcast") => "播客".to_string(),
    (Language::En, "nav.podcast") => "Podcast".to_string(),
    (Language::Zh, "nav.start") => "开始学习".to_string(),
    (Language::En, "nav.start") => "Get Started".to_string(),
    (Language::Zh, "auth.sign_in") => "登录".to_string(),
    (Language::En, "auth.sign_in") => "Sign In".to_string(),
    (Language::Zh, "auth.sign_in_desc") => "选择一种方式继续".to_string(),
    (Language::En, "auth.sign_in_desc") => "Choose a method to continue".to_string(),
    (Language::Zh, "auth.continue_with") => "继续".to_string(),
    (Language::En, "auth.continue_with") => "Continue with".to_string(),
    (Language::Zh, "auth.terms") => "登录即表示你同意我们的服务条款和隐私政策".to_string(),
    (Language::En, "auth.terms") => {
      "By signing in, you agree to our Terms and Privacy Policy".to_string()
    }
    (Language::Zh, "auth.logout") => "退出登录".to_string(),
    (Language::En, "auth.logout") => "Sign Out".to_string(),
    // ── 博客页 ──
    (Language::Zh, "blog.title") => "博客".to_string(),
    (Language::En, "blog.title") => "Blog".to_string(),
    (Language::Zh, "blog.subtitle") => "探索 Rust 的无限可能".to_string(),
    (Language::En, "blog.subtitle") => "Explore the boundless possibilities of Rust".to_string(),
    (Language::Zh, "blog.filter") => "标签筛选".to_string(),
    (Language::En, "blog.filter") => "Filter by Tag".to_string(),
    (Language::Zh, "blog.all") => "全部".to_string(),
    (Language::En, "blog.all") => "All".to_string(),
    (Language::Zh, "blog.empty") => "暂无文章".to_string(),
    (Language::En, "blog.empty") => "No articles yet".to_string(),
    (Language::Zh, "blog.articles") => "文章".to_string(),
    (Language::En, "blog.articles") => "Articles".to_string(),
    (Language::Zh, "blog.no_results") => "没有匹配该标签的文章".to_string(),
    (Language::En, "blog.no_results") => "No articles match this tag".to_string(),
    (_, k) => k.to_string(),
  }
}
