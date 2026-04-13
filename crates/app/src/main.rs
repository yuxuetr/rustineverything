use dioxus::prelude::*;
use dioxus::router::Router;

mod components;
mod i18n;
mod routes;
mod server;

use crate::i18n::init_i18n;
use crate::routes::Route;
use crate::server::get_aggregated_theme_css;

/// Static assets used by the application.
// Dioxus 0.7 默认在 crate root 的 assets 目录下寻找
const FAVICON: Asset = asset!("/assets/images/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/css/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
  dioxus::launch(App);
}

#[component]
fn App() -> Element {
  init_i18n();

  // Fetch aggregated theme CSS from WASM plugins
  let theme_css = use_resource(move || async move {
      let result = get_aggregated_theme_css().await;
      match &result {
          Ok(css) => println!("[Frontend] Fetched theme CSS (len: {})", css.len()),
          Err(e) => println!("[Frontend] Failed to fetch theme: {:?}", e),
      }
      result.unwrap_or_default()
  });

  // 使用 eval 动态注入和更新样式
  use_effect(move || {
      if let Some(css) = theme_css.read().as_ref() {
          let js = format!(
              r#"
              console.log("[Frontend] Injecting CSS into #wasm-theme-style");
              let styleTag = document.getElementById('wasm-theme-style');
              if (!styleTag) {{
                  styleTag = document.createElement('style');
                  styleTag.id = 'wasm-theme-style';
                  document.head.appendChild(styleTag);
              }}
              styleTag.innerHTML = `{}`;
              "#,
              css
          );
          dioxus::document::eval(&js);
      }
  });

  rsx! {
      // Head links
      document::Link { rel: "icon", href: FAVICON }
      document::Link { rel: "stylesheet", href: MAIN_CSS }
      document::Link { rel: "stylesheet", href: TAILWIND_CSS }
      
      // Global Fixed Styles (Static)
      document::Style { "
        body {{ 
          background-color: var(--color-bg, white); 
          color: var(--color-text, #0f172a);
          transition: background-color 0.3s ease, color 0.3s ease; 
        }}
        .dark body {{ 
          background-color: var(--color-bg, #020617); 
          color: var(--color-text, #f8fafc);
        }}
      " }

      // PrismJS for syntax highlighting
      document::Link { rel: "stylesheet", href: "https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/themes/prism-tomorrow.min.css" }
      document::Script { src: "https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/prism.min.js" }
      document::Script { src: "https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/plugins/autoloader/prism-autoloader.min.js" }

      // Main router entry
      Router::<Route> {}
  }
}
