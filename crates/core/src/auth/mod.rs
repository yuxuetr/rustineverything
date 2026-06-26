#[cfg(feature = "server")]
pub mod crypto;

#[cfg(feature = "server")]
use crate::entities::{user, user_identity};
use crate::settings::SiteConfig;
#[cfg(feature = "server")]
use crate::PluginManager;
#[cfg(feature = "server")]
use chrono::Utc;
use sdk::{AuthProviderConfig, AuthProviderDisplay, StandardUser};
#[cfg(feature = "server")]
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
#[cfg(feature = "server")]
use std::time::{SystemTime, UNIX_EPOCH};

/// PKCE / state 条目的过期时间（5 分钟）。也是 oauth_pkce cookie 的 Max-Age。
#[cfg(feature = "server")]
pub const PKCE_TTL_SECS: u64 = 5 * 60;

/// 加密 OAuth 授权流程临时凭据的 cookie 名（Phase 7.2）。
#[cfg(feature = "server")]
pub const PKCE_COOKIE_NAME: &str = "oauth_pkce";

/// Cookie 限定路径：只对 `/api/auth` 子路径回传。
#[cfg(feature = "server")]
pub const PKCE_COOKIE_PATH: &str = "/api/auth";

/// OAuth 授权流程的临时凭据（Phase 7.2 持久化方案）。
///
/// 通过 AES-256-GCM 加密成 cookie value 后由浏览器在 OAuth 重定向间携带，
/// 替代 Phase 1A.4 的进程内 `HashMap`，从而支持多副本部署：
/// 即使 login 与 callback 落在不同后端实例上，state / verifier 仍由
/// 浏览器 cookie 同步过来，服务端无需共享存储。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PkceCookiePayload {
  /// OAuth provider id（如 `github` / `google`）。用于 callback 时校验
  /// URL 中的 `<provider>` 段与 cookie 携带的一致。
  pub provider: String,
  /// 32 字符随机 state，用于 CSRF：必须与回调 URL 的 `state` query 完全相等。
  pub state: String,
  /// 仅在 provider 启用 PKCE 时存在；64 字符 code_verifier 原文，
  /// 作为 token exchange 请求的 `code_verifier` 字段。
  pub verifier: Option<String>,
  /// 签发时的 UNIX 时间戳（秒）。callback 端用它 + [`PKCE_TTL_SECS`] 判定过期。
  pub issued_at_secs: u64,
}

#[cfg(feature = "server")]
impl PkceCookiePayload {
  pub fn new_now(provider: String, state: String, verifier: Option<String>) -> Self {
    let issued_at_secs =
      SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Self { provider, state, verifier, issued_at_secs }
  }

  /// JSON → AES-256-GCM 加密 → base64url(no-pad)，作为 cookie value。
  pub fn encode(&self) -> Result<String, String> {
    let json =
      serde_json::to_string(self).map_err(|e| format!("PKCE payload 序列化失败: {}", e))?;
    crypto::encrypt_token(&json)
  }

  /// base64url(no-pad) → AES-256-GCM 解密 → JSON。
  pub fn decode(cipher: &str) -> Result<Self, String> {
    let json = crypto::decrypt_token(cipher)?;
    serde_json::from_str(&json).map_err(|e| format!("PKCE payload 反序列化失败: {}", e))
  }

  /// 是否已超过 [`PKCE_TTL_SECS`]。系统时钟倒退视作未过期（保守）。
  pub fn is_expired(&self) -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    now.saturating_sub(self.issued_at_secs) > PKCE_TTL_SECS
  }
}

