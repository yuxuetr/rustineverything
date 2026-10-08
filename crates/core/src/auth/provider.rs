//! 内置 OAuth provider（R1 / SEC-05）。
//!
//! 端点、scope 与 profile 字段映射都写死在这里：`client_secret` 只会发往本文件
//! 给出的 token 端点，`external_id` 只从 provider 自己的 profile 响应里取。
//! 新增登录方式 = 加一个枚举分支，编译器会指出所有要补的 `match`。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 登录弹窗里一个 provider 按钮的展示信息（发给客户端）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthProviderDisplay {
  pub provider_id: String,
  pub display_name: String,
  /// SVG path 的 `d` 属性。
  pub icon_svg: String,
  pub brand_color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
  Github,
  Google,
  Discord,
  Twitter,
}

/// token 交换时客户端凭据的放法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenAuth {
  /// `client_id` / `client_secret` 放在表单里。
  Form,
  /// HTTP Basic（X 要求）。
  Basic,
}

/// 一个 provider 的固定 OAuth 参数。
#[derive(Debug)]
pub struct ProviderSpec {
  pub auth_url: &'static str,
  pub token_url: &'static str,
  pub profile_url: &'static str,
  pub scopes: &'static [&'static str],
  pub requires_pkce: bool,
  pub token_auth: TokenAuth,
  display_name: &'static str,
  icon_svg: &'static str,
  brand_color: &'static str,
}

/// 从 profile 响应映射出的、用来匹配 / 创建账号的用户信息。
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderUser {
  pub uid: String,
  pub nickname: String,
  pub avatar_url: Option<String>,
}

const GITHUB: ProviderSpec = ProviderSpec {
  auth_url: "https://github.com/login/oauth/authorize",
  token_url: "https://github.com/login/oauth/access_token",
  profile_url: "https://api.github.com/user",
  scopes: &["read:user", "user:email"],
  requires_pkce: false,
  token_auth: TokenAuth::Form,
  display_name: "GitHub",
  icon_svg: "M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.042-1.416-4.042-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.744.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.44-1.304.806-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z",
  brand_color: "#24292f",
};

const GOOGLE: ProviderSpec = ProviderSpec {
  auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
  token_url: "https://oauth2.googleapis.com/token",
  profile_url: "https://www.googleapis.com/oauth2/v2/userinfo",
  scopes: &["openid", "email", "profile"],
  requires_pkce: false,
  token_auth: TokenAuth::Form,
  display_name: "Google",
  icon_svg: "M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1zM12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23zM5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62zM12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z",
  brand_color: "#ffffff",
};

const DISCORD: ProviderSpec = ProviderSpec {
  auth_url: "https://discord.com/oauth2/authorize",
  token_url: "https://discord.com/api/oauth2/token",
  profile_url: "https://discord.com/api/users/@me",
  scopes: &["identify", "email"],
  requires_pkce: false,
  token_auth: TokenAuth::Form,
  display_name: "Discord",
  icon_svg: "M20.317 4.37a19.791 19.791 0 00-4.885-1.515.074.074 0 00-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 00-5.487 0 12.64 12.64 0 00-.617-1.25.077.077 0 00-.079-.037A19.736 19.736 0 003.677 4.37a.07.07 0 00-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 00.031.057 19.9 19.9 0 005.993 3.03.078.078 0 00.084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 00-.041-.106 13.107 13.107 0 01-1.872-.892.077.077 0 01-.008-.128 10.2 10.2 0 00.372-.292.074.074 0 01.077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 01.078.01c.12.098.246.198.373.292a.077.077 0 01-.006.127 12.299 12.299 0 01-1.873.892.077.077 0 00-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 00.084.028 19.839 19.839 0 006.002-3.03.077.077 0 00.032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 00-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.947 2.418-2.157 2.418z",
  brand_color: "#5865F2",
};

const TWITTER: ProviderSpec = ProviderSpec {
  auth_url: "https://twitter.com/i/oauth2/authorize",
  token_url: "https://api.x.com/2/oauth2/token",
  profile_url: "https://api.x.com/2/users/me?user.fields=profile_image_url,name,username",
  scopes: &["users.read", "tweet.read"],
  requires_pkce: true,
  token_auth: TokenAuth::Basic,
  display_name: "X (Twitter)",
  icon_svg: "M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z",
  brand_color: "#000000",
};

impl Provider {
  pub const ALL: [Provider; 4] =
    [Provider::Github, Provider::Google, Provider::Discord, Provider::Twitter];

