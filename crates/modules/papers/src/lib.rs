//! 论文资讯：每天从 HF Daily Papers 选题，读 arXiv 全文生成带出处的解读。
//!
//! 设计见内容仓库 `plans/papers.md`。页面与读库在这里；采集流水线（`ingest` feature）
//! 是单独的二进制，不在 app 进程里跑。

#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "ingest")]
pub mod ingest;
pub mod pages;
pub mod server;

pub use pages::PapersIndexPage;
