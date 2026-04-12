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
const FAVICON: Asset = asset!("/assets/images/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/css/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/css/tailwind.css");

fn main() {
  dioxus::launch(App);
}

#[component]
fn App() -> Element {
  init_i18n();

  // Fetch aggregated theme CSS from WASM plugins
  let theme_css = use_resource(move || async move {
      get_aggregated_theme_css().await.unwrap_or_default()
  });

  rsx! {
      // Head links
      document::Link { rel: "icon", href: FAVICON }
      document::Link { rel: "stylesheet", href: MAIN_CSS }
      document::Link { rel: "stylesheet", href: TAILWIND_CSS }
      
      // Inject Dynamic Theme CSS from WASM Plugins
      if let Some(css) = theme_css.read().as_ref() {
          document::Style { "{css}" }
      }

      // PrismJS for syntax highlighting
      document::Link { rel: "stylesheet", href: "https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/themes/prism-tomorrow.min.css" }
      document::Script { src: "https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/prism.min.js" }
      document::Script { src: "https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/plugins/autoloader/prism-autoloader.min.js" }

      // Main router entry
      Router::<Route> {}
  }
}
