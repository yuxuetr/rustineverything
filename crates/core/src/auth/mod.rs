use serde::{Deserialize, Serialize};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set};
use crate::entities::{user, user_identity};
use chrono::Utc;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub github_client_id: String,
    pub github_client_secret: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub redirect_url: String,
}

pub struct AuthService {
    pub config: AuthConfig,
}

impl AuthService {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }

    pub async fn sync_github_user(&self, db: &DatabaseConnection, code: String) -> Result<user::Model, Box<dyn std::error::Error>> {
        let http_client = reqwest::Client::new();
        
        // 1. 直接通过 reqwest 发送 Token 交换请求
        let token_response: Value = http_client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", &self.config.github_client_id),
                ("client_secret", &self.config.github_client_secret),
                ("code", &code),
                ("redirect_uri", &self.config.redirect_url),
            ])
            .send()
            .await?
            .json()
            .await?;

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or_else(|| format!("GitHub 认证失败: {:?}", token_response))?;

        // 2. 获取用户信息
        let github_user: Value = http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "rustineverything-app")
            .send()
            .await?
            .json()
            .await?;

        let uid = github_user["id"].as_i64().ok_or("无效的 GitHub UID")?.to_string();
        let nickname = github_user["login"].as_str().unwrap_or("GitHub用户").to_string();
        let avatar_url = github_user["avatar_url"].as_str().map(|s| s.to_string());

        self.sync_user_to_db(db, "github", uid, nickname, avatar_url, access_token.to_string()).await
    }

    async fn sync_user_to_db(&self, db: &DatabaseConnection, provider: &str, uid: String, nickname: String, avatar_url: Option<String>, token: String) -> Result<user::Model, Box<dyn std::error::Error>> {
        let identity = user_identity::Entity::find()
            .filter(user_identity::Column::Provider.eq(provider))
            .filter(user_identity::Column::ProviderUid.eq(&uid))
            .one(db)
            .await?;

        if let Some(ident) = identity {
            let user = user::Entity::find_by_id(ident.user_id)
                .one(db)
                .await?
                .ok_or("找不到关联用户")?;
            Ok(user)
        } else {
            let new_user = user::ActiveModel {
                nickname: Set(nickname),
                avatar_url: Set(avatar_url),
                role: Set("member".to_string()),
                created_at: Set(Utc::now().fixed_offset()),
                updated_at: Set(Utc::now().fixed_offset()),
                ..Default::default()
            };
            let user_res = user::Entity::insert(new_user).exec(db).await?;
            
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
                .ok_or("无法获取新用户")?;
            Ok(user_final)
        }
    }
}
