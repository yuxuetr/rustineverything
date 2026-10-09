//! 站点双生态分类法（单一事实来源）。
//!
//! 导航 mega 菜单、首页 pillars、（后续 M3）领域筛选页共用此配置；
//! 新增/调整领域只动这一处。详见 `docs/SITE_REDESIGN_SPEC.md` §2.2。
//!
//! `Route` 不是 const，故以函数 `ecosystems()` 在调用处构造（与 `routes::Home`
//! 里的 `modules` 同样手法）。

use crate::routes::Route;

/// 一个领域条目：展示名 i18n key + 目标路由 + 用于「站点模块开关」gating 的 module id。
#[derive(Clone, PartialEq)]
pub struct Domain {
  /// 稳定 id，用作列表 key 与未来筛选标签。
  pub id: &'static str,
  /// 展示名的 i18n key。
  pub label_key: &'static str,
  /// 点击跳转的路由。
  pub route: Route,
  /// 对应 `enabled_module_ids` 的模块 id；模块关闭时该领域从导航隐藏。
  pub module_id: &'static str,
}

/// 一个生态：Rust 生态 / AI 生态。
#[derive(Clone, PartialEq)]
pub struct Ecosystem {
  /// 稳定 id（"rust" | "ai"）。
  pub id: &'static str,
  /// 生态名 i18n key。
  pub label_key: &'static str,
  /// 一句话简介 i18n key（mega 菜单与首页 pillars 复用）。
  pub blurb_key: &'static str,
  pub domains: Vec<Domain>,
}

/// 按 id 查找单个生态（"rust" | "ai"）。
pub fn ecosystem_by_id(id: &str) -> Option<Ecosystem> {
  ecosystems().into_iter().find(|e| e.id == id)
}

/// 案例 `category` 归属哪个生态（生态页过滤的单一映射来源）。
///
/// 数据模型里只有 `ai` 这一类显式属于 AI 生态，其余工程类目都归 Rust 生态。
/// 返回 `None` 表示无法判定（不计入任一生态过滤）。
pub fn ecosystem_of_case_category(category: &str) -> Option<&'static str> {
  match category {
    "ai" => Some("ai"),
    "embedded" | "web3" | "cli" | "wasm" | "backend" | "frontend" | "fullstack" | "library"
    | "tool" | "desktop" => Some("rust"),
    _ => None,
  }
}

/// 返回两大生态及其领域。
pub fn ecosystems() -> Vec<Ecosystem> {
  vec![
    Ecosystem {
      id: "rust",
      label_key: "nav.eco.rust",
      blurb_key: "nav.eco.rust.blurb",
      domains: vec![
        Domain {
          id: "embedded",
          label_key: "nav.embedded",
          route: Route::Embedded {},
          module_id: "embedded",
        },
        Domain { id: "web3", label_key: "nav.web3", route: Route::Web3 {}, module_id: "web3" },
        Domain { id: "wasm", label_key: "nav.wasm", route: Route::Wasm {}, module_id: "wasm" },
        Domain { id: "cli", label_key: "nav.cli", route: Route::Cli {}, module_id: "cli" },
      ],
    },
    Ecosystem {
      id: "ai",
      label_key: "nav.eco.ai",
      blurb_key: "nav.eco.ai.blurb",
      // M3 前 AI 子领域的标签筛选尚未就绪，这些条目先统一指向 `/ai` 索引；
      // 标签本身已体现分类法，待 M3 再改成 `/ai?d=<id>` 之类的筛选视图。
      domains: vec![
        Domain { id: "llm", label_key: "nav.ai.llm", route: Route::Ai {}, module_id: "ai" },
        Domain {
          id: "inference",
          label_key: "nav.ai.inference",
          route: Route::Ai {},
          module_id: "ai",
        },
        Domain { id: "agent", label_key: "nav.ai.agent", route: Route::Ai {}, module_id: "ai" },
        Domain { id: "rust-ai", label_key: "nav.ai.rust_ai", route: Route::Ai {}, module_id: "ai" },
      ],
    },
  ]
}
