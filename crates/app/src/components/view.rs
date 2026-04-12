use dioxus::prelude::*;

#[component]
pub fn Container(children: Element) -> Element {
  rsx! {
      div { class: "mx-auto max-w-7xl px-4 sm:px-6 lg:px-8", {children} }
  }
}

#[component]
pub fn SectionTitle(title: String, subtitle: Option<String>) -> Element {
  rsx! {
      div { class: "text-center mb-10",
          h2 { class: "text-3xl font-bold tracking-tight text-[var(--color-text)] sm:text-4xl", "{title}" }
          if let Some(s) = subtitle {
              p { class: "mt-4 text-lg leading-8 text-[var(--color-text-muted)]", "{s}" }
          }
      }
  }
}
