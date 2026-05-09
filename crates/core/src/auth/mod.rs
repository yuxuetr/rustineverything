use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, TransactionTrait};
#[cfg(feature = "server")]
use crate::entities::{user, user_identity};
#[cfg(feature = "server")]
use chrono::Utc;
use serde_json::Value;
use rustineverything_sdk::{StandardUser, AuthProviderConfig, AuthProviderDisplay};
#[cfg(feature = "server")]
use crate::PluginManager;
use crate::settings::SiteConfig;
use std::path::PathBuf;
#[cfg(feature = "server")]
use std::collections::HashMap;
#[cfg(feature = "server")]
use std::sync::Mutex;
#[cfg(feature = "server")]
use std::time::{Duration, Instant};

/// PKCE / state 条目的过期时间（5 分钟）
#[cfg(feature = "server")]
const PKCE_TTL_SECS: u64 = 5 * 60;

/// PKCE 仓储条目：code_verifier + 创建时间
#[cfg(feature = "server")]
struct PkceEntry {
    verifier: String,
    created_at: Instant,
}

/// state CSRF 仓储条目：provider + 创建时间
#[cfg(feature = "server")]
struct StateEntry {
    provider: String,
    created_at: Instant,
}

/// 全局存储 PKCE code_verifier，key 为 state 参数
#[cfg(feature = "server")]
static PKCE_STORE: std::sync::LazyLock<Mutex<HashMap<String, PkceEntry>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// 全局存储 OAuth state（CSRF 防御），key 为 state 参数
#[cfg(feature = "server")]
static STATE_STORE: std::sync::LazyLock<Mutex<HashMap<String, StateEntry>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// 清理过期的 PKCE / state 条目（在每次插入 / 查询时调用）
#[cfg(feature = "server")]
fn cleanup_expired_pkce(store: &mut HashMap<String, PkceEntry>) {
    let ttl = Duration::from_secs(PKCE_TTL_SECS);
    store.retain(|_, entry| entry.created_at.elapsed() < ttl);
}

#[cfg(feature = "server")]
fn cleanup_expired_states(store: &mut HashMap<String, StateEntry>) {
    let ttl = Duration::from_secs(PKCE_TTL_SECS);
    store.retain(|_, entry| entry.created_at.elapsed() < ttl);
}

/// 动态认证配置，不再硬编码单个 provider
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub base_url: String,         // 如 "http://localhost:8080"
}

impl AuthConfig {
    /// 从环境变量动态读取 provider 的 client_id 和 client_secret
    /// 约定：{PROVIDER}_CLIENT_ID, {PROVIDER}_CLIENT_SECRET (全大写)
    #[cfg(feature = "server")]
    pub fn get_credentials(provider: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
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

        // 生成随机 state
        use rand::Rng;
        let state: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(32)
            .map(|b| b as char)
            .collect();

        // 记录 state 以供后续 CSRF 验证（带 5 分钟 TTL）
        if let Ok(mut store) = STATE_STORE.lock() {
            cleanup_expired_states(&mut store);
            store.insert(state.clone(), StateEntry {
                provider: provider.to_string(),
                created_at: Instant::now(),
            });
        }

        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&scope={}&response_type=code&state={}",
            provider_config.auth_url,
            client_id,
            redirect_url,
            scopes,
            state
        );

        // PKCE: 生成 code_verifier 和 code_challenge
        if provider_config.requires_pkce {
            use sha2::Digest;
            use base64::Engine;

            let code_verifier: String = rand::rng()
                .sample_iter(&rand::distr::Alphanumeric)
                .take(64)
                .map(|b| b as char)
                .collect();

            let digest = sha2::Sha256::digest(code_verifier.as_bytes());
            let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);

            url.push_str(&format!("&code_challenge={}&code_challenge_method=S256", code_challenge));

