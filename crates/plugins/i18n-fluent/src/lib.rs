use fluent_bundle::{FluentBundle, FluentResource};
use sdk::{capabilities, plugin_export, PluginManifest};
use serde::Deserialize;
use unic_langid::langid;

const FTL_ZH: &str = "nav-blog = 博客\nnav-podcast = 播客\nnav-forum = 论坛";
const FTL_EN: &str = "nav-blog = Blog\nnav-podcast = Podcast\nnav-forum = Forum";

#[plugin_export]
fn get_manifest() -> PluginManifest {
  PluginManifest::new("i18n-fluent", "i18n Fluent", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::I18N)
    .with_description("中文 / EN 翻译插件 (Fluent)")
    .with_author("yuxuetr")
}

#[derive(Deserialize, Default)]
struct TranslateRequest {
  #[serde(default)]
  key: String,
  #[serde(default)]
  lang: String,
}

#[plugin_export]
fn translate(req: TranslateRequest) -> String {
  let lang_id = if req.lang == "en" { langid!("en-US") } else { langid!("zh-CN") };
  let mut bundle = FluentBundle::new(vec![lang_id]);
  let ftl = if req.lang == "en" { FTL_EN } else { FTL_ZH };
  let res = FluentResource::try_new(ftl.to_string()).unwrap_or_else(|(res, _errors)| res);
  let _ = bundle.add_resource(res);
  let msg = bundle.get_message(&req.key);
  let pattern = msg.and_then(|m| m.value());
  if let Some(p) = pattern {
    let mut errors = vec![];
    bundle.format_pattern(p, None, &mut errors).to_string()
  } else {
    req.key
  }
}
