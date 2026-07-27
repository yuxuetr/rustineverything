#[cfg(feature = "server")]
pub mod auth_routes;
#[cfg(feature = "server")]
pub mod health;
#[cfg(feature = "server")]
pub mod pay_routes;
#[cfg(feature = "server")]
pub mod rate_limit;
#[cfg(feature = "server")]
pub mod security;
#[cfg(feature = "server")]
pub mod seo;
#[cfg(feature = "server")]
pub mod static_assets;

use app_core::session::SessionUser;
use app_core::settings::SiteConfig;
#[cfg(feature = "server")]
use app_core::utils::get_asset_root;
use dioxus::fullstack::{post, ServerFnError};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ========== 辅助：从 FullstackContext 读取 Cookie 中的用户 ==========

/// server-only: 从当前请求上下文的 Cookie 中解析 SessionUser
#[cfg(feature = "server")]
fn current_session_user() -> Option<SessionUser> {
  use app_core::session::parse_session_from_cookie_header;
  use dioxus::fullstack::FullstackContext;

  let ctx = FullstackContext::current()?;
  let parts = ctx.parts_mut();
  let cookie_str = parts.headers.get("cookie").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
  drop(parts);
  parse_session_from_cookie_header(cookie_str.as_deref())
}

/// server-only: 读当前请求 Cookie 头中指定 key 的值（Phase 3.1 主题覆盖用）。
#[cfg(feature = "server")]
fn read_request_cookie(name: &str) -> Option<String> {
  use dioxus::fullstack::FullstackContext;
  let ctx = FullstackContext::current()?;
  let parts = ctx.parts_mut();
  let cookie_str = parts.headers.get("cookie").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
  drop(parts);
  let raw = cookie_str?;
  for pair in raw.split(';') {
    let pair = pair.trim();
    if let Some((k, v)) = pair.split_once('=') {
      if k == name && !v.is_empty() {
        return Some(v.to_string());
      }
    }
  }
  None
}

/// Cookie 名（Phase 3.1）：存储用户选择的主题插件文件名。
#[cfg(feature = "server")]
pub const THEME_COOKIE_NAME: &str = "site_theme";

// ========== 站点配置 ==========

#[post("/api/site/config")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let config_path = get_asset_root().join("site.json");
    // S10：mtime 缓存读取；返回需要 owned，clone 一次（配置体积小）。
    SiteConfig::load_cached(config_path.to_str().unwrap_or_default())
      .map(|cfg| (*cfg).clone())
      .map_err(|e| ServerFnError::new(format!("配置文件加载失败: {}", e)))
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(SiteConfig::default())
  }
}

// ========== 插件浏览（Phase 5.5 公开页） ==========

/// 公开的插件信息（不含 admin-only 的凭据/配置状态）。供 `/plugins` 浏览页用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicPluginInfo {
  pub filename: String,
  pub id: String,
  pub name: String,
  pub version: String,
  pub description: String,
  pub capabilities: Vec<String>,
  /// ABI 是否与当前宿主兼容。
  pub abi_compatible: bool,
}

/// 列出 `assets/plugins/` 中已安装且导出了 `get_manifest` 的插件（公开，无需登录）。
/// 无 manifest 的老插件被跳过。
#[post("/api/plugins/public-list")]
pub async fn list_public_plugins() -> Result<Vec<PublicPluginInfo>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::PluginManifest;

    let plugin_dir = get_asset_root().join("plugins");
    let manager = app_core::shared_plugin_manager();
    let entries = match std::fs::read_dir(&plugin_dir) {
      Ok(e) => e,
      Err(_) => return Ok(vec![]),
    };

    let mut out: Vec<PublicPluginInfo> = Vec::new();
    for entry in entries.flatten() {
      let name = match entry.file_name().to_str() {
        Some(s) => s.to_string(),
        None => continue,
      };
      if !name.ends_with(".wasm") {
        continue;
      }
      let path = entry.path();
      let manifest_json = match manager.call_path_with_string(&path, "get_manifest", "").await {
        Ok(j) => j,
        Err(_) => continue, // 无 manifest（老插件）→ 跳过
      };
      let m: PluginManifest = match serde_json::from_str(&manifest_json) {
        Ok(m) => m,
        Err(_) => continue,
      };
      out.push(PublicPluginInfo {
        filename: name,
        abi_compatible: m.is_compatible(),
        id: m.id,
        name: m.name,
        version: m.version,
        description: m.description,
        capabilities: m.capabilities,
      });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

// ========== i18n ==========

#[post("/api/i18n/translate")]
pub async fn translate_server(key: String, lang: String) -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let plugin_dir = get_asset_root().join("plugins");
    let wasm_path = plugin_dir.join("i18n_fluent_plugin.wasm");

    if !wasm_path.exists() {
      return Ok(key);
    }
    let manager = app_core::shared_plugin_manager();
    let input = serde_json::json!({ "key": key, "lang": lang }).to_string();
    manager
      .call_path_with_string(&wasm_path, "translate", &input)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (key, lang);
    Ok(String::new())
  }
}

