#![allow(clippy::missing_safety_doc)] // WASM ABI exports: 安全契约见 docs/PLUGIN_ABI.md
use rustineverything_sdk::{
  AuthProviderConfig, AuthProviderDisplay, PluginManifest, StandardUser, alloc, capabilities,
  dealloc, pack_json,
};
use serde_json::Value;
use std::slice;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
  let manifest = PluginManifest::new("twitter-auth", "X (Twitter) Auth", env!("CARGO_PKG_VERSION"))
    .with_capability(capabilities::AUTH_PROVIDER)
    .with_description("X (Twitter) OAuth2 + PKCE 登录插件")
    .with_author("yuxuetr");
  pack_json(&manifest)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_provider_config(_ptr: *mut u8, _len: usize) -> u64 {
  // Twitter OAuth 2.0 (with PKCE)
  let config = AuthProviderConfig {
    auth_url: "https://twitter.com/i/oauth2/authorize".to_string(),
    token_url: "https://api.x.com/2/oauth2/token".to_string(),
    profile_url: "https://api.x.com/2/users/me?user.fields=profile_image_url,name,username"
      .to_string(),
    scopes: vec!["users.read".to_string(), "tweet.read".to_string()],
    requires_pkce: true,
    token_auth_method: "basic_auth".to_string(),
  };

  let result_str = serde_json::to_string(&config).unwrap_or_default();
  unsafe { pack_result(result_str) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_display_info(_ptr: *mut u8, _len: usize) -> u64 {
  let display = AuthProviderDisplay {
        provider_id: "twitter".to_string(),
        display_name: "X (Twitter)".to_string(),
        // X logo
        icon_svg: "M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z".to_string(),
        brand_color: "#000000".to_string(),
    };

  let result_str = serde_json::to_string(&display).unwrap_or_default();
  unsafe { pack_result(result_str) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn map_profile(ptr: *mut u8, len: usize) -> u64 {
  let input_bytes = unsafe { slice::from_raw_parts(ptr, len) };
  let raw_profile: Value = serde_json::from_slice(input_bytes).unwrap_or_default();

  // Twitter API v2 /users/me response: { data: { id, name, username, profile_image_url } }
  let data = &raw_profile["data"];
  let standard_user = StandardUser {
    external_id: data["id"].as_str().unwrap_or("0").to_string(),
    nickname: data["name"]
      .as_str()
      .or_else(|| data["username"].as_str())
      .unwrap_or("X User")
      .to_string(),
    avatar_url: data["profile_image_url"].as_str().map(|s| s.to_string()),
    email: None, // Twitter API v2 doesn't return email in basic scope
    provider: "twitter".to_string(),
    raw_data: raw_profile.to_string(),
  };

  let result_str = serde_json::to_string(&standard_user).unwrap_or_default();
  unsafe { pack_result(result_str) }
}

/// Helper to pack a String result into the (ptr << 32 | len) return format
unsafe fn pack_result(s: String) -> u64 {
  let result_bytes = s.into_bytes();
  let res_len = result_bytes.len();
  let res_ptr = alloc(res_len);

  let res_slice = unsafe { slice::from_raw_parts_mut(res_ptr, res_len) };
  res_slice.copy_from_slice(&result_bytes);

  ((res_ptr as u64) << 32) | (res_len as u64)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn plugin_unused_fix() {
  unsafe {
    dealloc(std::ptr::null_mut(), 0);
  }
}
