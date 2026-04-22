use serde::{Deserialize, Serialize};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set};
use crate::entities::{user, user_identity};
use chrono::Utc;
use serde_json::Value;
use rustineverything_sdk::{StandardUser, AuthProviderConfig, AuthProviderDisplay};
use crate::PluginManager;
use crate::settings::{SiteConfig, AuthProviderEntry};
use std::path::PathBuf;

/// 动态认证配置，不再硬编码单个 provider
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub base_url: String,         // 如 "http://localhost:8080"
}

impl AuthConfig {
    /// 从环境变量动态读取 provider 的 client_id 和 client_secret
    /// 约定：{PROVIDER}_CLIENT_ID, {PROVIDER}_CLIENT_SECRET (全大写)
    pub fn get_credentials(provider: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
        let upper = provider.to_uppercase();
        let client_id = std::env::var(format!("{}_CLIENT_ID", upper))
            .map_err(|_| format!("未配置环境变量: {}_CLIENT_ID", upper))?;
        let client_secret = std::env::var(format!("{}_CLIENT_SECRET", upper))
            .map_err(|_| format!("未配置环境变量: {}_CLIENT_SECRET", upper))?;
        Ok((client_id, client_secret))
    }

    /// 检查 provider 是否已配置凭据
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

pub struct AuthService {
    pub config: AuthConfig,
    pub plugin_manager: PluginManager,
    pub plugin_dir: PathBuf,
}

impl AuthService {
    pub fn new(config: AuthConfig, plugin_dir: PathBuf) -> Self {
        Self {
            config,
            plugin_manager: PluginManager::new(),
            plugin_dir,
        }
    }

    /// 根据 site.json 配置，返回已安装且已配置凭据的 auth provider 展示列表
    pub fn list_available_providers(&self, site_config: &SiteConfig) -> Vec<AuthProviderDisplay> {
        if !site_config.auth.enabled {
            return vec![];
        }

        let mut result = Vec::new();
        for entry in &site_config.auth.providers {
            let plugin_path = self.plugin_dir.join(&entry.plugin);

            // 插件文件必须存在
            if !plugin_path.exists() {
                println!("[Auth] 插件不存在，跳过: {:?}", plugin_path);
                continue;
            }

            // 环境变量必须配置
            if !AuthConfig::has_credentials(&entry.id) {
                println!("[Auth] 未配置凭据，跳过: {}", entry.id);
                continue;
            }

            // 调用插件获取展示信息
            match std::fs::read(&plugin_path) {
                Ok(wasm_bytes) => {
                    match self.plugin_manager.call_with_string(&wasm_bytes, "get_display_info", "") {
                        Ok(json) => {
                            match serde_json::from_str::<AuthProviderDisplay>(&json) {
                                Ok(display) => result.push(display),
                                Err(e) => println!("[Auth] 解析 display_info 失败 ({}): {}", entry.id, e),
                            }
                        }
                        Err(e) => println!("[Auth] 调用 get_display_info 失败 ({}): {}", entry.id, e),
                    }
                }
                Err(e) => println!("[Auth] 读取插件失败 ({}): {}", entry.id, e),
            }
        }

        result
    }

    /// 加载插件并生成授权 URL
    pub fn get_auth_url(&self, provider: &str, plugin_filename: &str) -> Result<String, Box<dyn std::error::Error>> {
        let plugin_path = self.plugin_dir.join(plugin_filename);
        if !plugin_path.exists() {
            return Err(format!("未找到插件: {:?}", plugin_path).into());
        }

        let wasm_bytes = std::fs::read(plugin_path)?;
        let config_json = self.plugin_manager.call_with_string(&wasm_bytes, "get_provider_config", "")?;
        let provider_config: AuthProviderConfig = serde_json::from_str(&config_json)?;

        let (client_id, _) = AuthConfig::get_credentials(provider)?;
        let redirect_url = self.config.redirect_url(provider);

        let scopes = provider_config.scopes.join(" ");
        let url = format!(
            "{}?client_id={}&redirect_uri={}&scope={}&response_type=code&state=TODO_STATE",
            provider_config.auth_url,
            client_id,
            redirect_url,
            scopes
        );

        Ok(url)
    }