/// 构造 `Set-Cookie` 头值，用于 OAuth 重定向时下发加密 PKCE payload。
///
/// 属性：
/// - `HttpOnly`：禁止 JS 读取。
/// - `Secure`：当 `secure=true`（即 `BASE_URL` 是 https）时附加。
/// - `SameSite=Lax`：cross-site top-level GET 仍会发送 → callback 拿得到。
/// - `Path=/api/auth`：限定回传路径。
/// - `Max-Age=PKCE_TTL_SECS`：与 PKCE TTL 对齐。
#[cfg(feature = "server")]
pub fn build_pkce_set_cookie(value: &str, secure: bool) -> String {
  let secure_flag = if secure { "; Secure" } else { "" };
  format!(
    "{}={}; HttpOnly; Path={}; Max-Age={}; SameSite=Lax{}",
    PKCE_COOKIE_NAME, value, PKCE_COOKIE_PATH, PKCE_TTL_SECS, secure_flag
  )
}

/// 构造清空 cookie 的 `Set-Cookie` 头值（callback 成功 / 失败后都应清掉，
/// 避免遗留可重放的 verifier）。
#[cfg(feature = "server")]
pub fn build_pkce_clear_cookie(secure: bool) -> String {
  let secure_flag = if secure { "; Secure" } else { "" };
  format!(
    "{}=; HttpOnly; Path={}; Max-Age=0; SameSite=Lax{}",
    PKCE_COOKIE_NAME, PKCE_COOKIE_PATH, secure_flag
  )
}

/// 从 `Cookie` 请求头原文中抽出 [`PKCE_COOKIE_NAME`] 的值。
///
/// 简易解析：按 `;` 分段，找首个 `key=value` 命中。允许 key 前后空白。
/// 不做百分号解码（cookie value 是 base64url，已无需解码）。
#[cfg(feature = "server")]
pub fn extract_pkce_cookie(cookie_header: &str) -> Option<String> {
  for segment in cookie_header.split(';') {
    let trimmed = segment.trim();
    if let Some((k, v)) = trimmed.split_once('=') {
      if k.trim() == PKCE_COOKIE_NAME {
        return Some(v.trim().to_string());
      }
    }
  }
  None
}

/// 动态认证配置，不再硬编码单个 provider
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
  pub base_url: String, // 如 "http://localhost:8080"
}

impl AuthConfig {
  /// 从环境变量动态读取 provider 的 client_id 和 client_secret
  /// 约定：{PROVIDER}_CLIENT_ID, {PROVIDER}_CLIENT_SECRET (全大写)
  #[cfg(feature = "server")]
  pub fn get_credentials(provider: &str) -> crate::error::AppResult<(String, String)> {
    let upper = provider.to_uppercase();
    let client_id = std::env::var(format!("{}_CLIENT_ID", upper))
      .map_err(|_| format!("未配置环境变量: {}_CLIENT_ID", upper))?;
    let client_secret = std::env::var(format!("{}_CLIENT_SECRET", upper))
      .map_err(|_| format!("未配置环境变量: {}_CLIENT_SECRET", upper))?;
    Ok((client_id, client_secret))
  }

  /// 检查 provider 是否已配置凭据
  #[cfg(feature = "server")]
  pub fn has_credentials(provider: &str) -> bool {
    let upper = provider.to_uppercase();
    std::env::var(format!("{}_CLIENT_ID", upper)).is_ok()
      && std::env::var(format!("{}_CLIENT_SECRET", upper)).is_ok()
  }

  /// 构建回调 URL
  pub fn redirect_url(&self, provider: &str) -> String {
    format!("{}/api/auth/callback/{}", self.base_url, provider)
  }
}

#[cfg(feature = "server")]
pub struct AuthService {
  pub config: AuthConfig,
  pub plugin_manager: PluginManager,
  pub plugin_dir: PathBuf,
}

#[cfg(feature = "server")]
impl AuthService {
  pub fn new(config: AuthConfig, plugin_dir: PathBuf) -> Self {
    Self { config, plugin_manager: PluginManager::new(), plugin_dir }
  }

