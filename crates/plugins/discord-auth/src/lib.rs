use std::slice;
use rustineverything_sdk::{alloc, dealloc, AuthProviderConfig, AuthProviderDisplay, StandardUser};
use serde_json::Value;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_provider_config(_ptr: *mut u8, _len: usize) -> u64 {
    let config = AuthProviderConfig {
        auth_url: "https://discord.com/oauth2/authorize".to_string(),
        token_url: "https://discord.com/api/oauth2/token".to_string(),
        profile_url: "https://discord.com/api/users/@me".to_string(),
        scopes: vec!["identify".to_string(), "email".to_string()],
    };

    let result_str = serde_json::to_string(&config).unwrap_or_default();
    unsafe { pack_result(result_str) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_display_info(_ptr: *mut u8, _len: usize) -> u64 {
    let display = AuthProviderDisplay {
        provider_id: "discord".to_string(),
        display_name: "Discord".to_string(),
        icon_svg: "M20.317 4.37a19.791 19.791 0 00-4.885-1.515.074.074 0 00-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 00-5.487 0 12.64 12.64 0 00-.617-1.25.077.077 0 00-.079-.037A19.736 19.736 0 003.677 4.37a.07.07 0 00-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 00.031.057 19.9 19.9 0 005.993 3.03.078.078 0 00.084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 00-.041-.106 13.107 13.107 0 01-1.872-.892.077.077 0 01-.008-.128 10.2 10.2 0 00.372-.292.074.074 0 01.077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 01.078.01c.12.098.246.198.373.292a.077.077 0 01-.006.127 12.299 12.299 0 01-1.873.892.077.077 0 00-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 00.084.028 19.839 19.839 0 006.002-3.03.077.077 0 00.032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 00-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.947 2.418-2.157 2.418z".to_string(),
        brand_color: "#5865F2".to_string(),
    };

    let result_str = serde_json::to_string(&display).unwrap_or_default();
    unsafe { pack_result(result_str) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn map_profile(ptr: *mut u8, len: usize) -> u64 {
    let input_bytes = unsafe { slice::from_raw_parts(ptr, len) };
    let raw_profile: Value = serde_json::from_slice(input_bytes).unwrap_or_default();

    // Discord user object fields:
    // id, username, discriminator, avatar, email, global_name
    let user_id = raw_profile["id"].as_str().unwrap_or("0");
    let username = raw_profile["global_name"]
        .as_str()
        .or_else(|| raw_profile["username"].as_str())
        .unwrap_or("Discord User");
    let avatar_hash = raw_profile["avatar"].as_str();
    let avatar_url = avatar_hash.map(|hash| {
        format!("https://cdn.discordapp.com/avatars/{}/{}.png", user_id, hash)
    });

    let standard_user = StandardUser {
        external_id: user_id.to_string(),
        nickname: username.to_string(),
        avatar_url,
        email: raw_profile["email"].as_str().map(|s| s.to_string()),
        provider: "discord".to_string(),
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