    pub async fn handle_callback(&self, db: &DatabaseConnection, provider: &str, plugin_filename: &str, code: String) -> Result<user::Model, Box<dyn std::error::Error>> {
        let plugin_path = self.plugin_dir.join(plugin_filename);
        let wasm_bytes = std::fs::read(&plugin_path)?;

        // 1. 获取插件配置
        let config_json = self.plugin_manager.call_with_string(&wasm_bytes, "get_provider_config", "")?;
        let provider_config: AuthProviderConfig = serde_json::from_str(&config_json)?;

        let (client_id, client_secret) = AuthConfig::get_credentials(provider)?;
        let redirect_url = self.config.redirect_url(provider);

        // 2. Token 交换
        let http_client = reqwest::Client::new();
        println!("[Auth] Token exchange: url={}, client_id={}, redirect_uri={}", provider_config.token_url, client_id, redirect_url);
        let token_response: Value = http_client
            .post(&provider_config.token_url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("code", &code),
                ("redirect_uri", redirect_url.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await?
            .json()
            .await?;
        println!("[Auth] Token response: {:?}", token_response);

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or_else(|| format!("Token 交换失败: {:?}", token_response))?;

        // 3. 获取用户信息
        println!("[Auth] Fetching user profile from: {}", provider_config.profile_url);
        let profile_response: Value = http_client
            .get(&provider_config.profile_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "rustineverything-app")
            .send()
            .await?
            .json()
            .await?;
        println!("[Auth] Profile response: {:?}", profile_response);

        // 4. 插件 Profile 映射
        let standard_user_json = self.plugin_manager.call_with_string(
            &wasm_bytes,
            "map_profile",
            &profile_response.to_string()
        )?;
        let standard_user: StandardUser = serde_json::from_str(&standard_user_json)?;

        // 5. 同步至数据库
        self.sync_user_to_db(
            db,
            provider,
            standard_user.external_id,
            standard_user.nickname,
            standard_user.avatar_url,
            access_token.to_string()
        ).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_github_auth_plugin_logic() {
        // 插件路径（基于 target-dir 的位置或 build 后的位置）
        let wasm_path = "../../assets/plugins/github_auth_plugin.wasm";
        if !std::path::Path::new(wasm_path).exists() {
            println!("跳过测试：插件文件不存在");
            return;
        }

        let wasm_bytes = fs::read(wasm_path).expect("读取插件失败");
        let manager = crate::PluginManager::new();

        // 1. 测试获取配置
        let config_json = manager.call_with_string(&wasm_bytes, "get_provider_config", "").expect("调用 get_provider_config 失败");
        let config: AuthProviderConfig = serde_json::from_str(&config_json).expect("解析配置 JSON 失败");
        assert_eq!(config.auth_url, "https://github.com/login/oauth/authorize");
        assert!(config.scopes.contains(&"read:user".to_string()));

        // 2. 测试 Profile 映射
        let mock_raw_profile = serde_json::json!({
            "id": 12345,
            "login": "test_user",
            "avatar_url": "https://example.com/avatar.png",
            "email": "test@example.com"
        }).to_string();

        let standard_user_json = manager.call_with_string(&wasm_bytes, "map_profile", &mock_raw_profile).expect("调用 map_profile 失败");
        let user: StandardUser = serde_json::from_str(&standard_user_json).expect("解析 StandardUser 失败");
        
        assert_eq!(user.external_id, "12345");
        assert_eq!(user.nickname, "test_user");
        assert_eq!(user.provider, "github");
        assert_eq!(user.email, Some("test@example.com".to_string()));
    }
}
