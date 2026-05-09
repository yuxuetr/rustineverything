use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use rustineverything_core::settings::SiteConfig;
use rustineverything_core::session::SessionUser;
use serde::{Deserialize, Serialize};
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
        use rustineverything_core::PluginManager;
        let _config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join("i18n_fluent_plugin.wasm");

        if !wasm_path.exists() { return Ok(key); }
        let wasm_bytes = fs::read(wasm_path).map_err(|e| ServerFnError::new(e.to_string()))?;
        let manager = PluginManager::new();
        let input = serde_json::json!({ "key": key, "lang": lang }).to_string();
        manager.call_with_string(&wasm_bytes, "translate", &input).map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "server"))]
    { Ok(key) }
}

// ========== 主题 ==========

#[post("/api/theme/aggregated-css")]
pub async fn get_aggregated_theme_css() -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::PluginManager;
        let config = SiteConfig::from_file(get_asset_root().join("site.json").to_str().unwrap()).unwrap_or_default();
        let plugin_dir = get_asset_root().join("plugins");
        let wasm_path = plugin_dir.join(&config.active_theme);

        if !wasm_path.exists() { return Ok("".to_string()); }
        let wasm_bytes = fs::read(wasm_path).map_err(|e| ServerFnError::new(e.to_string()))?;
        let manager = PluginManager::new();
        Ok(manager.aggregate_theme_css(&[wasm_bytes]))
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
    use rustineverything_core::db::init_db;
    use rustineverything_core::session::create_jwt;

    let (auth_service, site_config) = build_auth_service();
    let plugin_filename = find_plugin_filename(&site_config, &provider)
        .ok_or_else(|| format!("未在 site.json 中配置 provider: {}", provider))?;

    println!("[Auth Callback] provider={}, code_len={}, state={:?}", provider, code.len(), state);

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/rustineverything".to_string());
    let db = init_db(&db_url).await?;

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

// ========== 评论系统 (数据库) ==========

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub id: String,
    pub blog_id: String,
    pub content: String,
    pub author: String,
    pub author_avatar: Option<String>,
    pub user_id: Option<i32>,
    pub date: String,
}

#[post("/api/comments/list")]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::db::init_db;
        use rustineverything_core::entities::{comment, user};
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, QueryOrder};

        let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
        let db = init_db(&db_url).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let results = comment::Entity::find()
            .filter(comment::Column::BlogId.eq(&blog_id))
            .find_also_related(user::Entity)
            .order_by_desc(comment::Column::CreatedAt)
            .all(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let comments = results
            .into_iter()
            .map(|(c, u)| Comment {
                id: c.id.to_string(),
                blog_id: c.blog_id,
                content: c.content,
                author: u.as_ref().map(|u| u.nickname.clone()).unwrap_or_else(|| "已注销".to_string()),
                author_avatar: u.as_ref().and_then(|u| u.avatar_url.clone()),
                user_id: Some(c.user_id),
                date: c.created_at.format("%Y-%m-%d %H:%M").to_string(),
            })
            .collect();

        Ok(comments)
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

#[post("/api/comments/post")]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::db::init_db;
        use rustineverything_core::entities::comment;
        use sea_orm::{EntityTrait, Set};
        use chrono::Utc;

        let session_user = current_session_user()
            .ok_or_else(|| ServerFnError::new("请先登录后再发表评论"))?;

        let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
        let db = init_db(&db_url).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let new_comment = comment::ActiveModel {
            blog_id: Set(blog_id.clone()),
            user_id: Set(session_user.id),
            content: Set(content),
            created_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        };
        comment::Entity::insert(new_comment)
            .exec(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        get_comments(blog_id).await
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

// ========== 文档系统 ==========

/// 文档 frontmatter 元数据（类似 Docusaurus）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DocMeta {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub sidebar_label: Option<String>,  // 侧栏显示名称（覆盖 title）
    #[serde(default)]
    pub sidebar_position: Option<i32>,  // 侧栏排序（越小越前）
    #[serde(default)]
    pub image: Option<String>,          // OG 图片
    /// 子项排序方向："asc"（默认，升序）或 "desc"（降序）
    /// 在父目录的 index.md 中设置，控制该目录下子项的排序方向
    /// 适合：周报/日报等以递增编号但需要最新期优先的场景
    #[serde(default)]
    pub sort_children: Option<String>,
}

/// 文档树节点（最多三级）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocTreeNode {
    pub slug: String,
    pub title: String,
    pub path: String,
    pub has_content: bool,
    pub description: String,
    pub children: Vec<DocTreeNode>,
}