  /// 根据 site.json 配置，返回已安装且已配置凭据的 auth provider 展示列表。
  ///
  /// 内部需要进入 wasmi 沙箱调用 `get_display_info`，因此整个函数是 async。
  pub async fn list_available_providers(
    &self,
    site_config: &SiteConfig,
  ) -> Vec<AuthProviderDisplay> {
    if !site_config.auth.enabled {
      return vec![];
    }

    let mut result = Vec::new();
    for entry in &site_config.auth.providers {
      let plugin_path = self.plugin_dir.join(&entry.plugin);

      // 插件文件必须存在
      if !plugin_path.exists() {
        tracing::warn!(plugin = ?plugin_path, "auth: plugin file missing, skipping");
        continue;
      }

      // 环境变量必须配置
      if !AuthConfig::has_credentials(&entry.id) {
        tracing::warn!(provider = %entry.id, "auth: no credentials configured, skipping");
        continue;
      }

      // 调用插件获取展示信息
      match std::fs::read(&plugin_path) {
        Ok(wasm_bytes) => {
          match self.plugin_manager.call_with_string(&wasm_bytes, "get_display_info", "").await {
            Ok(json) => match serde_json::from_str::<AuthProviderDisplay>(&json) {
              Ok(display) => result.push(display),
              Err(e) => {
                tracing::warn!(provider = %entry.id, error = %e, "auth: failed to parse display_info")
              }
            },
            Err(e) => {
              tracing::warn!(provider = %entry.id, error = %e, "auth: get_display_info call failed")
            }
          }
        }
        Err(e) => tracing::warn!(provider = %entry.id, error = %e, "auth: failed to read plugin"),
      }
    }

    result
  }

  /// 准备 OAuth 登录：返回 (跳转 URL, [`PkceCookiePayload`])。
  ///
  /// 调用方负责把 payload 调 [`PkceCookiePayload::encode`] 后塞进 `Set-Cookie`
  /// （见 [`build_pkce_set_cookie`]）。Phase 7.2：服务端不再保存任何 state /
  /// verifier，全部由浏览器 cookie 承担。
  pub async fn prepare_login(
    &self,
    provider: &str,
    plugin_filename: &str,
  ) -> crate::error::AppResult<(String, PkceCookiePayload)> {
    let plugin_path = self.plugin_dir.join(plugin_filename);
    if !plugin_path.exists() {
      return Err(format!("未找到插件: {:?}", plugin_path).into());
    }

    let wasm_bytes = std::fs::read(plugin_path)?;
    let config_json =
      self.plugin_manager.call_with_string(&wasm_bytes, "get_provider_config", "").await?;
    let provider_config: AuthProviderConfig = serde_json::from_str(&config_json)?;

    let (client_id, _) = AuthConfig::get_credentials(provider)?;
    let redirect_url = self.config.redirect_url(provider);
    let scopes = provider_config.scopes.join(" ");

    // 生成随机 state
    //
    // Phase 8.2：state / code_verifier 都是 CSRF / PKCE 防御的核心熵源，必须用
    // CSPRNG。`rand::rngs::OsRng` 直读 OS 熵池（`getrandom(2)` 等），相比 `rand::rng()`
    // 的 `ThreadRng` 更稳定且不会被替换为 `SmallRng` 等弱 RNG。
    //
    // **不要**改成 `rand::rng()` / `SmallRng` —— 任何 PRNG 都可能被预测出 state，
    // 导致 OAuth 回调被伪造。OsRng 是 `TryRngCore`；用 `unwrap_err()` 包成
    // `RngCore`（OS 熵失败极罕见，发生时 panic 比签发可预测 state 更安全）。
    use rand::{rngs::OsRng, Rng, TryRngCore};
    let state: String = OsRng
      .unwrap_err()
      .sample_iter(&rand::distr::Alphanumeric)
      .take(32)
      .map(|b| b as char)
      .collect();

    let mut url = format!(
      "{}?client_id={}&redirect_uri={}&scope={}&response_type=code&state={}",
      provider_config.auth_url, client_id, redirect_url, scopes, state
    );

    // PKCE: 生成 code_verifier 和 code_challenge
    let verifier = if provider_config.requires_pkce {
      use base64::Engine;
      use sha2::Digest;

      // 同上 state：必须用 OsRng 而非 ThreadRng；code_verifier 被预测会让 PKCE 退化为无防。
      let code_verifier: String = OsRng
        .unwrap_err()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(64)
        .map(|b| b as char)
        .collect();

      let digest = sha2::Sha256::digest(code_verifier.as_bytes());
      let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
      url.push_str(&format!("&code_challenge={}&code_challenge_method=S256", code_challenge));

      tracing::debug!(provider = %provider, "auth: PKCE enabled");
      Some(code_verifier)
    } else {
      None
    };

    let payload = PkceCookiePayload::new_now(provider.to_string(), state, verifier);
    Ok((url, payload))
  }

