//! 引擎层抽象（Phase 1C.1）。
//!
//! 把站点能力切分到 8 个核心引擎：PluginEngine / ModuleEngine /
//! AuthEngine / ThemeEngine / LayoutEngine / ContentEngine /
//! ModerationEngine / SearchEngine。
//!
//! 本模块仅定义最薄的「注册 + 生命周期」抽象：
//! - [`Engine`]：每个引擎实现的统一 trait（`name() / init() / shutdown()`）。
//! - [`EngineRegistry`]：按注册顺序保存引擎，支持按名/按类型获取。
//! - [`EngineContext`]：注入到每个引擎 `init` 阶段的共享上下文。
//!
//! ## 生命周期
//! 1. 注册阶段：`registry.register(engine)`
//! 2. 初始化阶段：`registry.init_all(&ctx)?`，按注册顺序调用 `init`。
//! 3. 运行阶段：`registry.get::<T>("name")` 拿出具体引擎使用。
//! 4. 关闭阶段：`registry.shutdown_all()?`，按注册逆序调用 `shutdown`。
//!
//! ## 设计要点
//! - **注册顺序即依赖顺序**：先注册被依赖者（如 PluginEngine），后注册依赖
//!   它们的引擎（如 ThemeEngine 通过 PluginEngine 调 wasm）。
//! - **重复名拒绝**：注册同名引擎返回 `AppError::Other`，避免静默覆盖。
//! - **Send + Sync**：所有引擎可跨线程；server fn 中通过 `Arc<EngineRegistry>`
//!   共享访问。

// 子引擎模块在后续阶段（1C.2 / 1C.3 / 1C.4）逐步加入。
pub mod plugin;

use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::{AppError, AppResult};
use crate::settings::SiteConfig;

/// 注入到每个引擎 `init` 阶段的共享上下文。
///
/// 不直接放 `PluginManager` / DB 句柄等可变资源，避免和 `EngineRegistry`
/// 互相借用。这些资源由对应引擎自身管理（`PluginEngine` 持有插件管理器，
/// 数据库句柄通过 `crate::db::get_or_init_pool()` 全局取出）。
#[derive(Clone)]
pub struct EngineContext {
    /// 站点配置（site.json 解析结果）。
    pub site_config: Arc<SiteConfig>,
    /// 资产根目录，例如 `assets`。
    pub asset_root: PathBuf,
}

impl EngineContext {
    pub fn new(site_config: SiteConfig, asset_root: PathBuf) -> Self {
        Self {
            site_config: Arc::new(site_config),
            asset_root,
        }
    }

    /// 默认上下文：调试 / 测试 / 单元测试用。
    pub fn for_tests() -> Self {
        Self::new(SiteConfig::default(), PathBuf::from("assets"))
    }
}

/// 所有引擎实现的统一 trait。
///
/// 约定：
/// - `name()` 返回静态字符串，用于注册表 key（必须唯一）。
/// - `init()` 在注册后被 [`EngineRegistry::init_all`] 触发，可访问
///   注入的 [`EngineContext`]。
/// - `shutdown()` 由 [`EngineRegistry::shutdown_all`] 反序触发，用于
///   释放外部资源（关闭 wasm 实例 / 关闭网络连接等）。
/// - `as_any` / `as_any_mut` 方便注册表按类型 downcast。
pub trait Engine: Send + Sync + Any {
    /// 引擎唯一名（注册表 key）。
    fn name(&self) -> &'static str;

    /// 初始化钩子。默认空实现；具体引擎可读 `ctx.site_config` 进行装配。
    fn init(&mut self, _ctx: &EngineContext) -> AppResult<()> {
        Ok(())
    }

    /// 关闭钩子。默认空实现；持有外部资源的引擎应在这里释放。
    fn shutdown(&mut self) -> AppResult<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// 引擎注册表：按注册顺序保存引擎，支持按名/按类型获取。
#[derive(Default)]
pub struct EngineRegistry {
    engines: Vec<Box<dyn Engine>>,
    name_to_index: HashMap<&'static str, usize>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个引擎。重复名返回 `AppError::Other`。
    pub fn register<E: Engine + 'static>(&mut self, engine: E) -> AppResult<()> {
        let name = engine.name();
        if self.name_to_index.contains_key(name) {
            return Err(AppError::other(format!("引擎已注册: {}", name)));
        }
        let idx = self.engines.len();
        self.engines.push(Box::new(engine));
        self.name_to_index.insert(name, idx);
        Ok(())
    }

    /// 按注册顺序初始化全部引擎。任一引擎 init 失败即返回错误。
    pub fn init_all(&mut self, ctx: &EngineContext) -> AppResult<()> {
        for engine in self.engines.iter_mut() {
            engine.init(ctx)?;
        }
        Ok(())
    }

