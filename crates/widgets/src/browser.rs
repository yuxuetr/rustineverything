//! 浏览器操作的 web-sys 实现（B1 / SEC-07）。
//!
//! 站点 CSP 不含 `'unsafe-eval'`。`document::eval` 用 `new Function` 执行脚本，
//! 被拦截后 wasm 运行时直接崩溃；`document::Title` 在客户端同样走 eval。这里用
//! web-sys 直接调用替代，`crates/app/tests/csp_no_eval.rs` 防止它们回到源码里。
//!
//! 非 wasm 目标（SSR、单测）上：读操作返回 `None` / 默认值，写操作什么也不做。
//! 这些函数只应在事件回调、`use_effect` 或 `spawn` 里调用。

use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

#[cfg(target_arch = "wasm32")]
fn document() -> Option<web_sys::Document> {
  web_sys::window()?.document()
}

#[cfg(target_arch = "wasm32")]
fn html_document() -> Option<web_sys::HtmlDocument> {
  document()?.dyn_into::<web_sys::HtmlDocument>().ok()
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
  web_sys::window()?.local_storage().ok()?
}

/// 整页跳转到 `url`。
pub fn navigate(url: &str) {
  #[cfg(target_arch = "wasm32")]
  if let Some(w) = web_sys::window() {
    let _ = w.location().set_href(url);
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = url;
}

/// 以 POST 整页提交到 `url`（无表单字段），用于登出这类有副作用、不能用 GET 的跳转。
pub fn post_navigate(url: &str) {
  #[cfg(target_arch = "wasm32")]
  {
    let submit = || -> Option<()> {
      let doc = document()?;
      let form = doc.create_element("form").ok()?.dyn_into::<web_sys::HtmlFormElement>().ok()?;
      form.set_method("post");
      form.set_action(url);
      doc.body()?.append_child(&form).ok()?;
      form.submit().ok()
    };
    let _ = submit();
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = url;
}

/// 重新加载当前页。
pub fn reload() {
  #[cfg(target_arch = "wasm32")]
  if let Some(w) = web_sys::window() {
    let _ = w.location().reload();
  }
}

/// 读取 cookie，值按 `decodeURIComponent` 解码。
pub fn cookie(name: &str) -> Option<String> {
  #[cfg(target_arch = "wasm32")]
  {
    let all = html_document()?.cookie().ok()?;
    let raw = all.split("; ").find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))?;
    js_sys::decode_uri_component(raw).ok().map(String::from)
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = name;
    None
  }
}