// ========== 主题 ==========

/// 可选主题的展示信息（Phase 3.1）。前端 ThemePicker 用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeInfo {
  /// 插件文件名（如 `theme_ocean_plugin.wasm`），概括 cookie 存储的值。
  pub filename: String,
  /// 从插件 manifest 读到的 id（如 `theme-ocean`），不可用时以文件名去后缀代替。
  pub id: String,
  /// 展示名（如 `Theme Ocean`）；没读到 manifest 时同 id。
  pub label: String,
  /// 在当前 site.json::themes 栈中是否默认激活的最顶层主题。
  pub is_active: bool,
}

#[post("/api/theme/aggregated-css")]
pub async fn get_aggregated_theme_css() -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::engines::theme::theme_with_override;

    let asset_root = get_asset_root();
    // S10：热路径（每次页面渲染拉主题 CSS）——mtime 缓存读取。
    let config = SiteConfig::load_cached(asset_root.join("site.json").to_str().unwrap_or_default())
      .unwrap_or_default();
    let plugin_dir = asset_root.join("plugins");

    // Phase 3.1：主题栈 + 可选 cookie 覆盖。
    let stack: Vec<std::path::PathBuf> = config
      .theme_stack()
      .into_iter()
      .filter(|name| !name.is_empty())
      .map(|name| plugin_dir.join(name))
      .collect();
    let cookie = read_request_cookie(THEME_COOKIE_NAME);
    let resolved = theme_with_override(&stack, &plugin_dir, cookie.as_deref());

    if resolved.is_empty() {
      return Ok(String::new());
    }
    let manager = app_core::shared_plugin_manager();
    Ok(manager.aggregate_theme_css_paths(&resolved).await)
  }
  #[cfg(not(feature = "server"))]
  {
    Ok("".to_string())
  }
}

/// Phase 3.1：枚举 `assets/plugins/` 下声明为 `theme` 的插件。
///
/// 实现策略：递归扫描 wasm 文件，试读 manifest，过滤 capability 含 `theme` 的。
/// 失败不 manifest 的插件以“文件名含 `theme`”启发式判定为主题，以保证老插件可见。
#[post("/api/theme/list")]
pub async fn list_available_themes() -> Result<Vec<ThemeInfo>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::{capabilities, PluginManifest};

    let asset_root = get_asset_root();
    let plugin_dir = asset_root.join("plugins");
    // S10：mtime 缓存读取。
    let config = SiteConfig::load_cached(asset_root.join("site.json").to_str().unwrap_or_default())
      .unwrap_or_default();
    let stack = config.theme_stack();
    // 激活态优先看用户 cookie（与 `get_aggregated_theme_css` 的覆盖语义一致），
    // 没有 cookie 时回退到 site.json 主题栈顶。
    let active_top = read_request_cookie(THEME_COOKIE_NAME)
      .map(|c| c.trim().to_string())
      .filter(|c| !c.is_empty())
      .unwrap_or_else(|| stack.last().cloned().unwrap_or_default());

    let manager = app_core::shared_plugin_manager();

    let entries = match std::fs::read_dir(&plugin_dir) {
      Ok(e) => e,
      Err(_) => return Ok(vec![]),
    };

    let mut out: Vec<ThemeInfo> = Vec::new();
    for entry in entries.flatten() {
      let name = match entry.file_name().to_str() {
        Some(s) => s.to_string(),
        None => continue,
      };
      if !name.ends_with(".wasm") {
        continue;
      }
      let path = entry.path();

      // 读 manifest。读不到时以启发式判定。
      let manifest_json = manager.call_path_with_string(&path, "get_manifest", "").await.ok();
      let (id, label, is_theme) =
        match manifest_json.as_deref().and_then(|s| serde_json::from_str::<PluginManifest>(s).ok())
        {
          Some(m) => {
            let is_theme = m.has_capability(capabilities::THEME);
            (m.id.clone(), m.name.clone(), is_theme)
          }
          None => {
            let lower = name.to_lowercase();
            let is_theme = lower.contains("theme");
            let stem = name.trim_end_matches(".wasm").to_string();
            (stem.clone(), stem, is_theme)
          }
        };
      if !is_theme {
        continue;
      }
      out.push(ThemeInfo { filename: name.clone(), id, label, is_active: name == active_top });
    }
    out.sort_by_key(|a| a.label.to_lowercase());
    Ok(out)
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

