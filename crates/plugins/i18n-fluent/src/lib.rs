#![allow(clippy::missing_safety_doc)] // WASM ABI exports: 安全契约见 docs/PLUGIN_ABI.md
use fluent_bundle::{FluentBundle, FluentResource};
use rustineverything_sdk::{alloc, capabilities, pack_json, PluginManifest};
use std::slice;
use unic_langid::langid;

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let manifest = PluginManifest::new("i18n-fluent", "i18n Fluent", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::I18N)
    .with_description("中文 / EN 翻译插件 (Fluent)")
    .with_author("yuxuetr");
  pack_json(&manifest)
}

/// 静态加载 FTL 资源
const FTL_ZH: &str = "nav-blog = 博客\nnav-podcast = 播客\nnav-forum = 论坛";
const FTL_EN: &str = "nav-blog = Blog\nnav-podcast = Podcast\nnav-forum = Forum";

#[no_mangle]
pub unsafe extern "C" fn translate(ptr: *mut u8, len: usize) -> u64 {
  // 1. 获取输入 (JSON 格式: { "key": "...", "lang": "..." })
  let input_bytes = slice::from_raw_parts(ptr, len);
  let input: serde_json::Value = serde_json::from_slice(input_bytes).unwrap_or_default();

  let key = input["key"].as_str().unwrap_or("");
  let lang = input["lang"].as_str().unwrap_or("zh");

  // 2. 初始化 Fluent
  let lang_id = if lang == "en" { langid!("en-US") } else { langid!("zh-CN") };
  let mut bundle = FluentBundle::new(vec![lang_id]);

  let ftl = if lang == "en" { FTL_EN } else { FTL_ZH };
  let res = FluentResource::try_new(ftl.to_string()).expect("Failed to parse ftl");
  bundle.add_resource(res).expect("Failed to add resource");

  // 3. 执行翻译
  let msg = bundle.get_message(key);
  let pattern = msg.and_then(|m| m.value());

  let result_str = if let Some(p) = pattern {
    let mut errors = vec![];
    bundle.format_pattern(p, None, &mut errors).to_string()
  } else {
    key.to_string()
  };

  // 4. 返回结果 (通过 packed ptr/len)
  let result_bytes = result_str.into_bytes();
  let res_len = result_bytes.len();
  let res_ptr = alloc(res_len);

  let res_slice = slice::from_raw_parts_mut(res_ptr, res_len);
  res_slice.copy_from_slice(&result_bytes);

  ((res_ptr as u64) << 32) | (res_len as u64)
}