  /// 在 cookie 解出 [`PkceCookiePayload`] 之后执行的回调主体。
  ///
  /// 调用方先从 `Cookie` 头解密出 payload；本函数负责所有业务校验：
  /// - state CSRF: `payload.state == received_state`
  /// - provider 绑定: `payload.provider == provider`
  /// - TTL: `!payload.is_expired()`
  /// - 若插件要求 PKCE：`payload.verifier.is_some()`
  pub async fn handle_callback(
    &self,
    db: &DatabaseConnection,
    provider: &str,
    plugin_filename: &str,
    code: String,
    received_state: &str,
    pkce_cookie: PkceCookiePayload,
  ) -> crate::error::AppResult<user::Model> {
    // 0. CSRF + 绑定 + TTL 校验（任何一项失败都视为不合法授权流）
    if pkce_cookie.is_expired() {
      return Err("OAuth 会话已过期，请重新登录".into());
    }
    if pkce_cookie.provider != provider {
      return Err("OAuth 会话 provider 不匹配".into());
    }
    if pkce_cookie.state != received_state {
      return Err("OAuth state 不匹配（疑似 CSRF）".into());
    }

    let plugin_path = self.plugin_dir.join(plugin_filename);
    let wasm_bytes = std::fs::read(&plugin_path)?;

    // 1. 获取插件配置
    let config_json =
      self.plugin_manager.call_with_string(&wasm_bytes, "get_provider_config", "").await?;
    let provider_config: AuthProviderConfig = serde_json::from_str(&config_json)?;

    let (client_id, client_secret) = AuthConfig::get_credentials(provider)?;
    let redirect_url = self.config.redirect_url(provider);

    // 2. 构建 Token 交换请求
    let http_client = reqwest::Client::new();
    tracing::debug!(
        provider = %provider,
        auth_method = %provider_config.token_auth_method,
        "auth: token exchange initiated"
    );

    let mut form_params: Vec<(&str, String)> = vec![
      ("code", code),
      ("redirect_uri", redirect_url),
      ("grant_type", "authorization_code".to_string()),
    ];

    // PKCE: 若插件要求 PKCE 但 cookie 里没有 verifier → 拒绝
    if provider_config.requires_pkce && pkce_cookie.verifier.is_none() {
      return Err("PKCE code_verifier 缺失".into());
    }
    if let Some(ref cv) = pkce_cookie.verifier {
      form_params.push(("code_verifier", cv.clone()));
      tracing::debug!("auth: PKCE code_verifier attached");
    }

    // 根据认证方式构建请求
    let request = if provider_config.token_auth_method == "basic_auth" {
      // Basic Auth: client_id:client_secret in Authorization header
      http_client
        .post(&provider_config.token_url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .basic_auth(&client_id, Some(&client_secret))
        .form(&form_params)
    } else {
      // Form body (default): client_id/secret in form data
      form_params.push(("client_id", client_id.clone()));
      form_params.push(("client_secret", client_secret.clone()));
      http_client
        .post(&provider_config.token_url)
        .header("Accept", "application/json")
        .form(&form_params)
    };

    let token_response: Value = request.send().await?.json().await?;
    // 安全：不输出完整 token 响应，仅记录是否拿到 access_token
    let access_token = token_response["access_token"].as_str().ok_or_else(|| {
      let err_kind = token_response["error"].as_str().unwrap_or("unknown_error");
      format!("Token 交换失败 (provider={}, error={})", provider, err_kind)
    })?;
    tracing::info!(provider = %provider, "auth: token exchange success");

    // 3. 获取用户信息
    let profile_response: Value = http_client
      .get(&provider_config.profile_url)
      .header("Authorization", format!("Bearer {}", access_token))
      .header("User-Agent", "app")
      .send()
      .await?
      .json()
      .await?;
    tracing::info!(provider = %provider, "auth: profile fetched");

    // 4. 插件 Profile 映射
    let standard_user_json = self
      .plugin_manager
      .call_with_string(&wasm_bytes, "map_profile", &profile_response.to_string())
      .await?;
    let standard_user: StandardUser = serde_json::from_str(&standard_user_json)?;

    // 5. 同步至数据库（Phase 8.2：不再持久化 access_token，直接丢弃）
    let _ = access_token; // 调用链结束于此；进入 sync_user_to_db 之后不再需要
    self
      .sync_user_to_db(
        db,
        provider,
        standard_user.external_id,
        standard_user.nickname,
        standard_user.avatar_url,
      )
      .await
  }

  async fn sync_user_to_db(
    &self,
    db: &DatabaseConnection,
    provider: &str,
    uid: String,
    nickname: String,
    avatar_url: Option<String>,
  ) -> crate::error::AppResult<user::Model> {
    let identity = user_identity::Entity::find()
      .filter(user_identity::Column::Provider.eq(provider))
      .filter(user_identity::Column::ProviderUid.eq(&uid))
      .one(db)
      .await?;

    if let Some(ident) = identity {
      let user = user::Entity::find_by_id(ident.user_id).one(db).await?.ok_or("找不到关联用户")?;
      Ok(user)
    } else {
      // 事务包裹：user 与 user_identity 要么同时成功要么同时回滚，避免孤儿 user
      let txn = db.begin().await?;

      let new_user = user::ActiveModel {
        nickname: Set(nickname),
        avatar_url: Set(avatar_url),
        role: Set("member".to_string()),
        created_at: Set(Utc::now().fixed_offset()),
        updated_at: Set(Utc::now().fixed_offset()),
        ..Default::default()
      };
      let user_res = user::Entity::insert(new_user).exec(&txn).await?;

      let new_ident = user_identity::ActiveModel {
        user_id: Set(user_res.last_insert_id),
        provider: Set(provider.to_string()),
        provider_uid: Set(uid),
        created_at: Set(Utc::now().fixed_offset()),
        ..Default::default()
      };
      user_identity::Entity::insert(new_ident).exec(&txn).await?;

      let user_final = user::Entity::find_by_id(user_res.last_insert_id)
        .one(&txn)
        .await?
        .ok_or("无法获取新用户")?;

      txn.commit().await?;
      Ok(user_final)
    }
  }
}

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;
  use std::fs;

