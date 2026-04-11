use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct ContainerProps {
  pub children: Element,
}

/// A simple max-width container used across pages.
#[component]
pub fn Container(props: ContainerProps) -> Element {
  rsx! {
      div { class: "max-w-6xl mx-auto px-4 sm:px-6 lg:px-8", {props.children} }
  }
}

#[derive(Clone, PartialEq, Props)]
pub struct SectionTitleProps {
  pub title: String,
  #[props(optional)]
  pub subtitle: Option<String>,
}

#[component]
pub fn SectionTitle(props: SectionTitleProps) -> Element {
  rsx! {
      div { class: "mb-6",
          h2 { class: "text-2xl md:text-3xl font-bold text-slate-900 dark:text-white", "{props.title}" }
          if let Some(subtitle) = props.subtitle.clone() {
              p { class: "mt-2 text-slate-600 dark:text-slate-300", "{subtitle}" }
          }
      }
  }
}