/// Phase 3.1：设置用户主题 cookie（覆盖主题栈最后一项）。
///
/// 传入空字符串表示“重置”（删除 cookie）。其他值会被严格校验：
/// 1. 不允许路径分隔符 / `..`
/// 2. 必须以 `.wasm` 结尾
/// 3. 必须在 `assets/plugins/` 中实际存在
/// 写 `Set-Cookie: site_theme=...; HttpOnly; Path=/; Max-Age=31536000; SameSite=Lax`。
/// 生产环境（`BASE_URL` 以 https 开头）额外附加 `Secure`。
#[post("/api/theme/set")]
pub async fn set_user_theme(filename: String) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use dioxus::fullstack::FullstackContext;

    let trimmed = filename.trim();
    let cookie_value = if trimmed.is_empty() {
      // 清除 cookie
      String::new()
    } else {
      // 校验输入
      if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(ServerFnError::new("主题名包含非法字符".to_string()));
      }
      if !trimmed.ends_with(".wasm") {
        return Err(ServerFnError::new("主题名必须以 .wasm 结尾".to_string()));
      }
      let path = get_asset_root().join("plugins").join(trimmed);
      if !path.exists() {
        return Err(ServerFnError::new(format!("主题插件不存在: {}", trimmed)));
      }
      trimmed.to_string()
    };

    let secure_flag = std::env::var("BASE_URL")
      .ok()
      .filter(|u| u.starts_with("https://"))
      .map(|_| "; Secure")
      .unwrap_or("");

    // 主题名是非敏感展示偏好，不使用 HttpOnly；前端也会用 document.cookie 兜底写入，
    // 确保随后的主题 CSS 重新请求能立即携带新 cookie，刷新后也能保持选择。
    let header_value = if cookie_value.is_empty() {
      format!("{}=; Path=/; Max-Age=0; SameSite=Lax{}", THEME_COOKIE_NAME, secure_flag)
    } else {
      format!(
        "{}={}; Path=/; Max-Age=31536000; SameSite=Lax{}",
        THEME_COOKIE_NAME, cookie_value, secure_flag
      )
    };

    let parsed: axum::http::HeaderValue =
      header_value.parse().map_err(|e| ServerFnError::new(format!("不合法 Cookie 头: {}", e)))?;
    if let Some(ctx) = FullstackContext::current() {
      ctx.add_response_header(axum::http::header::SET_COOKIE, parsed);
    }
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = filename;
    Ok(())
  }
}

// ========== Auth 辅助 (server-only) ==========

#[cfg(feature = "server")]
fn build_auth_service() -> (app_core::auth::AuthService, SiteConfig) {
  use app_core::auth::{AuthConfig, AuthService};

  // BASE_URL 未配置时 panic，避免生产环境误用 localhost
  // S9 豁免：与启动门禁同策略，缺失即 panic 是有意的 fail-fast。
  #[allow(clippy::expect_used)]
  let base_url =
    std::env::var("BASE_URL").expect("BASE_URL 未配置，请在环境变量或 .env 中设置 BASE_URL");
  let config = AuthConfig { base_url };
  let site_path = get_asset_root().join("site.json");
  // S10：mtime 缓存读取；返回需要 owned，clone 一次（配置体积小）。
  let site_config = site_path
    .to_str()
    .and_then(|p| SiteConfig::load_cached(p).ok())
    .map(|cfg| (*cfg).clone())
    .unwrap_or_default();
  let auth_service = AuthService::new(config, get_asset_root().join("plugins"));
  (auth_service, site_config)
}

#[cfg(feature = "server")]
fn find_plugin_filename(site_config: &SiteConfig, provider: &str) -> Option<String> {
  site_config.auth.providers.iter().find(|p| p.id == provider).map(|p| p.plugin.clone())
}

// ========== Auth 端点 ==========

#[post("/api/auth/providers")]
pub async fn get_auth_providers() -> Result<Vec<app_core::AuthProviderDisplay>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let (auth_service, site_config) = build_auth_service();
    Ok(auth_service.list_available_providers(&site_config).await)
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

/// Phase 7.2：准备一次 OAuth 登录，返回 (跳转 URL, 加密后的 cookie value)。
/// 仅供 axum 路由 `/api/auth/login/{provider}` 使用：路由把 cookie value
/// 套上 [`app_core::auth::build_pkce_set_cookie`] 后下发，再 302 到 url。
#[cfg(feature = "server")]
pub async fn prepare_login_for_provider(
  provider: String,
) -> Result<(String, String), app_core::error::AppError> {
  let (auth_service, site_config) = build_auth_service();
  let plugin_filename = find_plugin_filename(&site_config, &provider)
    .ok_or_else(|| format!("未在 site.json 中配置 provider: {}", provider))?;
  let (url, payload) = auth_service.prepare_login(&provider, &plugin_filename).await?;
  let cookie_value = payload.encode()?;
  Ok((url, cookie_value))
}

