use std::slice;
use rustineverything_sdk::{
    alloc, capabilities, dealloc, pack_json, AuthProviderConfig, AuthProviderDisplay,
    PluginManifest, StandardUser,
};
use serde_json::Value;

#[no_mangle]
pub unsafe extern "C" fn get_manifest(_ptr: *mut u8, _len: usize) -> u64 {
    let manifest = PluginManifest::new("github-auth", "GitHub Auth", env!("CARGO_PKG_VERSION"))
        .with_capability(capabilities::AUTH_PROVIDER)
        .with_description("GitHub OAuth2 登录插件")
        .with_author("yuxuetr");
    pack_json(&manifest)
}

#[no_mangle]
pub unsafe extern "C" fn get_provider_config(_ptr: *mut u8, _len: usize) -> u64 {
    let config = AuthProviderConfig {
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
        profile_url: "https://api.github.com/user".to_string(),
        scopes: vec!["read:user".to_string(), "user:email".to_string()],
        requires_pkce: false,
        token_auth_method: "form".to_string(),
    };

    let result_str = serde_json::to_string(&config).unwrap_or_default();
    let result_bytes = result_str.into_bytes();
    let res_len = result_bytes.len();
    let res_ptr = alloc(res_len);
    
    let res_slice = slice::from_raw_parts_mut(res_ptr, res_len);
    res_slice.copy_from_slice(&result_bytes);

    ((res_ptr as u64) << 32) | (res_len as u64)
}

#[no_mangle]
pub unsafe extern "C" fn get_display_info(_ptr: *mut u8, _len: usize) -> u64 {
    let display = AuthProviderDisplay {
        provider_id: "github".to_string(),
        display_name: "GitHub".to_string(),
        icon_svg: "M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.042-1.416-4.042-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.744.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.44-1.304.806-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z".to_string(),
        brand_color: "#24292f".to_string(),
    };

    let result_str = serde_json::to_string(&display).unwrap_or_default();
    let result_bytes = result_str.into_bytes();
    let res_len = result_bytes.len();
    let res_ptr = alloc(res_len);

    let res_slice = slice::from_raw_parts_mut(res_ptr, res_len);
    res_slice.copy_from_slice(&result_bytes);

    ((res_ptr as u64) << 32) | (res_len as u64)
}

#[no_mangle]
pub unsafe extern "C" fn map_profile(ptr: *mut u8, len: usize) -> u64 {
    let input_bytes = slice::from_raw_parts(ptr, len);
    let raw_profile: Value = serde_json::from_slice(input_bytes).unwrap_or_default();
    
    let standard_user = StandardUser {
        external_id: raw_profile["id"].as_i64().unwrap_or(0).to_string(),
        nickname: raw_profile["login"].as_str().unwrap_or("GitHub用户").to_string(),
        avatar_url: raw_profile["avatar_url"].as_str().map(|s| s.to_string()),
        email: raw_profile["email"].as_str().map(|s| s.to_string()),
        provider: "github".to_string(),
        raw_data: raw_profile.to_string(),
    };

    let result_str = serde_json::to_string(&standard_user).unwrap_or_default();
    let result_bytes = result_str.into_bytes();
    let res_len = result_bytes.len();
    let res_ptr = alloc(res_len);
    
    let res_slice = slice::from_raw_parts_mut(res_ptr, res_len);
    res_slice.copy_from_slice(&result_bytes);

    ((res_ptr as u64) << 32) | (res_len as u64)
}

#[no_mangle]
pub unsafe extern "C" fn plugin_unused_fix() {
    let _ = dealloc(std::ptr::null_mut(), 0);
}
