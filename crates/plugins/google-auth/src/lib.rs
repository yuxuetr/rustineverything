use std::slice;
use rustineverything_sdk::{alloc, dealloc, AuthProviderConfig, AuthProviderDisplay, StandardUser};
use serde_json::Value;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_provider_config(_ptr: *mut u8, _len: usize) -> u64 {
    let config = AuthProviderConfig {
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        profile_url: "https://www.googleapis.com/oauth2/v2/userinfo".to_string(),
        scopes: vec![
            "openid".to_string(),
            "email".to_string(),
            "profile".to_string(),
        ],
    };

    let result_str = serde_json::to_string(&config).unwrap_or_default();
    unsafe { pack_result(result_str) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_display_info(_ptr: *mut u8, _len: usize) -> u64 {
    let display = AuthProviderDisplay {
        provider_id: "google".to_string(),
        display_name: "Google".to_string(),
        icon_svg: "M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1zM12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23zM5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62zM12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z".to_string(),
        brand_color: "#ffffff".to_string(),
    };

    let result_str = serde_json::to_string(&display).unwrap_or_default();
    unsafe { pack_result(result_str) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn map_profile(ptr: *mut u8, len: usize) -> u64 {
    let input_bytes = unsafe { slice::from_raw_parts(ptr, len) };
    let raw_profile: Value = serde_json::from_slice(input_bytes).unwrap_or_default();

    // Google userinfo API response fields:
    // id, email, verified_email, name, given_name, family_name, picture, locale
    let standard_user = StandardUser {
        external_id: raw_profile["id"].as_str().unwrap_or("0").to_string(),
        nickname: raw_profile["name"].as_str().unwrap_or("Google User").to_string(),
        avatar_url: raw_profile["picture"].as_str().map(|s| s.to_string()),
        email: raw_profile["email"].as_str().map(|s| s.to_string()),
        provider: "google".to_string(),
        raw_data: raw_profile.to_string(),
    };

    let result_str = serde_json::to_string(&standard_user).unwrap_or_default();
    unsafe { pack_result(result_str) }
}

/// Helper to pack a String result into the (ptr << 32 | len) return format
unsafe fn pack_result(s: String) -> u64 {
    let result_bytes = s.into_bytes();
    let res_len = result_bytes.len();
    let res_ptr = unsafe { alloc(res_len) };

    let res_slice = unsafe { slice::from_raw_parts_mut(res_ptr, res_len) };
    res_slice.copy_from_slice(&result_bytes);

    ((res_ptr as u64) << 32) | (res_len as u64)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn plugin_unused_fix() {
    unsafe { let _ = dealloc(std::ptr::null_mut(), 0); }
}
