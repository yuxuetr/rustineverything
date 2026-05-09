//! Phase 3.3：Layout 包。
//!
//! `Navbar`（在 `crates/app/src/components/nav.rs`）作为 Dioxus Routable
//! `#[layout(...)]` 入口，内部根据 server fn `get_active_layout` 的结果
//! 选择渲染 [`ClassicShell`] 或 [`MinimalShell`]。
//!
//! - **ClassicShell**：原始 Navbar+Footer，最完整体验。
//! - **MinimalShell**：紧凑顶部条 + 无 Footer + 仅 Logo/搜索/主题/暗色/语言；
//!   适合写作 / 阅读优先场景。
//!
//! 两个 shell 都嵌入 `Outlet::<Route>` 以渲染当前路由的内容。

pub mod classic;
pub mod minimal;

pub use classic::ClassicShell;
pub use minimal::MinimalShell;
