use app_core::AuthProviderDisplay;
use dioxus::prelude::*;
use dioxus_shadcn::{
  Dialog, DialogClose, DialogContent, DialogDescription, DialogOverlay, DialogTitle, Spinner,
};

use crate::i18n::{t, use_i18n, Language};
use crate::server::get_auth_providers;

#[component]
pub fn AuthModal(show: Signal<bool>) -> Element {
  let lang = use_i18n();

  // Fetch available providers from server (plugin-driven)
  let providers =
    use_resource(move || async move { get_auth_providers().await.unwrap_or_default() });

  let provider_list = providers.read();
  let provider_list = provider_list.as_ref().cloned().unwrap_or_default();

  rsx! {
      Dialog { open: show(), on_open_change: move |v| show.set(v),
          DialogOverlay { class: "z-[100] backdrop-blur-sm" }
          DialogContent { class: "z-[100] max-w-md gap-0 rounded-2xl p-8 shadow-2xl",
              DialogClose { class: "p-1",
                  svg { class: "w-5 h-5", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", "aria-hidden": "true",
                      path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M6 18L18 6M6 6l12 12" }
                  }
                  span { class: "sr-only", "{t(lang(), \"auth.close\")}" }
              }

              div { class: "text-center mb-8",
                  DialogTitle { class: "text-2xl font-bold mb-2", "{t(lang(), \"auth.sign_in\")}" }
                  DialogDescription { "{t(lang(), \"auth.sign_in_desc\")}" }
              }

              // Provider buttons (dynamic from plugins)
              div { class: "flex flex-col gap-3",
                  if provider_list.is_empty() {
                      Spinner { class: "mx-auto my-4 border-t-primary" }
                  }

                  for provider in provider_list.iter() {
                      {render_provider_button(provider, lang())}
                  }
              }

              div { class: "my-6 h-px bg-border" }

              p { class: "text-center text-xs text-muted-foreground",
                  "{t(lang(), \"auth.terms\")}"
              }
          }
      }
  }
}

fn render_provider_button(provider: &AuthProviderDisplay, lang: Language) -> Element {
  let provider_id = provider.provider_id.clone();
  let display_name = provider.display_name.clone();
  let icon_svg = provider.icon_svg.clone();
  let brand_color = provider.brand_color.clone();

  // Determine text color based on brand color brightness
  let text_color = if is_light_color(&brand_color) {
    "rgb(55, 65, 81)" // gray-700
  } else {
    "white"
  };

  let border = if is_light_color(&brand_color) { "border: 1px solid #d1d5db;" } else { "" };

  let btn_style = format!("background-color: {}; color: {}; {}", brand_color, text_color, border);

  let label = if lang == Language::En {
    format!("{} {}", t(lang, "auth.continue_with"), display_name)
  } else {
    format!("{} {}", display_name, t(lang, "auth.continue_with"))
  };

  rsx! {
      button {
          key: "{provider_id}",
          r#type: "button",
          class: "flex items-center justify-center gap-3 w-full px-4 py-3 rounded-xl text-sm font-semibold transition-all duration-150 cursor-pointer hover:opacity-90",
          style: "{btn_style}",
          onclick: move |_| {
              // Phase 7.2：直接跳转到 server 路由，由服务端在重定向响应里下发
              // 加密的 oauth_pkce cookie（state + verifier）。不再走 server fn
              // 取 URL，避免 PKCE 状态丢失的 race。
              widgets::browser::navigate(&format!("/api/auth/login/{}", provider_id));
          },

          svg {
              class: "w-5 h-5 shrink-0",
              fill: "currentColor",
              view_box: "0 0 24 24",
              path { d: "{icon_svg}" }
          }

          span { "{label}" }
      }
  }
}

/// Simple heuristic to determine if a hex color is "light"
fn is_light_color(hex: &str) -> bool {
  // brand_color 来自插件 manifest：先确认是 ASCII 十六进制，再按字节切片。
  let Some(hex) =
    hex.trim_start_matches('#').get(..6).filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()))
  else {
    return false;
  };
  let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0) as f32;
  let (r, g, b) = (channel(0), channel(2), channel(4));
  // Relative luminance
  (0.299 * r + 0.587 * g + 0.114 * b) > 186.0
}

#[cfg(test)]
mod tests {
  use super::is_light_color;

  #[test]
  fn reads_hex_brightness() {
    assert!(is_light_color("#ffffff"));
    assert!(!is_light_color("#24292f"));
    assert!(!is_light_color("#fff"));
  }

  #[test]
  fn non_hex_brand_color_is_not_light_and_does_not_panic() {
    // 插件 manifest 里的 brand_color 不受站点控制；多字节字符曾让字节切片 panic。
    assert!(!is_light_color("#aé1234"));
    assert!(!is_light_color("#ééééé"));
    assert!(!is_light_color("red"));
  }
}
