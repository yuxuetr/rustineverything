use serde::{Deserialize, Serialize};
use std::mem;
use std::slice;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// 导出供 WASM 调用的分配函数
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf); // 告诉 Rust 不要释放这段内存
    ptr
}

/// 导出供 WASM 调用的释放函数
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, size);
}

/// 核心 Trait 定义
pub trait Plugin {
    fn manifest(&self) -> PluginManifest;
    fn on_load(&self) {}
}

/// 标准化用户信息 (用于 Auth 插件输出)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StandardUser {
    pub external_id: String,      // 第三方平台唯一 ID
    pub nickname: String,         // 用户昵称
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub provider: String,         // "github", "google", "feishu" 等
    pub raw_data: String,         // 存储原始 Profile JSON 备份
}

/// 认证平台配置 (由插件提供给宿主)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProviderConfig {
    pub auth_url: String,         // 授权页面地址
    pub token_url: String,        // 获取 Token 的 API 地址
    pub profile_url: String,      // 获取用户信息的 API 地址
    pub scopes: Vec<String>,      // 需要申请的权限列表
    #[serde(default)]
    pub requires_pkce: bool,      // 是否需要 PKCE (Twitter 等)
    #[serde(default = "default_token_auth_method")]
    pub token_auth_method: String, // Token 交换认证方式: "form" (default) 或 "basic_auth"
}

fn default_token_auth_method() -> String {
    "form".to_string()
}

/// 插件展示信息 (由插件通过 get_display_info 导出)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthProviderDisplay {
    pub provider_id: String,      // 插件标识，如 "github"
    pub display_name: String,     // 显示名称，如 "GitHub"
    pub icon_svg: String,         // SVG path d 属性
    pub brand_color: String,      // 品牌色 hex，如 "#24292f"
}

/// 内置业务模块 Trait
/// 用于规范 博客、播客、论坛等模块的初始化
pub trait AppModule {
    fn name(&self) -> &'static str;
    
    /// 模块初始化钩子（例如初始化数据库表）
    fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