  #[tokio::test]
  async fn test_github_auth_plugin_logic() {
    // 插件路径（基于 target-dir 的位置或 build 后的位置）
    let wasm_path = "../../assets/plugins/github_auth_plugin.wasm";
    if !std::path::Path::new(wasm_path).exists() {
      println!("跳过测试：插件文件不存在");
      return;
    }

    let wasm_bytes = fs::read(wasm_path).expect("读取插件失败");
    let manager = crate::PluginManager::new();

    // 1. 测试获取配置
    let config_json = manager
      .call_with_string(&wasm_bytes, "get_provider_config", "")
      .await
      .expect("调用 get_provider_config 失败");
    let config: AuthProviderConfig =
      serde_json::from_str(&config_json).expect("解析配置 JSON 失败");
    assert_eq!(config.auth_url, "https://github.com/login/oauth/authorize");
    assert!(config.scopes.contains(&"read:user".to_string()));

    // 2. 测试 Profile 映射
    let mock_raw_profile = serde_json::json!({
        "id": 12345,
        "login": "test_user",
        "avatar_url": "https://example.com/avatar.png",
        "email": "test@example.com"
    })
    .to_string();

    let standard_user_json = manager
      .call_with_string(&wasm_bytes, "map_profile", &mock_raw_profile)
      .await
      .expect("调用 map_profile 失败");
    let user: StandardUser =
      serde_json::from_str(&standard_user_json).expect("解析 StandardUser 失败");

    assert_eq!(user.external_id, "12345");
    assert_eq!(user.nickname, "test_user");
    assert_eq!(user.provider, "github");
    assert_eq!(user.email, Some("test@example.com".to_string()));
  }