    /// 按注册逆序关闭全部引擎。任一引擎 shutdown 失败仅记录到第一个错误。
    pub fn shutdown_all(&mut self) -> AppResult<()> {
        let mut first_err: Option<AppError> = None;
        for engine in self.engines.iter_mut().rev() {
            if let Err(e) = engine.shutdown() {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// 按类型获取引擎不可变引用。`name` 不存在或类型不匹配 → `None`。
    pub fn get<E: Engine + 'static>(&self, name: &str) -> Option<&E> {
        let idx = *self.name_to_index.get(name)?;
        self.engines
            .get(idx)
            .and_then(|e| e.as_any().downcast_ref::<E>())
    }

    /// 按类型获取引擎可变引用。`name` 不存在或类型不匹配 → `None`。
    pub fn get_mut<E: Engine + 'static>(&mut self, name: &str) -> Option<&mut E> {
        let idx = *self.name_to_index.get(name)?;
        self.engines
            .get_mut(idx)
            .and_then(|e| e.as_any_mut().downcast_mut::<E>())
    }

    /// 检查是否已注册某名字的引擎（不关心具体类型）。
    pub fn contains(&self, name: &str) -> bool {
        self.name_to_index.contains_key(name)
    }

    /// 已注册的引擎总数。
    pub fn len(&self) -> usize {
        self.engines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.engines.is_empty()
    }

    /// 已注册的引擎名清单（顺序与注册顺序一致）。
    pub fn names(&self) -> Vec<&'static str> {
        self.engines.iter().map(|e| e.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 测试用引擎：记录 init / shutdown 调用次数。
    struct CountingEngine {
        name: &'static str,
        init_count: Arc<AtomicUsize>,
        shutdown_count: Arc<AtomicUsize>,
        fail_init: bool,
    }

    impl CountingEngine {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                init_count: Arc::new(AtomicUsize::new(0)),
                shutdown_count: Arc::new(AtomicUsize::new(0)),
                fail_init: false,
            }
        }

        fn failing(name: &'static str) -> Self {
            Self {
                fail_init: true,
                ..Self::new(name)
            }
        }
    }

    impl Engine for CountingEngine {
        fn name(&self) -> &'static str {
            self.name
        }
        fn init(&mut self, _ctx: &EngineContext) -> AppResult<()> {
            self.init_count.fetch_add(1, Ordering::SeqCst);
            if self.fail_init {
                Err(AppError::other("init failed"))
            } else {
                Ok(())
            }
        }
        fn shutdown(&mut self) -> AppResult<()> {
            self.shutdown_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    /// 用于验证按类型 downcast 失败 → None 的另一种引擎。
    struct OtherEngine;
    impl Engine for OtherEngine {
        fn name(&self) -> &'static str {
            "other"
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn register_two_distinct_engines() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("a")).unwrap();
        reg.register(CountingEngine::new("b")).unwrap();
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.names(), vec!["a", "b"]);
        assert!(reg.contains("a"));
        assert!(!reg.contains("missing"));
    }

    #[test]
    fn register_duplicate_name_fails() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("dup")).unwrap();
        let err = reg.register(CountingEngine::new("dup")).unwrap_err();
        assert!(matches!(err, AppError::Other(_)));
        // 失败后注册表保持原状
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn init_all_invokes_each_engine_in_order() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("a")).unwrap();
        reg.register(CountingEngine::new("b")).unwrap();
        let ctx = EngineContext::for_tests();
        reg.init_all(&ctx).unwrap();
        let a = reg.get::<CountingEngine>("a").expect("a should be present");
        let b = reg.get::<CountingEngine>("b").expect("b should be present");
        assert_eq!(a.init_count.load(Ordering::SeqCst), 1);
        assert_eq!(b.init_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn init_all_propagates_first_error() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("ok")).unwrap();
        reg.register(CountingEngine::failing("boom")).unwrap();
        let ctx = EngineContext::for_tests();
        let err = reg.init_all(&ctx).unwrap_err();
        // 错误信息来自 failing 引擎
        assert!(format!("{}", err).contains("init failed"));
    }

    #[test]
    fn shutdown_all_invokes_each_engine() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("first")).unwrap();
        reg.register(CountingEngine::new("second")).unwrap();
        reg.shutdown_all().unwrap();
        let first = reg.get::<CountingEngine>("first").unwrap();
        let second = reg.get::<CountingEngine>("second").unwrap();
        assert_eq!(first.shutdown_count.load(Ordering::SeqCst), 1);
        assert_eq!(second.shutdown_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn get_with_wrong_type_returns_none() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("x_engine")).unwrap();
        // 用错误的类型查询会返回 None（不会 panic）
        assert!(reg.get::<OtherEngine>("x_engine").is_none());
        // 用正确的类型查询 OK
        assert!(reg.get::<CountingEngine>("x_engine").is_some());
    }

    #[test]
    fn get_mut_allows_mutation() {
        let mut reg = EngineRegistry::new();
        reg.register(CountingEngine::new("m")).unwrap();
        let engine = reg
            .get_mut::<CountingEngine>("m")
            .expect("engine should exist");
        // 直接调用 init 增加计数
        let ctx = EngineContext::for_tests();
        engine.init(&ctx).unwrap();
        assert_eq!(engine.init_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn engine_context_for_tests_is_usable() {
        let ctx = EngineContext::for_tests();
        // SiteConfig::default() 默认主题应来自 ocean
        assert_eq!(ctx.site_config.active_theme, "theme_ocean_plugin.wasm");
        assert_eq!(ctx.asset_root, PathBuf::from("assets"));
    }
}