  /// URL 路径、site.json 与 `user_identity.provider` 列里用的 id。
  pub fn id(self) -> &'static str {
    match self {
      Provider::Github => "github",
      Provider::Google => "google",
      Provider::Discord => "discord",
      Provider::Twitter => "twitter",
    }
  }

  pub fn from_id(id: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|p| p.id() == id)
  }

  pub fn spec(self) -> &'static ProviderSpec {
    match self {
      Provider::Github => &GITHUB,
      Provider::Google => &GOOGLE,
      Provider::Discord => &DISCORD,
      Provider::Twitter => &TWITTER,
    }
  }

  pub fn display(self) -> AuthProviderDisplay {
    let spec = self.spec();
    AuthProviderDisplay {
      provider_id: self.id().to_string(),
      display_name: spec.display_name.to_string(),
      icon_svg: spec.icon_svg.to_string(),
      brand_color: spec.brand_color.to_string(),
    }
  }

  /// 把 profile 响应映射成账号信息。拿不到 provider 用户 ID 时返回 `None`：
  /// 用占位 ID 会把不同的人登进同一个账号（SEC-04）。
  pub fn map_profile(self, profile: &Value) -> Option<ProviderUser> {
    let str_field = |v: &Value, key: &str| v[key].as_str().map(str::to_string);
    let (uid, nickname, avatar_url) = match self {
      Provider::Github => (
        profile["id"].as_i64().map(|id| id.to_string()),
        str_field(profile, "login"),
        str_field(profile, "avatar_url"),
      ),
      Provider::Google => {
        (str_field(profile, "id"), str_field(profile, "name"), str_field(profile, "picture"))
      }
      Provider::Discord => {
        let uid = str_field(profile, "id");
        let avatar = match (&uid, profile["avatar"].as_str()) {
          (Some(uid), Some(hash)) => {
            Some(format!("https://cdn.discordapp.com/avatars/{uid}/{hash}.png"))
          }
          _ => None,
        };
        let nickname = str_field(profile, "global_name").or_else(|| str_field(profile, "username"));
        (uid, nickname, avatar)
      }
      Provider::Twitter => {
        let data = &profile["data"];
        let nickname = str_field(data, "name").or_else(|| str_field(data, "username"));
        (str_field(data, "id"), nickname, str_field(data, "profile_image_url"))
      }
    };
    let uid = uid.filter(|uid| is_usable_uid(uid))?;
    let nickname = nickname.unwrap_or_else(|| format!("{} 用户", self.spec().display_name));
    Some(ProviderUser { uid, nickname, avatar_url })
  }
}

/// 空串与 `"0"` 是取不到 ID 时的典型占位值，不能用来匹配账号。
fn is_usable_uid(uid: &str) -> bool {
  let uid = uid.trim();
  !uid.is_empty() && uid != "0"
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn ids_round_trip_and_unknown_ids_are_rejected() {
    for p in Provider::ALL {
      assert_eq!(Provider::from_id(p.id()), Some(p));
    }
    for bad in ["", "GitHub", "github ", "myplatform"] {
      assert_eq!(Provider::from_id(bad), None, "{bad:?}");
    }
  }

  /// SEC-05：client_secret 与 access token 只发往这些固定的 https 端点。
  #[test]
  fn every_endpoint_is_fixed_https() {
    for p in Provider::ALL {
      let spec = p.spec();
      for url in [spec.auth_url, spec.token_url, spec.profile_url] {
        assert!(url.starts_with("https://"), "{}: {url}", p.id());
      }
    }
  }

  #[test]
  fn maps_each_provider_profile() {
    let cases = [
      (
        Provider::Github,
        json!({"id": 12345, "login": "octo", "avatar_url": "https://a/x.png"}),
        ("12345", "octo", Some("https://a/x.png")),
      ),
      (
        Provider::Google,
        json!({"id": "1098", "name": "Ann", "picture": "https://g/p.png"}),
        ("1098", "Ann", Some("https://g/p.png")),
      ),
      (
        Provider::Discord,
        json!({"id": "80351", "username": "dc", "global_name": "Disco", "avatar": "abc"}),
        ("80351", "Disco", Some("https://cdn.discordapp.com/avatars/80351/abc.png")),
      ),
      (
        Provider::Twitter,
        json!({"data": {"id": "2244", "username": "xu", "profile_image_url": "https://x/i.png"}}),
        ("2244", "xu", Some("https://x/i.png")),
      ),
    ];
    for (p, profile, (uid, nickname, avatar)) in cases {
      let user = p.map_profile(&profile).unwrap_or_else(|| panic!("{} 映射失败", p.id()));
      assert_eq!(user.uid, uid);
      assert_eq!(user.nickname, nickname);
      assert_eq!(user.avatar_url.as_deref(), avatar);
    }
  }

  #[test]
  fn missing_nickname_falls_back_to_provider_name() {
    let user = Provider::Github.map_profile(&json!({"id": 7})).map(|u| u.nickname);
    assert_eq!(user.as_deref(), Some("GitHub 用户"));
  }

  /// SEC-04：错误响应体、缺 id、占位 id 都不能映射出账号。
  #[test]
  fn profiles_without_a_usable_uid_are_rejected() {
    let error_body = json!({"message": "Bad credentials"});
    for p in Provider::ALL {
      assert_eq!(p.map_profile(&error_body), None, "{}", p.id());
    }
    assert_eq!(Provider::Github.map_profile(&json!({"id": 0})), None);
    assert_eq!(Provider::Github.map_profile(&json!({"id": "12345"})), None, "GitHub id 是数字");
    assert_eq!(Provider::Google.map_profile(&json!({"id": ""})), None);
    assert_eq!(Provider::Discord.map_profile(&json!({"id": "0", "avatar": "abc"})), None);
    assert_eq!(Provider::Twitter.map_profile(&json!({"id": "2244"})), None, "X 的字段在 data 里");
  }
}
