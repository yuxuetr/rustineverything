use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
    AuthorizationCode, TokenResponse,
};
use serde::{Deserialize, Serialize};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set};
use crate::entities::{user, user_identity};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub github_client_id: String,
    pub github_client_secret: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub redirect_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub uid: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

pub struct AuthService {
    pub config: AuthConfig,
}

impl AuthService {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }

    pub fn get_github_client(&self) -> BasicClient {
        BasicClient::new(
            ClientId::new(self.config.github_client_id.clone()),
            Some(ClientSecret::new(self.config.github_client_secret.clone())),
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.config.redirect_url.clone()).unwrap())
    }

    pub fn get_google_client(&self) -> BasicClient {
        BasicClient::new(
            ClientId::new(self.config.google_client_id.clone()),
            Some(ClientSecret::new(self.config.google_client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.config.redirect_url.clone()).unwrap())
    }

    /// 执行 GitHub 登录后的用户同步
    pub async fn sync_github_user(&self, db: &DatabaseConnection, code: String) -> Result<user::Model, Box<dyn std::error::Error>> {
        let client = self.get_github_client();
        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(oauth2::reqwest::async_http_client)
            .await?;

        // 1. 获取用户信息 (Mock API call)
        let http_client = reqwest::Client::new();
        let github_user: Value = http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token.access_token().secret()))
            .header("User-Agent", "rustineverything-app")
            .send()
            .await?
            .json()
            .await?;

        let uid = github_user["id"].as_i64().unwrap().to_string();
        let nickname = github_user["login"].as_str().unwrap().to_string();
        let avatar_url = github_user["avatar_url"].as_str().map(|s| s.to_string());

        // 2. 数据库检查与落库 (实现核心逻辑)
        self.sync_user_to_db(db, "github", uid, nickname, avatar_url, token.access_token().secret().to_string()).await
    }

    async fn sync_user_to_db(&self, db: &DatabaseConnection, provider: &str, uid: String, nickname: String, avatar_url: Option<String>, token: String) -> Result<user::Model, Box<dyn std::error::Error>> {
        // 先检查 identity
        let identity = user_identity::Entity::find()
            .filter(user_identity::Column::Provider.eq(provider))
            .filter(user_identity::Column::ProviderUid.eq(&uid))
            .one(db)
            .await?;

        if let Some(ident) = identity {
            // 已存在，返回关联的 User
            let user = user::Entity::find_by_id(ident.user_id)
                .one(db)
                .await?
                .ok_or("User not found for identity")?;
            Ok(user)
        } else {
            // 新用户：创建 User
            let new_user = user::ActiveModel {
                nickname: Set(nickname),
                avatar_url: Set(avatar_url),
                role: Set("member".to_string()),
                created_at: Set(Utc::now().fixed_offset()),
                updated_at: Set(Utc::now().fixed_offset()),
                ..Default::default()
            };
            let user_res = user::Entity::insert(new_user).exec(db).await?;
            
            // 创建 Identity
            let new_ident = user_identity::ActiveModel {
                user_id: Set(user_res.last_insert_id),
                provider: Set(provider.to_string()),
                provider_uid: Set(uid),
                access_token: Set(Some(token)),
                created_at: Set(Utc::now().fixed_offset()),
                ..Default::default()
            };
            user_identity::Entity::insert(new_ident).exec(db).await?;

            let user_final = user::Entity::find_by_id(user_res.last_insert_id)
                .one(db)
                .await?
                .ok_or("Failed to retrieve new user")?;
            Ok(user_final)
        }
    }
}

use serde_json::Value;
