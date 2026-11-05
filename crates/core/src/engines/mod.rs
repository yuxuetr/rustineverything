//! 引擎层（Phase 1C → 8.7）。
//!
//! 本模块是站点能力的命名空间根。早期（Phase 1C.1）设计的「`Engine` trait +
//! `EngineRegistry` 全局生命周期注册中心」抽象在真实部署里从未被实例化（
//! 由 server fn 各自直接构造对应 engine 即可），Phase 8.7 删掉以减少
//! 死代码维护成本。各子模块现在就是普通的 struct + 自由函数：
//!
//! - [`theme`]：CSS 聚合（按主题栈）
//! - [`module`]：业务模块开关 + 元数据（Navbar / sitemap 通过 [`module::ModuleSpec`] 单点读取）
//! - [`moderation`]：审核 verdict 与阈值（流水线在 module-moderation）
//!
//! ## 历史
//! - Phase 8.7：删除 `EngineRegistry` / `EngineContext` / `Engine` trait（死代码）；
//!   删除 `core::engines::content::ComponentRegistry`（与 `widgets::registry` 重复）。
//! - 2026-11-05：删除从未被使用的 `layout`（LayoutEngine）、`search`（SearchSource /
//!   SearchEngine，module-search 自有实现）与 `ModerationStage` / `ModerationEngine`。

// 子引擎模块。
pub mod doc_source;
pub mod moderation;
pub mod module;
pub mod theme;
