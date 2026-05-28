use crate::server::echo_server;
use dioxus::prelude::*;

/// Echo component that demonstrates a fullstack server function.
///
/// Renders an input box and shows the echoed response from the server
/// under the field once the server replies.
#[component]
pub fn Echo() -> Element {
  let mut response = use_signal(String::new);

  rsx! {
      div {
          id: "echo",
          class: "mt-8 max-w-xl mx-auto px-4",
          h4 {
              class: "text-lg font-semibold mb-2",
              "ServerFn Echo 示例"
          }
          p {
              class: "text-sm text-slate-500 mb-4",
              "在输入框中输入内容后，文本会通过 server function 发送到服务器，并将响应结果展示在下方。"
          }
          input {
              class: "w-full border border-slate-300 rounded px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500",
              r#type: "text",
              placeholder: "Type here to echo...",
              oninput: move |event| async move {
                  // Call the server function with the current input value.
                  // This runs on the server in a fullstack setup and returns
                  // the echoed string back to the client.
                  if let Ok(data) = echo_server(event.value()).await {
                      response.set(data);
                  }
              },
          }

          if !response().is_empty() {
              p {
                  class: "mt-3 text-sm text-slate-700",
                  "Server echoed: "
                  i { "{response}" }
              }
          }
      }
  }
}