  // ── Phase 7.2：PkceCookiePayload 加密 cookie 持久化 ──

  fn ensure_jwt_secret_for_pkce_tests() {
    if std::env::var("JWT_SECRET").is_err() {
      // SAFETY: 单测环境单线程下设置环境变量；与 crypto.rs 测试同套路。
      unsafe {
        std::env::set_var("JWT_SECRET", "test-secret-for-pkce-cookie-tests-1234");
      }
    }
  }

  fn payload_now(verifier: Option<&str>) -> PkceCookiePayload {
    PkceCookiePayload::new_now(
      "github".to_string(),
      "state-abc-32".to_string(),
      verifier.map(|s| s.to_string()),
    )
  }

  #[test]
  fn pkce_cookie_round_trip_preserves_payload() {
    ensure_jwt_secret_for_pkce_tests();
    let p = payload_now(Some("verifier-xyz-64"));
    let cipher = p.encode().expect("encode");
    let back = PkceCookiePayload::decode(&cipher).expect("decode");
    assert_eq!(back, p);
  }

  #[test]
  fn pkce_cookie_round_trip_without_verifier() {
    ensure_jwt_secret_for_pkce_tests();
    let p = payload_now(None);
    let cipher = p.encode().expect("encode");
    let back = PkceCookiePayload::decode(&cipher).expect("decode");
    assert_eq!(back.verifier, None);
    assert_eq!(back.provider, "github");
  }

  #[test]
  fn pkce_cookie_decode_rejects_tampered_cipher() {
    ensure_jwt_secret_for_pkce_tests();
    let p = payload_now(Some("v"));
    let mut cipher = p.encode().expect("encode");
    // 翻转尾部 1 字符以触发 GCM tag 校验失败
    let last = cipher.pop().expect("non-empty");
    let flipped = if last == 'a' { 'b' } else { 'a' };
    cipher.push(flipped);
    assert!(PkceCookiePayload::decode(&cipher).is_err());
  }

  #[test]
  fn pkce_cookie_decode_rejects_random_garbage() {
    ensure_jwt_secret_for_pkce_tests();
    assert!(PkceCookiePayload::decode("not-a-real-ciphertext").is_err());
    assert!(PkceCookiePayload::decode("").is_err());
  }

  #[test]
  fn pkce_cookie_is_expired_at_ttl_boundary() {
    let mut p = payload_now(None);
    // 模拟很久以前签发
    p.issued_at_secs =
      SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (PKCE_TTL_SECS + 10);
    assert!(p.is_expired(), "超出 TTL 应判为过期");

    p.issued_at_secs =
      SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (PKCE_TTL_SECS - 10);
    assert!(!p.is_expired(), "TTL 内不应过期");
  }

  #[test]
  fn build_pkce_set_cookie_attaches_secure_when_https() {
    let s = build_pkce_set_cookie("ABC", true);
    assert!(s.starts_with("oauth_pkce=ABC;"));
    assert!(s.contains("HttpOnly"));
    assert!(s.contains("Path=/api/auth"));
    assert!(s.contains("SameSite=Lax"));
    assert!(s.contains("Max-Age=300"));
    assert!(s.contains("Secure"));
  }

  #[test]
  fn build_pkce_set_cookie_omits_secure_when_http() {
    let s = build_pkce_set_cookie("ABC", false);
    assert!(!s.contains("Secure"));
  }

  #[test]
  fn build_pkce_clear_cookie_uses_zero_max_age() {
    let s = build_pkce_clear_cookie(true);
    assert!(s.starts_with("oauth_pkce=;"));
    assert!(s.contains("Max-Age=0"));
    assert!(s.contains("Secure"));
  }

