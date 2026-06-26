use serde::{Deserialize, Serialize};

/// 已知角色取值（与 `init.sql` 默认值保持一致）
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_MEMBER: &str = "member";
pub const ROLE_GUEST: &str = "guest";

/// 系统支持的全部角色（顺序与 UI 下拉一致）
pub const ALL_ROLES: &[&str] = &[ROLE_ADMIN, ROLE_MEMBER, ROLE_GUEST];

/// 校验任意字符串是否是合法的角色取值
pub fn is_known_role(role: &str) -> bool {
  ALL_ROLES.contains(&role)
}

/// 会话用户信息，前后端共享
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUser {
  pub id: i32,
  pub nickname: String,
  pub avatar_url: Option<String>,
  pub role: String,
}

impl SessionUser {
  pub fn is_admin(&self) -> bool {
    self.role == ROLE_ADMIN
  }
}

// ---- 以下为 server-only JWT 工具 ----

#[cfg(feature = "server")]
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
  sub: i32,
  nickname: String,
  avatar_url: Option<String>,
  role: String,
  exp: usize,
}

/// 取出 JWT_SECRET。
/// **安全策略**：未配置 / 空 / 命中 placeholder 子串均直接 panic，避免生产环境
/// 误用默认值。启动时（如 `init_pool` 之后）应主动调用一次以早失败。
#[cfg(feature = "server")]
pub fn get_jwt_secret() -> String {
  match std::env::var("JWT_SECRET") {
    Ok(secret) if !secret.is_empty() => {
      assert_not_placeholder("JWT_SECRET", &secret);
      secret
    }
    Ok(_) => panic!("JWT_SECRET 不能为空，请在环境变量或 .env 中配置（建议 32+ 字符随机字符串）"),
    Err(_) => panic!("JWT_SECRET 未配置，请在环境变量或 .env 中设置 JWT_SECRET"),
  }
}

/// 已知 placeholder 模板片段（来自 `.env.example` 等）。命中即视为未配置真实值。
///
/// 模式选择原则：足够具体以避免误伤真实凭据（例如不直接拒绝 `password` 子串，
/// 因为用户的真实密码完全可能包含 `password` 词根），同时覆盖最常见的占位串。
#[cfg(feature = "server")]
const KNOWN_PLACEHOLDER_FRAGMENTS: &[&str] =
  &["change-me", "changeme", "your-", "<your", "replace-me", "placeholder"];

/// 在配置值里搜索任意已知 placeholder 片段（大小写不敏感）。
///
/// 仅做子串匹配；不强制 schema。允许例如 `BASE_URL=http://127.0.0.1:8080` 这类
/// 本地开发值通过——它们不在 placeholder 列表里。
#[cfg(feature = "server")]
pub fn looks_like_placeholder(value: &str) -> bool {
  let lower = value.to_ascii_lowercase();
  KNOWN_PLACEHOLDER_FRAGMENTS.iter().any(|frag| lower.contains(frag))
}

/// 命中 placeholder 时立即 panic；附带变量名提示运维该改哪个 env。
#[cfg(feature = "server")]
pub fn assert_not_placeholder(env_var_name: &str, value: &str) {
  if looks_like_placeholder(value) {
    panic!(
      "{} 仍是 .env.example 的占位值（命中 placeholder 子串），请替换为真实凭据后再启动",
      env_var_name
    );
  }
}

/// 根据用户模型签发 JWT
#[cfg(feature = "server")]
pub fn create_jwt(user: &crate::entities::user::Model) -> crate::error::AppResult<String> {
  use crate::error::AppError;
  use jsonwebtoken::{encode, EncodingKey, Header};

  let expiration = chrono::Utc::now()
    .checked_add_signed(chrono::Duration::days(7))
    .ok_or_else(|| AppError::auth("计算过期时间失败"))?
    .timestamp() as usize;

  let claims = Claims {
    sub: user.id,
    nickname: user.nickname.clone(),
    avatar_url: user.avatar_url.clone(),
    role: user.role.clone(),
    exp: expiration,
  };

  let token =
    encode(&Header::default(), &claims, &EncodingKey::from_secret(get_jwt_secret().as_bytes()))
      .map_err(|e| AppError::auth(format!("JWT 签发失败: {}", e)))?;

  Ok(token)
}

/// 验证 JWT 并返回 SessionUser
#[cfg(feature = "server")]
pub fn verify_jwt(token: &str) -> crate::error::AppResult<SessionUser> {
  use crate::error::AppError;
  use jsonwebtoken::{decode, DecodingKey, Validation};

  let token_data = decode::<Claims>(
    token,
    &DecodingKey::from_secret(get_jwt_secret().as_bytes()),
    &Validation::default(),
  )
  .map_err(|e| AppError::auth(format!("JWT 校验失败: {}", e)))?;

  Ok(SessionUser {
    id: token_data.claims.sub,
    nickname: token_data.claims.nickname,
    avatar_url: token_data.claims.avatar_url,
    role: token_data.claims.role,
  })
}

/// 从 Cookie 头字符串中提取 session token
#[cfg(feature = "server")]
pub fn extract_session_cookie(cookie_header: &str) -> Option<String> {
  for pair in cookie_header.split(';') {
    let pair = pair.trim();
    if let Some(value) = pair.strip_prefix("session=") {
      if !value.is_empty() {
        return Some(value.to_string());
      }
    }
  }
  None
}

