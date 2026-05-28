use serde::{Deserialize, Serialize};

/// 已知角色取值（与 `init.sql` 默认值保持一致）
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_MEMBER: &str = "member";
pub const ROLE_GUEST: &str = "guest";

/// 系统支持的全部角色（顺序与 UI 下拉一致）
pub const ALL_ROLES: &[&str] = &[ROLE_ADMIN, ROLE_MEMBER, ROLE_GUEST];

/// 校验任意字符串是否是合法的角色取值
pub fn is_known_role(role: &str) -> bool {
  ALL_ROLES.iter().any(|r| *r == role)
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
/// **安全策略**：未配置直接 panic，避免在生产环境中误用默认值。
/// 启动时（如 `init_pool` 之后）应主动调用一次以早失败。
#[cfg(feature = "server")]
pub fn get_jwt_secret() -> String {
  match std::env::var("JWT_SECRET") {
    Ok(secret) if !secret.is_empty() => secret,
    Ok(_) => panic!("JWT_SECRET 不能为空，请在环境变量或 .env 中配置（建议 32+ 字符随机字符串）"),
    Err(_) => panic!("JWT_SECRET 未配置，请在环境变量或 .env 中设置 JWT_SECRET"),
  }
}

/// 根据用户模型签发 JWT
#[cfg(feature = "server")]
pub fn create_jwt(
  user: &crate::entities::user::Model,
) -> Result<String, Box<dyn std::error::Error>> {
  use jsonwebtoken::{encode, EncodingKey, Header};

  let expiration = chrono::Utc::now()
    .checked_add_signed(chrono::Duration::days(7))
    .ok_or("计算过期时间失败")?
    .timestamp() as usize;

  let claims = Claims {
    sub: user.id,
    nickname: user.nickname.clone(),
    avatar_url: user.avatar_url.clone(),
    role: user.role.clone(),
    exp: expiration,
  };

  let token =
    encode(&Header::default(), &claims, &EncodingKey::from_secret(get_jwt_secret().as_bytes()))?;

  Ok(token)
}

/// 验证 JWT 并返回 SessionUser
#[cfg(feature = "server")]
pub fn verify_jwt(token: &str) -> Result<SessionUser, Box<dyn std::error::Error>> {
  use jsonwebtoken::{decode, DecodingKey, Validation};

  let token_data = decode::<Claims>(
    token,
    &DecodingKey::from_secret(get_jwt_secret().as_bytes()),
    &Validation::default(),
  )?;

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
#[cfg(feature = "server")]
pub fn require_admin() -> Result<SessionUser, dioxus::fullstack::ServerFnError> {
  let user = require_session()?;
  if !user.is_admin() {
    return Err(dioxus::fullstack::ServerFnError::new("需要管理员权限".to_string()));
  }
  Ok(user)
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
}
