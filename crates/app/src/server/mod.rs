use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use rustineverything_core::settings::SiteConfig;
use rustineverything_core::session::SessionUser;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use std::fs;

/// 自动探测资产根目录
#[allow(dead_code)]
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        path = PathBuf::from("../../assets");
    }
    path
}

// ========== 辅助：从 FullstackContext 读取 Cookie 中的用户 ==========

/// server-only: 从当前请求上下文的 Cookie 中解析 SessionUser
#[cfg(feature = "server")]
fn current_session_user() -> Option<SessionUser> {
    use dioxus::fullstack::FullstackContext;
    use rustineverything_core::session::parse_session_from_cookie_header;

    let ctx = FullstackContext::current()?;
    let parts = ctx.parts_mut();
    let cookie_str = parts
        .headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    drop(parts);
    parse_session_from_cookie_header(cookie_str.as_deref())
}

// ========== 站点配置 ==========

#[post("/api/site/config")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
    let config_path = get_asset_root().join("site.json");
    SiteConfig::from_file(config_path.to_str().unwrap())
        .map_err(|e| ServerFnError::new(format!("配置文件加载失败: {}", e)))
}

// ========== i18n ==========

#[post("/api/i18n/translate")]
pub async fn translate_server(key: String, lang: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join("i18n_fluent_plugin.wasm");

        if !wasm_path.exists() { return Ok(key); }
        let manager = rustineverything_core::shared_plugin_manager();
        let input = serde_json::json!({ "key": key, "lang": lang }).to_string();
        manager.call_path_with_string(&wasm_path, "translate", &input)
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { let _ = (key, lang); Ok(String::new()) }
}

// ========== 主题 ==========

#[post("/api/theme/aggregated-css")]
pub async fn get_aggregated_theme_css() -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join(&config.active_theme);

        if !wasm_path.exists() { return Ok("".to_string()); }
        let manager = rustineverything_core::shared_plugin_manager();
        Ok(manager.aggregate_theme_css_paths(&[wasm_path]))
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

// ========== Auth 辅助 (server-only) ==========

#[cfg(feature = "server")]
fn build_auth_service() -> (rustineverything_core::auth::AuthService, SiteConfig) {
    use rustineverything_core::auth::{AuthService, AuthConfig};

    // BASE_URL 未配置时 panic，避免生产环境误用 localhost
    let base_url = std::env::var("BASE_URL")
        .expect("BASE_URL 未配置，请在环境变量或 .env 中设置 BASE_URL");
    let config = AuthConfig { base_url };
    let site_config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
    let auth_service = AuthService::new(config, get_asset_root().join("plugins"));
    (auth_service, site_config)
}

#[cfg(feature = "server")]
fn find_plugin_filename(site_config: &SiteConfig, provider: &str) -> Option<String> {
    site_config.auth.providers.iter()
        .find(|p| p.id == provider)
        .map(|p| p.plugin.clone())
}

// ========== Auth 端点 ==========

#[post("/api/auth/providers")]
pub async fn get_auth_providers() -> Result<Vec<rustineverything_core::AuthProviderDisplay>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let (auth_service, site_config) = build_auth_service();
        Ok(auth_service.list_available_providers(&site_config))
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

#[post("/api/auth/login-url")]
pub async fn get_login_url(provider: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let (auth_service, site_config) = build_auth_service();
        let plugin_filename = find_plugin_filename(&site_config, &provider)
            .ok_or_else(|| ServerFnError::new(format!("未在 site.json 中配置 provider: {}", provider)))?;
        auth_service.get_auth_url(&provider, &plugin_filename)
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { Ok("".to_string()) }
}

/// 内部 auth callback — 仅 server 端调用，返回 (welcome_message, jwt_token)
#[cfg(feature = "server")]
pub async fn auth_callback_internal(
    code: String,
    provider: String,
    state: Option<String>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    use rustineverything_core::db::get_or_init_pool;
    use rustineverything_core::session::create_jwt;

    let (auth_service, site_config) = build_auth_service();
    let plugin_filename = find_plugin_filename(&site_config, &provider)
        .ok_or_else(|| format!("未在 site.json 中配置 provider: {}", provider))?;

    println!("[Auth Callback] provider={}, code_len={}, state={:?}", provider, code.len(), state);

    let db = get_or_init_pool().await?;

    let user = auth_service
        .handle_callback(&db, &provider, &plugin_filename, code, state)
        .await?;

    let jwt_token = create_jwt(&user)?;
    println!("[Auth Callback] Login success: user={}", user.nickname);
    Ok((format!("欢迎回来, {}!", user.nickname), jwt_token))
}

/// 获取当前登录用户 — 前端调用
#[post("/api/auth/me")]
pub async fn get_current_user() -> Result<Option<SessionUser>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        Ok(current_session_user())
    }
    #[cfg(not(feature = "server"))]
    { Ok(None) }
}

// ========== 上传 / Echo ==========

/// 允许上传的最大字节数（5MB）
#[cfg(feature = "server")]
const UPLOAD_MAX_BYTES: usize = 5 * 1024 * 1024;