/// 文档内容响应（内容 + 元数据）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocContentResponse {
    pub content: String,
    pub meta: DocMeta,
}

/// 数据结构：_meta.json 中的条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct MetaEntry {
    slug: String,
    title: String,
}

/// 解析 frontmatter（YAML between --- delimiters）
#[cfg(feature = "server")]
fn parse_doc_frontmatter(content: &str) -> (DocMeta, String) {
    if !content.starts_with("---") {
        return (DocMeta::default(), content.to_string());
    }
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return (DocMeta::default(), content.to_string());
    }
    let meta: DocMeta = serde_yaml::from_str(parts[1]).unwrap_or_default();
    (meta, parts[2].to_string())
}

/// 从 index.md/index.mdx 提取元数据（标题、描述、排序等）
#[cfg(feature = "server")]
fn extract_doc_info(dir: &std::path::Path) -> (Option<String>, String, Option<i32>) {
    let (title, desc, pos, _) = extract_doc_info_full(dir);
    (title, desc, pos)
}

/// 完整提取（额外返回 sort_children）
#[cfg(feature = "server")]
fn extract_doc_info_full(dir: &std::path::Path) -> (Option<String>, String, Option<i32>, Option<String>) {
    let md = dir.join("index.md");
    let mdx = dir.join("index.mdx");
    let path = if md.exists() { md } else if mdx.exists() { mdx } else {
        return (None, String::new(), None, None);
    };
    let content = fs::read_to_string(&path).unwrap_or_default();
    let (meta, body) = parse_doc_frontmatter(&content);

    // 标题优先级：sidebar_label > frontmatter title > # heading > 目录名
    let title = meta.sidebar_label.clone()
        .or_else(|| if !meta.title.is_empty() { Some(meta.title.clone()) } else { None })
        .or_else(|| {
            for line in body.lines() {
                let trimmed = line.trim();
                if let Some(t) = trimmed.strip_prefix("# ") {
                    return Some(t.trim().to_string());
                }
            }
            None
        });

    (title, meta.description.clone(), meta.sidebar_position, meta.sort_children.clone())
}

/// 扫描目录生成文档树（递归，最多 3 级）
/// 优先级：_meta.json > 自动扫描目录（从 index.md 提取标题）
#[cfg(feature = "server")]
fn scan_doc_dir(dir: &std::path::Path, rel_prefix: &str, depth: u32) -> Vec<DocTreeNode> {
    if depth > 3 { return vec![]; }

    // 优先读取 _meta.json
    let meta_path = dir.join("_meta.json");
    let entries: Vec<MetaEntry> = if meta_path.exists() {
        fs::read_to_string(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        // 读取当前目录的 sort_children 设置
        let (_, _, _, sort_dir) = extract_doc_info_full(dir);
        let descending = sort_dir.as_deref().map(|s| s.eq_ignore_ascii_case("desc")).unwrap_or(false);

        // 自动扫描子目录，从 index.md 提取标题和排序
        let mut dirs: Vec<(String, String, String, Option<i32>)> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_string();
                if name.starts_with('_') || name.starts_with('.') { return None; }
                let (title, desc, pos) = extract_doc_info(&e.path());
                let title = title.unwrap_or_else(|| name.clone());
                Some((name, title, desc, pos))
            })
            .collect();
        // 按 sidebar_position 排序，无 position 的按字母顺序排在后面
        dirs.sort_by(|a, b| {
            let ord = match (a.3, b.3) {
                (Some(pa), Some(pb)) => pa.cmp(&pb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.0.cmp(&b.0),
            };
            if descending { ord.reverse() } else { ord }
        });
        dirs.into_iter().map(|(slug, title, _, _)| MetaEntry { slug, title }).collect()
    };

    entries.into_iter().map(|entry| {
        let child_dir = dir.join(&entry.slug);
        let rel_path = if rel_prefix.is_empty() {
            entry.slug.clone()
        } else {
            format!("{}/{}", rel_prefix, entry.slug)
        };
        let has_content = child_dir.join("index.md").exists() || child_dir.join("index.mdx").exists();
        let (_, desc, _) = extract_doc_info(&child_dir);
        let children = if child_dir.is_dir() {
            scan_doc_dir(&child_dir, &rel_path, depth + 1)
        } else {
            vec![]
        };
        DocTreeNode {
            slug: entry.slug,
            title: entry.title,
            path: rel_path,
            has_content,
            description: desc,
            children,
        }
    }).collect()
}

