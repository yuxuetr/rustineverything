use serde::{Deserialize, Serialize};

/// 会话用户信息，前后端共享
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUser {
    pub id: i32,
    pub nickname: String,
    pub avatar_url: Option<String>,
    pub role: String,
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

#[cfg(feature = "server")]
fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "rustineverything-default-secret-change-me".to_string())
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

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_jwt_secret().as_bytes()),
    )?;

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