            // 存储 code_verifier，回调时使用（带 TTL）
            if let Ok(mut store) = PKCE_STORE.lock() {
                cleanup_expired_pkce(&mut store);
                store.insert(state.clone(), PkceEntry {
                    verifier: code_verifier,
                    created_at: Instant::now(),
                });
            }
            println!("[Auth] PKCE enabled for provider={}", provider);
        }

        Ok(url)
    }

    /// 验证 OAuth state。如果 state 合法，从存储中移除并返回 Ok；否则返回错误。
    pub fn validate_state(state: &str, expected_provider: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut store = STATE_STORE.lock()
            .map_err(|_| "state 存储互斥异常")?;
        cleanup_expired_states(&mut store);
        let entry = store.remove(state).ok_or("不合法或已过期的 state")?;
        if entry.provider != expected_provider {
            return Err("state 与 provider 不匹配".into());
        }
        if entry.created_at.elapsed() > Duration::from_secs(PKCE_TTL_SECS) {
            return Err("state 已过期".into());
        }
        Ok(())
    }

    pub async fn handle_callback(&self, db: &DatabaseConnection, provider: &str, plugin_filename: &str, code: String, state: Option<String>) -> Result<user::Model, Box<dyn std::error::Error>> {
        // 0. CSRF 防御：验证 state
        let state_str = state.as_deref().ok_or("缺失 state 参数")?;
        Self::validate_state(state_str, provider)?;

        let plugin_path = self.plugin_dir.join(plugin_filename);
        let wasm_bytes = std::fs::read(&plugin_path)?;

        // 1. 获取插件配置
        let config_json = self.plugin_manager.call_with_string(&wasm_bytes, "get_provider_config", "")?;
        let provider_config: AuthProviderConfig = serde_json::from_str(&config_json)?;

        let (client_id, client_secret) = AuthConfig::get_credentials(provider)?;
        let redirect_url = self.config.redirect_url(provider);

        // 2. 构建 Token 交换请求
        let http_client = reqwest::Client::new();
        println!(
            "[Auth] Token exchange: provider={}, auth_method={}",
            provider, provider_config.token_auth_method
        );

        let mut form_params: Vec<(&str, String)> = vec![
            ("code", code),
            ("redirect_uri", redirect_url),
            ("grant_type", "authorization_code".to_string()),
        ];

        // PKCE: 添加 code_verifier
        let code_verifier = if provider_config.requires_pkce {
            let entry = PKCE_STORE.lock().ok()
                .and_then(|mut store| {
                    cleanup_expired_pkce(&mut store);
                    store.remove(state_str)
                })
                .ok_or("PKCE code_verifier not found for this state")?;
            if entry.created_at.elapsed() > Duration::from_secs(PKCE_TTL_SECS) {
                return Err("PKCE code_verifier 已过期".into());
            }
            println!("[Auth] PKCE code_verifier matched");
            Some(entry.verifier)
        } else {
            None
        };

        if let Some(ref cv) = code_verifier {
            form_params.push(("code_verifier", cv.clone()));
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
        let access_token = token_response["access_token"]
            .as_str()
            .ok_or_else(|| {
                let err_kind = token_response["error"].as_str().unwrap_or("unknown_error");
                format!("Token 交换失败 (provider={}, error={})", provider, err_kind)
            })?;
        println!("[Auth] Token exchange success: provider={}", provider);

        // 3. 获取用户信息
        let profile_response: Value = http_client
            .get(&provider_config.profile_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "rustineverything-app")
            .send()
            .await?
            .json()
            .await?;
        println!("[Auth] Profile fetched: provider={}", provider);

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
                access_token: Set(Some(token)),
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

    #[test]
    fn test_state_csrf_validation_invalid_state_rejected() {
        // 伪造一个未注册的 state，验证调用应被拒绝
        let result = AuthService::validate_state("this-state-was-never-stored-xyz", "github");
        assert!(result.is_err(), "未注册的 state 应该被拒绝");
    }

    #[test]
    fn test_state_csrf_validation_provider_mismatch() {
        // 手动插入一个 state，使用不同 provider 验证应拒绝
        let state = "test-state-mismatch";
        if let Ok(mut store) = STATE_STORE.lock() {
            store.insert(state.to_string(), StateEntry {
                provider: "github".to_string(),
                created_at: Instant::now(),
            });
        }
        let result = AuthService::validate_state(state, "google");
        assert!(result.is_err(), "provider 不匹配的 state 应被拒绝");
    }

    #[test]
    fn test_state_csrf_validation_consumed_once() {
        // state 验证成功后，重复使用同一 state 应拒绝
        let state = "test-state-once";
        if let Ok(mut store) = STATE_STORE.lock() {
            store.insert(state.to_string(), StateEntry {
                provider: "github".to_string(),
                created_at: Instant::now(),
            });
        }
        let first = AuthService::validate_state(state, "github");
        assert!(first.is_ok(), "首次验证应该通过");
        let second = AuthService::validate_state(state, "github");
        assert!(second.is_err(), "state 应只能被消费一次");
    }

    #[test]
    fn test_pkce_cleanup_removes_expired_entries() {
        // 加入 100 个超出 TTL 的过期项以及 1 个新项，验证 cleanup 会仅保留未过期的
        let mut local: HashMap<String, PkceEntry> = HashMap::new();
        let very_old = Instant::now()
            .checked_sub(Duration::from_secs(PKCE_TTL_SECS + 60))
            .expect("can subtract from now");
        for i in 0..100 {
            local.insert(
                format!("old-{}", i),
                PkceEntry { verifier: "v".to_string(), created_at: very_old },
            );
        }
        local.insert("fresh".to_string(), PkceEntry {
            verifier: "v".to_string(),
            created_at: Instant::now(),
        });

        cleanup_expired_pkce(&mut local);
        assert_eq!(local.len(), 1);
        assert!(local.contains_key("fresh"));
    }
}
