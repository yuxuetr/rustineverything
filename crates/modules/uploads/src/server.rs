#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;
use dioxus::fullstack::{post, ServerFnError};
#[allow(unused_imports)]
use dioxus::prelude::*;

/// 自动探测资产根目录
#[allow(dead_code)]
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        path = PathBuf::from("../../assets");
    }
    path
}

/// 允许上传的最大字节数（5MB）
#[cfg(feature = "server")]
const UPLOAD_MAX_BYTES: usize = 5 * 1024 * 1024;

/// 允许上传的图片扩展名白名单
#[cfg(feature = "server")]
const UPLOAD_ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// 检测上传文件的 MIME 类型。针对 png/jpg/gif/webp 的 magic bytes 进行验证。
/// 返回 "mime/subtype" 或 None。
#[cfg(feature = "server")]
pub(crate) fn sniff_image_mime(data: &[u8]) -> Option<&'static str> {
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
pub(crate) fn safe_upload_filename(original: &str, mime: &str) -> Result<String, String> {
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

    // 2. 仅提取原始文件名的"stem"，忽略原扩展名，过滤不安全字符
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
        use rustineverything_core::engines::moderation::ModerationLabel;
        use rustineverything_core::session::current_session_user;
        use rustineverything_module_moderation::{enqueue_if_flagged, evaluate_submission};
        use rustineverything_sdk::{ImageRef, ModerationSubmission};

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
            .map_err(ServerFnError::new)?;

        // 6. 验证扩展名为白名单（双保险）
        let ext_ok = std::path::Path::new(&filename)
            .extension()
            .and_then(|s| s.to_str())
            .map(|e| UPLOAD_ALLOWED_EXTS.contains(&e))
            .unwrap_or(false);
        if !ext_ok {
            return Err(ServerFnError::new("不允许的文件扩展名".to_string()));
        }

        // 7. ── 审核：在写盘前用 base64 data URL 调 vision LLM ──
        // 注意：用 base64 内联避免把私有图暴露给 LLM 供商前 URL；
        // 默认 moderation.enabled = false → pipeline 空，直接 Allow。
        let session_user = current_session_user();
        let session_id = session_user.as_ref().map(|u| u.id);
        let session_nick = session_user
            .as_ref()
            .map(|u| u.nickname.clone())
            .unwrap_or_default();
        let data_url = format!(
            "data:{};base64,{}",
            mime,
            base64::engine::general_purpose::STANDARD.encode(&data)
        );
        let saved_url = format!("/uploads/{}", filename);
        let submission = ModerationSubmission::new("用户上传的图片")
            .with_kind("upload")
            .with_ref_path(&saved_url)
            .with_images(vec![ImageRef::url(data_url).with_media_type(mime)]);
        let verdict = evaluate_submission(submission).await;
        if verdict.label == ModerationLabel::Block {
            tracing::warn!(
                user = %session_nick,
                filename = %filename,
                score = verdict.score,
                reason = %verdict.reason,
                "moderation: upload BLOCKED (not saved)"
            );
            return Err(ServerFnError::new(format!(
                "图片被审核拒绝：{}",
                if verdict.reason.is_empty() {
                    "未通过内容审核".to_string()
                } else {
                    verdict.reason
                }
            )));
        }

        // 8. 写盘
        let dir_path = get_asset_root().join("uploads");
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path).map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        let path = dir_path.join(&filename);
        fs::write(&path, &data).map_err(|e| ServerFnError::new(format!("保存失败: {}", e)))?;

        // 9. Flag → 入审核队列。ref_id 留 None（upload 无独立业务表）；
        //    队列里记录保存后的 URL 让 admin 可以直接预览。
        let db_opt = rustineverything_core::db::get_or_init_pool().await.ok();
        if let Some(db) = db_opt {
            enqueue_if_flagged(
                &db,
                &verdict,
                "upload",
                None,
                &saved_url,
                session_id,
                "用户上传的图片",
                std::slice::from_ref(&saved_url),
            )
            .await;
        }

        Ok(saved_url)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (name, data_base64);
        Ok("".to_string())
    }
}

// ========== Tests ==========

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

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