/// 写一个 `path=/; samesite=lax` 的 cookie，值经 `encodeURIComponent`；
/// `max_age_secs = 0` 即删除。
pub fn set_cookie(name: &str, value: &str, max_age_secs: u32) {
  #[cfg(target_arch = "wasm32")]
  if let Some(doc) = html_document() {
    let value = String::from(js_sys::encode_uri_component(value));
    let _ =
      doc.set_cookie(&format!("{name}={value}; path=/; max-age={max_age_secs}; samesite=lax"));
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = (name, value, max_age_secs);
}

/// `localStorage.getItem(key)`；存储不可用（隐私模式等）时为 `None`。
pub fn storage_get(key: &str) -> Option<String> {
  #[cfg(target_arch = "wasm32")]
  {
    local_storage()?.get_item(key).ok()?
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = key;
    None
  }
}

/// `localStorage.setItem(key, value)`，失败静默。
pub fn storage_set(key: &str, value: &str) {
  #[cfg(target_arch = "wasm32")]
  if let Some(s) = local_storage() {
    let _ = s.set_item(key, value);
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = (key, value);
}

/// 当前 URL 的查询参数。
pub fn query_param(name: &str) -> Option<String> {
  #[cfg(target_arch = "wasm32")]
  {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(name)
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = name;
    None
  }
}

/// 设置 `document.title`。页面里用 [`PageTitle`]，它在服务端也能输出 `<title>`。
pub fn set_title(title: &str) {
  #[cfg(target_arch = "wasm32")]
  if let Some(doc) = document() {
    doc.set_title(title);
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = title;
}

/// 给 `<html>`（`root = true`）或 `<body>` 加 / 去一个 class。
pub fn set_class(root: bool, class: &str, on: bool) {
  #[cfg(target_arch = "wasm32")]
  {
    let el =
      document().and_then(|d| if root { d.document_element() } else { d.body().map(Into::into) });
    if let Some(el) = el {
      let list = el.class_list();
      let _ = if on { list.add_1(class) } else { list.remove_1(class) };
    }
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = (root, class, on);
}

/// 用户的暗色偏好：`localStorage.theme`，没有则跟随系统。
pub fn dark_mode_preference() -> bool {
  #[cfg(target_arch = "wasm32")]
  {
    match storage_get("theme").as_deref() {
      Some(theme) => theme == "dark",
      None => web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
        .is_some_and(|m| m.matches()),
    }
  }
  #[cfg(not(target_arch = "wasm32"))]
  false
}

/// 切换暗色模式并记住选择（`<html class="dark">` + `localStorage.theme`）。
pub fn set_dark_mode(dark: bool) {
  set_class(true, "dark", dark);
  storage_set("theme", if dark { "dark" } else { "light" });
}

/// 对所有匹配 `selector` 的元素：`apply` 时以 `!important` 写入这些内联样式，
/// 否则移除它们。
pub fn set_inline_styles(selector: &str, styles: &[(&str, &str)], apply: bool) {
  #[cfg(target_arch = "wasm32")]
  {
    let Some(nodes) = document().and_then(|d| d.query_selector_all(selector).ok()) else {
      return;
    };
    for i in 0..nodes.length() {
      let Some(el) = nodes.item(i).and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok()) else {
        continue;
      };
      let style = el.style();
      for (prop, value) in styles {
        let _ = if apply {
          style.set_property_with_priority(prop, value, "important")
        } else {
          style.remove_property(prop).map(|_| ())
        };
      }
    }
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = (selector, styles, apply);
}

/// 调用页面脚本暴露的全局函数 `window[object][method](arg)`；`arg_json` 用
/// `JSON.parse` 解析（不是 eval）。对象或方法不存在、调用抛错时返回 `false`。
pub fn call_global(object: &str, method: &str, arg_json: Option<&str>) -> bool {
  #[cfg(target_arch = "wasm32")]
  {
    let call = || -> Option<()> {
      let window: JsValue = web_sys::window()?.into();
      let obj = js_sys::Reflect::get(&window, &object.into()).ok()?;
      let func =
        js_sys::Reflect::get(&obj, &method.into()).ok()?.dyn_into::<js_sys::Function>().ok()?;
      let arg = match arg_json {
        Some(json) => js_sys::JSON::parse(json).ok()?,
        None => JsValue::UNDEFINED,
      };
      func.call1(&obj, &arg).ok().map(|_| ())
    };
    call().is_some()
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = (object, method, arg_json);
    false
  }
}

/// `window[name] = JSON.parse(json)`，供页面脚本加载后自取。
pub fn set_global_json(name: &str, json: &str) {
  #[cfg(target_arch = "wasm32")]
  if let (Some(w), Ok(value)) = (web_sys::window(), js_sys::JSON::parse(json)) {
    let _ = js_sys::Reflect::set(&w, &name.into(), &value);
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = (name, json);
}

/// 写入剪贴板；浏览器拒绝（无权限、非安全上下文）时返回 `false`。
pub async fn copy_text(text: &str) -> bool {
  #[cfg(target_arch = "wasm32")]
  {
    let Some(w) = web_sys::window() else { return false };
    let promise = w.navigator().clipboard().write_text(text);
    wasm_bindgen_futures::JsFuture::from(promise).await.is_ok()
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = text;
    false
  }
}

/// 基于 `setTimeout` 的异步等待。
pub async fn sleep_ms(ms: i32) {
  #[cfg(target_arch = "wasm32")]
  {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
      if let Some(w) = web_sys::window() {
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
      }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
  }
  #[cfg(not(target_arch = "wasm32"))]
  let _ = ms;
}

/// 文档级 `keydown` 监听，被 drop 时移除。
pub struct KeyListener {
  #[cfg(target_arch = "wasm32")]
  closure: Closure<dyn FnMut(web_sys::KeyboardEvent)>,
}

/// 注册文档级 `keydown` 监听。`handler(key, ctrl_or_meta)` 返回 `true` 时
/// 阻止浏览器默认行为。
pub fn on_document_keydown(handler: impl FnMut(&str, bool) -> bool + 'static) -> KeyListener {
  #[cfg(target_arch = "wasm32")]
  {
    let mut handler = handler;
    let closure =
      Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
        if handler(&e.key(), e.ctrl_key() || e.meta_key()) {
          e.prevent_default();
        }
      });
    if let Some(doc) = document() {
      let _ = doc.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
    }
    KeyListener { closure }
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = handler;
    KeyListener {}
  }
}

impl Drop for KeyListener {
  fn drop(&mut self) {
    #[cfg(target_arch = "wasm32")]
    if let Some(doc) = document() {
      let _ =
        doc.remove_event_listener_with_callback("keydown", self.closure.as_ref().unchecked_ref());
    }
  }
}

/// 页面标题。替代 `document::Title`：它在客户端经 eval 设置标题，严格 CSP 下会
/// 崩溃。服务端仍交给 Dioxus 文档输出 `<title>`（不经 eval），客户端在
/// effect 里用 web-sys 设置。与 `document::Title` 一样不渲染任何节点。
#[component]
pub fn PageTitle(title: String) -> Element {
  #[cfg(target_arch = "wasm32")]
  use_effect(use_reactive!(|title| set_title(&title)));
  #[cfg(not(target_arch = "wasm32"))]
  {
    let doc = use_hook(dioxus::document::document);
    use_hook(|| doc.set_title(title.clone()));
  }
  VNode::empty()
}