  #[test]
  fn extract_pkce_cookie_finds_value_among_other_cookies() {
    let header = "theme=dark; oauth_pkce=ENCRYPTEDVAL; session=abc";
    assert_eq!(extract_pkce_cookie(header).as_deref(), Some("ENCRYPTEDVAL"));
  }

  #[test]
  fn extract_pkce_cookie_returns_none_when_absent() {
    let header = "theme=dark; session=abc";
    assert_eq!(extract_pkce_cookie(header), None);
  }

  #[test]
  fn extract_pkce_cookie_handles_whitespace_around_pairs() {
    let header = " theme=dark ;  oauth_pkce=VAL  ; session=abc ";
    assert_eq!(extract_pkce_cookie(header).as_deref(), Some("VAL"));
  }

  // ── Live-DB 集成测试：验证 sync_user_to_db 的事务回滚 ──
  //
  // 默认 ignored；需一个可写的测试库（建议一次性 docker postgres，勿指向生产库）：
  //   DATABASE_URL=postgres://... \
  //     cargo test --features server -p app-core -- --ignored \
  //     sync_user_to_db_rolls_back
  //
  // 用例利用「provider_uid 超出 varchar(255)」制造确定性的事务中途失败：
  // find() 用 300 字符 uid 命中不到（返回 None）→ 进入插入分支 → user 先插入
  // 成功 → identity 因 "value too long" 插入失败 → `?` 传播 → 事务被 Drop（回滚）。
  // 若回滚正确，则不应残留以该唯一 nickname 创建的孤儿 user。
  #[tokio::test]
  #[ignore = "Live DB. Run with --ignored after setting DATABASE_URL (use a throwaway postgres)"]
  async fn sync_user_to_db_rolls_back_on_identity_insert_failure() {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::Database;

    let Ok(db_url) = std::env::var("DATABASE_URL") else {
      eprintln!("跳过 rollback live test：DATABASE_URL 未配置");
      return;
    };
    // Phase 8.2：sync_user_to_db 不再加密 access_token（列已删），但加 JWT_SECRET
    // 让 verify_jwt 等代码路径不 panic。
    std::env::set_var("JWT_SECRET", "test-secret-for-rollback-tests-1234");
    let db = Database::connect(&db_url).await.expect("连接测试数据库失败");
    Migrator::up(&db, None).await.expect("应用迁移失败");

    let service = AuthService::new(
      AuthConfig { base_url: "http://localhost:8080".to_string() },
      std::path::PathBuf::from("."),
    );

    // 唯一标记：避免与既有数据冲突，并能精确断言/清理
    let marker = Utc::now().timestamp_micros();
    let nickname = format!("rollback-orphan-{}", marker);

    // 负向：identity 插入必失败（uid 超长），事务必须回滚
    let over_long_uid = "u".repeat(300);
    let failed = service
      .sync_user_to_db(&db, "rollback_test_provider", over_long_uid, nickname.clone(), None)
      .await;
    assert!(failed.is_err(), "超长 provider_uid 的 identity 插入应当失败");

    let orphans = user::Entity::find()
      .filter(user::Column::Nickname.eq(&nickname))
      .all(&db)
      .await
      .expect("查询 user 失败");
    assert!(orphans.is_empty(), "事务应回滚：不应残留孤儿 user，实际残留 {} 行", orphans.len());

    // 正向对照：正常 uid 应成功创建 user+identity，证明失败仅源于约束而非环境
    let ok_uid = format!("uid-{}", marker);
    let ok_nick = format!("rollback-ok-{}", marker);
    let created = service
      .sync_user_to_db(&db, "rollback_test_provider", ok_uid, ok_nick, None)
      .await
      .expect("正常 uid 应成功创建用户");

    // 清理：删 user 即可级联删 identity（FK ON DELETE CASCADE）
    user::Entity::delete_by_id(created.id).exec(&db).await.expect("清理测试 user 失败");
  }
}