#[post("/api/docs/tree")]
pub async fn list_doc_tree() -> Result<Vec<DocTreeNode>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let docs_dir = get_asset_root().join("docs");
        if !docs_dir.exists() {
            return Ok(vec![]);
        }
        Ok(scan_doc_dir(&docs_dir, "", 1))
    }
    #[cfg(not(feature = "server"))]
    { Ok(vec![]) }
}

#[post("/api/docs/content")]
pub async fn get_doc_content(path: String) -> Result<DocContentResponse, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let docs_dir = get_asset_root().join("docs").join(&path);
        let md = docs_dir.join("index.md");
        let mdx = docs_dir.join("index.mdx");
        let filepath = if md.exists() { md } else if mdx.exists() { mdx } else {
            return Err(ServerFnError::new(format!("文档未找到: {}", path)));
        };
        let raw = fs::read_to_string(&filepath)
            .map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))?;
        let (meta, content) = parse_doc_frontmatter(&raw);
        Ok(DocContentResponse { content, meta })
    }
    #[cfg(not(feature = "server"))]
    { Ok(DocContentResponse { content: String::new(), meta: DocMeta::default() }) }
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
    use std::path::Path;
    use tempfile::TempDir;

    /// 辅助：在指定目录下创建 index.md（可选带 frontmatter）
    fn write_index(dir: &Path, frontmatter: Option<&str>, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let content = match frontmatter {
            Some(fm) => format!("---\n{}\n---\n\n{}", fm, body),
            None => body.to_string(),
        };
        std::fs::write(dir.join("index.md"), content).unwrap();
    }

    /// 辅助：提取一棵子树的 slug 列表（保留顺序）
    fn slugs(nodes: &[DocTreeNode]) -> Vec<String> {
        nodes.iter().map(|n| n.slug.clone()).collect()
    }

    fn find<'a>(nodes: &'a [DocTreeNode], slug: &str) -> &'a DocTreeNode {
        nodes.iter().find(|n| n.slug == slug)
            .unwrap_or_else(|| panic!("未找到节点: {}", slug))
    }

    #[test]
    fn test_frontmatter_parsing() {
        let raw = "---\ntitle: Hello\nkeywords: [a, b]\nsidebar_position: 5\n---\n\n# body";
        let (meta, body) = parse_doc_frontmatter(raw);
        assert_eq!(meta.title, "Hello");
        assert_eq!(meta.keywords, vec!["a", "b"]);
        assert_eq!(meta.sidebar_position, Some(5));
        assert!(body.trim_start().starts_with("# body"));
    }

    #[test]
    fn test_no_frontmatter_returns_default() {
        let (meta, body) = parse_doc_frontmatter("# Just heading\n\nsome body");
        assert_eq!(meta, DocMeta::default());
        assert!(body.contains("# Just heading"));
    }

    #[test]
    fn test_default_ascending_by_position() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        write_index(&docs.join("a"), Some("title: A\nsidebar_position: 3"), "# A");
        write_index(&docs.join("b"), Some("title: B\nsidebar_position: 1"), "# B");
        write_index(&docs.join("c"), Some("title: C\nsidebar_position: 2"), "# C");

        let tree = scan_doc_dir(docs, "", 1);
        assert_eq!(slugs(&tree), vec!["b", "c", "a"]);
    }

    #[test]
    fn test_descending_via_sort_children() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 父目录创建 index.md，并设置 sort_children: desc
        write_index(docs, Some("sort_children: desc"), "# root");

        write_index(&docs.join("issue-001"), Some("sidebar_position: 1"), "# 1");
        write_index(&docs.join("issue-002"), Some("sidebar_position: 2"), "# 2");
        write_index(&docs.join("issue-003"), Some("sidebar_position: 3"), "# 3");
        write_index(&docs.join("issue-005"), Some("sidebar_position: 5"), "# 5");
        write_index(&docs.join("issue-004"), Some("sidebar_position: 4"), "# 4");

        let tree = scan_doc_dir(docs, "", 1);
        assert_eq!(slugs(&tree), vec!["issue-005", "issue-004", "issue-003", "issue-002", "issue-001"]);
    }

    #[test]
    fn test_sort_children_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 大写 DESC 也应被识别
        write_index(docs, Some("sort_children: DESC"), "# root");
        write_index(&docs.join("v1"), Some("sidebar_position: 1"), "# 1");
        write_index(&docs.join("v2"), Some("sidebar_position: 2"), "# 2");

        let tree = scan_doc_dir(docs, "", 1);
        assert_eq!(slugs(&tree), vec!["v2", "v1"]);
    }

    #[test]
    fn test_no_position_falls_back_to_alphabetical() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 都没有 sidebar_position
        write_index(&docs.join("zebra"), None, "# Zebra");
        write_index(&docs.join("apple"), None, "# Apple");
        write_index(&docs.join("mango"), None, "# Mango");

        let tree = scan_doc_dir(docs, "", 1);
        assert_eq!(slugs(&tree), vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn test_mixed_position_and_no_position() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        write_index(&docs.join("first"), Some("sidebar_position: 1"), "# 1");
        write_index(&docs.join("middle"), Some("sidebar_position: 5"), "# 5");
        write_index(&docs.join("zzz"), None, "# zzz");
        write_index(&docs.join("aaa"), None, "# aaa");

        let tree = scan_doc_dir(docs, "", 1);
        // 有 position 的在前面（按 position 排），无 position 的在后面（按字母排）
        assert_eq!(slugs(&tree), vec!["first", "middle", "aaa", "zzz"]);
    }

    #[test]
    fn test_three_level_nesting_with_independent_sort() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 一级：axum（默认 asc）与 weekly（desc）
        write_index(&docs.join("axum"),   Some("sidebar_position: 1"), "# Axum");
        write_index(&docs.join("weekly"), Some("sidebar_position: 2\nsort_children: desc"), "# Weekly");

        // 二级：axum/basic（默认）与 axum/advance
        write_index(&docs.join("axum/basic"),   Some("sidebar_position: 1"), "# basic");
        write_index(&docs.join("axum/advance"), Some("sidebar_position: 2"), "# advance");

        // 三级：axum/basic/router 与 handler
        write_index(&docs.join("axum/basic/router"),  Some("sidebar_position: 1"), "# router");
        write_index(&docs.join("axum/basic/handler"), Some("sidebar_position: 2"), "# handler");

        // weekly 子项：递增编号
        write_index(&docs.join("weekly/issue-001"), Some("sidebar_position: 1"), "# 1");
        write_index(&docs.join("weekly/issue-002"), Some("sidebar_position: 2"), "# 2");
        write_index(&docs.join("weekly/issue-003"), Some("sidebar_position: 3"), "# 3");

        let tree = scan_doc_dir(docs, "", 1);

        // 顶层：axum 排在 weekly 前（默认 asc，不受子项的 sort_children 影响）
        assert_eq!(slugs(&tree), vec!["axum", "weekly"]);

        // axum 下面 —— 默认升序
        let axum = find(&tree, "axum");
        assert_eq!(slugs(&axum.children), vec!["basic", "advance"]);

        // axum/basic 下面 —— 默认升序
        let basic = find(&axum.children, "basic");
        assert_eq!(slugs(&basic.children), vec!["router", "handler"]);

        // weekly 下面 —— desc 降序
        let weekly = find(&tree, "weekly");
        assert_eq!(slugs(&weekly.children), vec!["issue-003", "issue-002", "issue-001"]);
    }

    #[test]
    fn test_sort_children_only_affects_direct_children() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 父目录 sort_children: desc
        write_index(docs, Some("sort_children: desc"), "# root");

        // 一级：子项 a/b 会被逆序
        write_index(&docs.join("a"), Some("sidebar_position: 1"), "# a");
        write_index(&docs.join("b"), Some("sidebar_position: 2"), "# b");

        // 二级：a 下面的 a-1/a-2 仍然应该是升序（父的 sort_children 不会传递）
        write_index(&docs.join("a/a-1"), Some("sidebar_position: 1"), "# a1");
        write_index(&docs.join("a/a-2"), Some("sidebar_position: 2"), "# a2");

        let tree = scan_doc_dir(docs, "", 1);
        // 一级逆序
        assert_eq!(slugs(&tree), vec!["b", "a"]);
        // 二级仍然升序
        let a = find(&tree, "a");
        assert_eq!(slugs(&a.children), vec!["a-1", "a-2"]);
    }

    #[test]
    fn test_path_propagation_in_nested_tree() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        write_index(&docs.join("axum"), Some("sidebar_position: 1"), "# Axum");
        write_index(&docs.join("axum/basic"), Some("sidebar_position: 1"), "# basic");
        write_index(&docs.join("axum/basic/router"), Some("sidebar_position: 1"), "# router");

        let tree = scan_doc_dir(docs, "", 1);
        let axum = find(&tree, "axum");
        assert_eq!(axum.path, "axum");
        let basic = find(&axum.children, "basic");
        assert_eq!(basic.path, "axum/basic");
        let router = find(&basic.children, "router");
        assert_eq!(router.path, "axum/basic/router");
    }

    #[test]
    fn test_max_depth_three_levels() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 创建 4 级嵌套，验证只扫描前 3 级
        write_index(&docs.join("l1/l2/l3/l4"), None, "# deep");
        write_index(&docs.join("l1/l2/l3"), None, "# l3");
        write_index(&docs.join("l1/l2"), None, "# l2");
        write_index(&docs.join("l1"), None, "# l1");

        let tree = scan_doc_dir(docs, "", 1);
        let l1 = find(&tree, "l1");
        let l2 = find(&l1.children, "l2");
        let l3 = find(&l2.children, "l3");
        // l3 是第 3 级，其 children 应为空（第 4 级被截断）
        assert!(l3.children.is_empty(), "超过 3 级的节点应不被扫描");
    }

    #[test]
    fn test_sidebar_label_overrides_title() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        write_index(&docs.join("x"),
            Some("title: Long Title For SEO\nsidebar_label: Short"),
            "# heading");

        let tree = scan_doc_dir(docs, "", 1);
        let x = find(&tree, "x");
        // 侧栏显示使用 sidebar_label
        assert_eq!(x.title, "Short");
    }

    #[test]
    fn test_title_falls_back_to_h1_then_slug() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        // 无 frontmatter，只有 # 标题
        write_index(&docs.join("with-h1"), None, "# From Heading");
        // 有 frontmatter 但不含标题相关字段，也没 h1
        write_index(&docs.join("no-title"), Some("description: just desc"), "some body");

        let tree = scan_doc_dir(docs, "", 1);
        assert_eq!(find(&tree, "with-h1").title, "From Heading");
        // 都拿不到时退化到目录名
        assert_eq!(find(&tree, "no-title").title, "no-title");
    }

    #[test]
    fn test_underscore_and_hidden_dirs_skipped() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path();

        write_index(&docs.join("visible"), None, "# visible");
        write_index(&docs.join("_private"), None, "# private");
        write_index(&docs.join(".hidden"), None, "# hidden");

        let tree = scan_doc_dir(docs, "", 1);
        assert_eq!(slugs(&tree), vec!["visible"]);
    }

    #[test]
    fn test_empty_dir_returns_empty_tree() {
        let tmp = TempDir::new().unwrap();
        let tree = scan_doc_dir(tmp.path(), "", 1);
        assert!(tree.is_empty());
    }

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
