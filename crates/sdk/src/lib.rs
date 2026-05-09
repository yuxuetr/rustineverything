use serde::{Deserialize, Serialize};
use std::mem;

/// 当前 SDK 定义的 WASM ABI 版本。宿主载入插件时会读插件中的
/// `get_manifest()` 返回体 [`PluginManifest::abi_version`]，数值不一致则拒绝加载。
///
/// 修改插件 ABI（函数签名 / 参数 / 返回格式 / 运行时内存约定）时需要
/// +1 并全量迁移已有插件。
pub const SDK_ABI_VERSION: u32 = 1;

/// 定义插件可声明的在余能力字符串。宿主据此判定插件应路由
/// 到哪个引擎（AuthEngine / ThemeEngine / ModerationEngine …）。
pub mod capabilities {
    pub const AUTH_PROVIDER: &str = "auth-provider";
    pub const THEME: &str = "theme";
    pub const I18N: &str = "i18n";
    pub const MODERATION_PROVIDER: &str = "moderation-provider";
    pub const NOTIFICATION: &str = "notification";
    pub const LAYOUT: &str = "layout";
    pub const MDX_COMPONENT: &str = "mdx-component";
}

/// 插件身份 / 能力联合信息，由 `get_manifest()` 导出。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    /// 插件实现的 ABI 版本。宿主加载时校验：不等于 [`SDK_ABI_VERSION`]
    /// 则拒绝运行（避免老插件遇上新宿主时静默出错）。
    #[serde(default)]
    pub abi_version: u32,
    /// 插件声明提供的能力。参见 [`capabilities`] 常量。
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl PluginManifest {
    /// 构造一个采用当前 ABI 版本的 manifest（插件一般调用）。
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: String::new(),
            author: String::new(),
            abi_version: SDK_ABI_VERSION,
            capabilities: Vec::new(),
        }
    }

    /// 添加一个能力。推荐使用 [`capabilities`] 中的常量。
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    /// 宿主用：检查插件是否与当前 SDK ABI 兼容。
    pub fn is_compatible(&self) -> bool {
        self.abi_version == SDK_ABI_VERSION
    }

    /// 判断插件是否声明了某个能力。
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

/// 导出供 WASM 调用的分配函数。插件作者一般不需直接调用，
/// 推荐使用 [`pack_output`] 隐藏底层细节。
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf); // 告诉 Rust 不要释放这段内存
    ptr
}

/// 导出供 WASM 调用的释放函数。同样建议使用 [`pack_output`] / [`read_input`]。
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, size);
}

/// 安全包装：读取宿主传入的输入字节串。
///
/// 该函数仅能在插件的 `unsafe extern "C"` 入口函数中调用。
/// 返回的 `&[u8]` 生命不能超过入口函数本身（宿主调用后会释放该内存）。
///
/// # Safety
/// `ptr` / `len` 必须是宿主通过 [`alloc`] 分配并写入的一段合法内存。
pub unsafe fn read_input<'a>(ptr: *mut u8, len: usize) -> &'a [u8] {
    if len == 0 || ptr.is_null() {
        return &[];
    }
    std::slice::from_raw_parts(ptr, len)
}

/// 安全包装：将插件的输出字节串写入 wasm 线性内存并返回打包后的 u64。
///
/// 返回值高 32 位 = 输出指针，低 32 位 = 输出长度。宿主读取后会调用
/// [`dealloc`] 释放该段内存。
///
/// 空输出返回 0（上下为零），宿主代码应以空字符串处理。
pub fn pack_output(bytes: Vec<u8>) -> u64 {
    let len = bytes.len();
    if len == 0 {
        return 0;
    }
    let ptr = alloc(len);
    // SAFETY: `alloc` 保证返回的指针有效且至少有 `len` 字节。
    unsafe {
        std::slice::from_raw_parts_mut(ptr, len).copy_from_slice(&bytes);
    }
    ((ptr as u64) << 32) | (len as u64)
}

/// 将任何可序列化为 JSON 的值打包为 [`pack_output`] 返回值。
/// 序列化失败返回空输出（0）。插件中用于减少样板代码。
pub fn pack_json<T: Serialize>(value: &T) -> u64 {
    match serde_json::to_vec(value) {
        Ok(bytes) => pack_output(bytes),
        Err(_) => 0,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_new_uses_current_abi() {
        let m = PluginManifest::new("test", "Test Plugin", "0.1.0");
        assert_eq!(m.abi_version, SDK_ABI_VERSION);
        assert!(m.is_compatible());
    }

    #[test]
    fn manifest_with_old_abi_is_incompatible() {
        let mut m = PluginManifest::new("test", "Test", "0.1.0");
        m.abi_version = 0; // 老插件未设置 -> default 0
        assert!(!m.is_compatible());
    }

    #[test]
    fn manifest_capability_helpers() {
        let m = PluginManifest::new("github-auth", "GitHub", "1.0.0")
            .with_capability(capabilities::AUTH_PROVIDER)
            .with_author("oz");
        assert!(m.has_capability(capabilities::AUTH_PROVIDER));
        assert!(!m.has_capability(capabilities::THEME));
        assert_eq!(m.author, "oz");
    }

    #[test]
    fn manifest_serialization_round_trip() {
        let original = PluginManifest::new("i18n", "i18n", "0.2.0")
            .with_capability(capabilities::I18N)
            .with_description("中文 / EN 翻译");
        let json = serde_json::to_string(&original).unwrap();
        let parsed: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn manifest_back_compat_with_missing_fields() {
        // 老插件只返回 id/name/version/description/author，新字段使用 default
        let json = r#"{"id":"old","name":"Old","version":"1.0","description":"","author":""}"#;
        let parsed: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.id, "old");
        assert_eq!(parsed.abi_version, 0);
        assert!(parsed.capabilities.is_empty());
        assert!(!parsed.is_compatible(), "未声明 ABI 的老插件不兼容");
    }

    #[test]
    fn pack_output_empty_returns_zero() {
        assert_eq!(pack_output(Vec::new()), 0);
    }

    /// pack_output 在 wasm32 上指针限于 32 位；native 下只验证返回值
    /// 与输入长度一致，并读取低 32 位进行释放防泄漏。
    #[test]
    fn pack_output_returns_correct_length_low_word() {
        let bytes = vec![1u8, 2, 3, 4, 5];
        let packed = pack_output(bytes);
        let len = (packed & 0xFFFF_FFFF) as usize;
        assert_eq!(len, 5);
        assert_ne!(packed, 0, "非空输入应产生非零打包值");
        // 不试图费劳释放：native 上指针高 32 位会被截断。
    }

    #[test]
    fn pack_json_returns_nonzero_for_value() {
        let value = serde_json::json!({"hello": "world"});
        let packed = pack_json(&value);
        let len = (packed & 0xFFFF_FFFF) as usize;
        assert!(len > 0);
        assert_ne!(packed, 0);
    }

    #[test]
    fn read_input_zero_len_returns_empty() {
        let buf = vec![1u8, 2, 3];
        let ptr = buf.as_ptr() as *mut u8;
        let slice = unsafe { read_input(ptr, 0) };
        assert!(slice.is_empty());
    }

    #[test]
    fn read_input_null_ptr_returns_empty() {
        let slice = unsafe { read_input(std::ptr::null_mut(), 5) };
        assert!(slice.is_empty());
    }
}