/// 允许上传的图片扩展名白名单
#[cfg(feature = "server")]
const UPLOAD_ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// 检测上传文件的 MIME 类型。针对 png/jpg/gif/webp 的 magic bytes 进行验证。
/// 返回 “mime/subtype” 或 None。
#[cfg(feature = "server")]
fn sniff_image_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    // WebP 的 magic：RIFF????WEBP
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// 生成一个安全的上传文件名。去除路径分隔符与隐藏路径，只保留 ASCII 字母 / 数字 / `_-.` 。
/// 返回例子：`1747200000_a1b2c3.png`
#[cfg(feature = "server")]
fn safe_upload_filename(original: &str, mime: &str) -> Result<String, String> {
    use std::path::Path;
    use rand::Rng;

    // 1. 按 mime 推断扩展名
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return Err(format!("不支持的图片类型: {}", mime)),
    };

    // 2. 仅提取原始文件名的“stem”，忽略原扩展名，过滤不安全字符
    let stem = Path::new(original)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upload");
    let safe_stem: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .take(40)
        .collect();
    let safe_stem = if safe_stem.is_empty() { "upload".to_string() } else { safe_stem };

    // 3. 拼接随机后缀避免冲突
    let suffix: String = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(6)
        .map(|b| b as char)
        .collect();
    Ok(format!(
        "{}_{}_{}.{}",
        chrono::Utc::now().timestamp(),
        safe_stem,
        suffix,
        ext
    ))
}

#[post("/api/upload")]
pub async fn upload_image(name: String, data_base64: String) -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
      use base64::Engine as _;

      // 1. 取出 Base64 负载部分（接受 data:URL 或裸 base64）
      let base64_str = data_base64.split(',').last().unwrap_or(&data_base64);

      // 2. 预估 base64 解码后的字节数，提前拒绝超大负载
      //    base64 长度 * 3/4 即为原字节数的上限
      let estimated_bytes = base64_str.len().saturating_mul(3) / 4;
      if estimated_bytes > UPLOAD_MAX_BYTES {
          return Err(ServerFnError::new(format!(
              "文件过大（限制 {} MB）",
              UPLOAD_MAX_BYTES / 1024 / 1024
          )));
      }

      let data = base64::engine::general_purpose::STANDARD
          .decode(base64_str)
          .map_err(|e| ServerFnError::new(format!("解码失败: {}", e)))?;

      // 3. 硬限制：解码后不能超过 5MB
      if data.len() > UPLOAD_MAX_BYTES {
          return Err(ServerFnError::new(format!(
              "文件过大（限制 {} MB）",
              UPLOAD_MAX_BYTES / 1024 / 1024
          )));
      }

      // 4. 检测 MIME 类型是否为允许的图片格式
      let mime = sniff_image_mime(&data)
          .ok_or_else(|| ServerFnError::new("仅支持 png / jpg / gif / webp 图片".to_string()))?;

      // 5. 生成安全文件名，按 MIME 决定扩展名
      let filename = safe_upload_filename(&name, mime)
          .map_err(|e| ServerFnError::new(e))?;

      // 6. 验证扩展名为白名单（双保险）
      let ext_ok = std::path::Path::new(&filename)
          .extension()
          .and_then(|s| s.to_str())
          .map(|e| UPLOAD_ALLOWED_EXTS.contains(&e))
          .unwrap_or(false);
      if !ext_ok {
          return Err(ServerFnError::new("不允许的文件扩展名".to_string()));
      }

      let dir_path = get_asset_root().join("uploads");
      if !dir_path.exists() {
          fs::create_dir_all(&dir_path).map_err(|e| ServerFnError::new(e.to_string()))?;
      }

      let path = dir_path.join(&filename);
      fs::write(&path, &data).map_err(|e| ServerFnError::new(format!("保存失败: {}", e)))?;

      Ok(format!("/uploads/{}", filename))
  }
  #[cfg(not(feature = "server"))]
  { let _ = (name, data_base64); Ok("".to_string()) }
}

#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> { Ok(input) }

// ========== Tests ==========

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    // ========== 上传校验测试 ==========

    #[test]
    fn test_sniff_png_magic() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0xFF];
        assert_eq!(sniff_image_mime(&png), Some("image/png"));
    }

    #[test]
    fn test_sniff_jpeg_magic() {
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0xFF];
        assert_eq!(sniff_image_mime(&jpg), Some("image/jpeg"));
    }

    #[test]
    fn test_sniff_gif_magic() {
        assert_eq!(sniff_image_mime(b"GIF89a\x00"), Some("image/gif"));
        assert_eq!(sniff_image_mime(b"GIF87a\x00"), Some("image/gif"));
    }

    #[test]
    fn test_sniff_webp_magic() {
        let webp = b"RIFF\x00\x00\x00\x00WEBPVP8";
        assert_eq!(sniff_image_mime(webp), Some("image/webp"));
    }

    #[test]
    fn test_sniff_rejects_non_image_payload() {
        // 可执行文件 / 脚本 / 随机字节都应被拒绝
        assert_eq!(sniff_image_mime(b"#!/bin/bash\necho hi"), None);
        assert_eq!(sniff_image_mime(b"<script>alert(1)</script>"), None);
        assert_eq!(sniff_image_mime(&[0u8; 4]), None);
    }

    #[test]
    fn test_safe_filename_strips_path_traversal() {
        let name = safe_upload_filename("../../etc/passwd", "image/png").unwrap();
        assert!(name.ends_with(".png"));
        assert!(!name.contains(".."));
        assert!(!name.contains('/'));
        assert!(!name.contains('\\'));
    }

    #[test]
    fn test_safe_filename_unicode_collapses_to_default_stem() {
        // 中文 / 特殊字符全部过滤后变空，退化为 "upload"
        let name = safe_upload_filename("测试.png", "image/png").unwrap();
        assert!(name.ends_with(".png"));
        assert!(name.contains("upload"));
    }

    #[test]
    fn test_safe_filename_rejects_unsupported_mime() {
        let result = safe_upload_filename("x.bmp", "image/bmp");
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_filename_extension_matches_mime() {
        // 原始名字谎报扩展名，最终付文件名仍按检测出的 MIME 打定扩展名
        let name = safe_upload_filename("photo.exe", "image/jpeg").unwrap();
        assert!(name.ends_with(".jpg"));
        assert!(!name.ends_with(".exe"));
    }
}