/// 从 Cookie 头解析当前登录用户
#[cfg(feature = "server")]
pub fn parse_session_from_cookie_header(cookie_header: Option<&str>) -> Option<SessionUser> {
  let cookie_str = cookie_header?;
  let token = extract_session_cookie(cookie_str)?;
  verify_jwt(&token).ok()
}

/// 从当前 Dioxus FullstackContext 的 Cookie 中解析 SessionUser。
/// 上层 server fn 可统一调用此函数，避免每个模块复制 cookie 解析逻辑。
#[cfg(feature = "server")]
pub fn current_session_user() -> Option<SessionUser> {
  use dioxus::fullstack::FullstackContext;

  let ctx = FullstackContext::current()?;
  let parts = ctx.parts_mut();
  let cookie_str = parts.headers.get("cookie").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
  drop(parts);
  parse_session_from_cookie_header(cookie_str.as_deref())
}

/// 要求当前请求带有合法 SessionUser；否则返回中文错误。
#[cfg(feature = "server")]
pub fn require_session() -> Result<SessionUser, dioxus::fullstack::ServerFnError> {
  current_session_user()
    .ok_or_else(|| dioxus::fullstack::ServerFnError::new("请先登录".to_string()))
}

/// 要求当前请求带有 admin 角色；非 admin 返回 403 风格错误。
///
/// Phase 8.6：除了校验 JWT 内的 role 字段，再追加一次 DB 实时回查。
/// 原因：JWT 默认 7 天有效；用户在 admin UI 被降级 / 删除后，缓存在浏览器
/// cookie 里的旧 JWT 仍会被当作 admin 接受最长 7 天。回查 `users.role`
/// 让降级 / 软删除 5 秒内（pool acquire 时间）生效。
///
/// 性能权衡：每次 admin 请求多 1 个 DB round-trip；admin 流量天然低
/// （站点作者维护界面），不会对常规读路径造成影响。
///
/// 失败语义：DB 不可达 / 用户已删 → 拒绝（fail-closed）。
#[cfg(feature = "server")]
pub async fn require_admin() -> Result<SessionUser, dioxus::fullstack::ServerFnError> {
  use crate::db::get_or_init_pool;
  use crate::entities::user;
  use dioxus::fullstack::ServerFnError;
  use sea_orm::EntityTrait;

  let user_from_jwt = require_session()?;
  if !user_from_jwt.is_admin() {
    return Err(ServerFnError::new("需要管理员权限".to_string()));
  }

  // DB recheck：fail-closed
  let db = get_or_init_pool()
    .await
    .map_err(|e| ServerFnError::new(format!("admin 权限校验失败（DB 不可达）: {}", e)))?;
  let db_user = user::Entity::find_by_id(user_from_jwt.id)
    .one(&db)
    .await
    .map_err(|e| ServerFnError::new(format!("admin 权限校验失败: {}", e)))?
    .ok_or_else(|| ServerFnError::new("用户已不存在或已被删除".to_string()))?;
  if db_user.role != ROLE_ADMIN {
    return Err(ServerFnError::new(format!("管理员权限已撤销（当前角色: {}）", db_user.role)));
  }
  Ok(user_from_jwt)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn user_with_role(role: &str) -> SessionUser {
    SessionUser { id: 1, nickname: "tester".to_string(), avatar_url: None, role: role.to_string() }
  }

  #[test]
  fn role_constants_are_known() {
    for r in ALL_ROLES {
      assert!(is_known_role(r));
    }
  }

  #[test]
  fn unknown_role_rejected() {
    assert!(!is_known_role(""));
    assert!(!is_known_role("super"));
    assert!(!is_known_role("Admin")); // 大小写敏感
  }

  #[test]
  fn is_admin_flag() {
    assert!(user_with_role(ROLE_ADMIN).is_admin());
    assert!(!user_with_role(ROLE_MEMBER).is_admin());
    assert!(!user_with_role(ROLE_GUEST).is_admin());
    assert!(!user_with_role("unknown").is_admin());
  }

  #[cfg(feature = "server")]
  #[test]
  fn placeholder_detector_catches_known_templates() {
    assert!(super::looks_like_placeholder("change-me-to-a-32-char-random-string"));
    assert!(super::looks_like_placeholder("Change-Me-Now")); // 大小写不敏感
    assert!(super::looks_like_placeholder("changeme123"));
    assert!(super::looks_like_placeholder("your-domain.example.com"));
    assert!(super::looks_like_placeholder("REPLACE-ME"));
    assert!(super::looks_like_placeholder("<your-token-here>"));
    assert!(super::looks_like_placeholder("just-a-placeholder"));
  }

  #[cfg(feature = "server")]
  #[test]
  fn placeholder_detector_passes_realistic_values() {
    // 真实 32+ 字节随机串
    assert!(!super::looks_like_placeholder("aE82kx9p1QrW7nZv4BcdLmTfHsJyU0qZ"));
    // 本地开发 BASE_URL
    assert!(!super::looks_like_placeholder("http://127.0.0.1:8080"));
    assert!(!super::looks_like_placeholder("https://blog.example.org"));
    // 真实密码可能包含 password 词根 → 不应误伤
    assert!(!super::looks_like_placeholder("Sup3r!StrongPa55word2026"));
  }
}
