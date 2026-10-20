use dioxus::prelude::*;

#[component]
pub fn Container(children: Element) -> Element {
  rsx! {
      div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8", {children} }
  }
}