/// 内部 auth callback — 仅 server 端调用，返回 (welcome_message, jwt_token)。
/// Phase 7.2：state / verifier 来自浏览器加密 cookie（不再有进程内 HashMap）。
///
/// Phase 7.5 debug 习惯：`#[tracing::instrument]` 自动记录入口 + 退出 +
/// elapsed；`code` skip 以免 OAuth code 进日志，但 code_len 入字段供
/// 联调判定；`provider` / 解密后 cookie 内的 `pkce_cookie.provider`
/// 都已脱敏。
#[cfg(feature = "server")]
#[tracing::instrument(
  name = "server::auth_callback",
  level = "info",
  skip(code, pkce_cookie),
  fields(
    provider = %provider,
    code_len = code.len(),
    state_match = received_state == pkce_cookie.state,
  ),
  err
)]
pub async fn auth_callback_internal(
  code: String,
  provider: String,
  received_state: String,
  pkce_cookie: app_core::auth::PkceCookiePayload,
) -> Result<(String, String), app_core::error::AppError> {
  use app_core::db::get_or_init_pool;
  use app_core::session::create_jwt;

  let (auth_service, site_config) = build_auth_service();
  let plugin_filename = find_plugin_filename(&site_config, &provider)
    .ok_or_else(|| format!("未在 site.json 中配置 provider: {}", provider))?;

  tracing::debug!(
    provider = %provider,
    code_len = code.len(),
    state_len = received_state.len(),
    "auth callback received"
  );

  let db = get_or_init_pool().await?;

  let user = auth_service
    .handle_callback(&db, &provider, &plugin_filename, code, &received_state, pkce_cookie)
    .await?;

  let jwt_token = create_jwt(&user)?;
  tracing::info!(user = %user.nickname, "auth callback login success");
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
  {
    Ok(None)
  }
}

// ========== SEO ==========

/// Phase 2.3: 返回站点根 URL（含 scheme），给 `inject_seo`
/// 拼接 canonical URL 使用。服务端读 `BASE_URL` 环境变量；未
/// 设置时返回空串（不报错，由 inject_seo 降级使用相对路径）。
#[post("/api/seo/base-url")]
pub async fn get_seo_base_url() -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
    Ok(std::env::var("BASE_URL").unwrap_or_default())
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(String::new())
  }
}

// ========== 模块开关（Phase 3.4） ==========

/// Phase 3.4：返回当前启用的模块 id 列表。
///
/// `site.json::modules.<id>.enabled = false` 的模块不会出现在结果中。
/// 默认（未在 site.json 显式声明）模块按 [`default_module_specs`] 全开。
///
/// 用例：
/// - 前端 Navbar 决定是否渲染 blog / podcast / cases / forum 等链接
/// - 各页面（`ModuleGate`）根据列表判定是否显示“该模块已停用”占位
#[post("/api/modules/enabled")]
pub async fn enabled_module_ids() -> Result<Vec<String>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let engine = app_core::engines::module::default_module_engine();
    Ok(engine.enabled_ids())
  }
  #[cfg(not(feature = "server"))]
  {
    // 客户端 fallback：默认全开，避免 hydration 闪烁。
    Ok(vec![
      "blog".to_string(),
      "podcast".to_string(),
      "cases".to_string(),
      "forum".to_string(),
      "course".to_string(),
      "docs".to_string(),
    ])
  }
}

/// Phase 3.4：单模块开关查询。便于页面级 ModuleGate 调用。
#[post("/api/modules/is-enabled")]
pub async fn is_module_enabled(id: String) -> Result<bool, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let engine = app_core::engines::module::default_module_engine();
    Ok(engine.is_enabled(&id))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Ok(true)
  }
}

// ========== 布局 ==========

/// Phase 3.3：返回 `site.json::active_layout`（空字符串回退到 `"classic"`）。
#[post("/api/layout/active")]
pub async fn get_active_layout() -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
    // S10：mtime 缓存读取（每次布局渲染都会调）。
    let cfg =
      SiteConfig::load_cached(get_asset_root().join("site.json").to_str().unwrap_or_default())
        .unwrap_or_default();
    Ok(cfg.active_layout_or_default().to_string())
  }
  #[cfg(not(feature = "server"))]
  {
    Ok("classic".to_string())
  }
}

// ========== Echo ==========

#[post("/api/echo")]
pub async fn echo_server(input: String) -> Result<String, ServerFnError> {
  Ok(input)
}
