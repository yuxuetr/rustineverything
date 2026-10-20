//! S7（风险 R8）：SEO 路由（sitemap.xml / feed.xml / robots.txt）。
//!
//! 从 `main.rs` 拆出。此前 sitemap 与 feed 各自复制了一份
//! `collect_board!` 宏与条目收集逻辑——新增内容板块时漏改一处就会造成
//! 「sitemap 有、feed 无」的不一致。现统一为 [`collect_content_entries`]，
//! 两个端点共用同一收集函数。

use axum::routing::get;
use axum::Router;
use widgets::ContentEntry;

/// 收集全部启用模块的内容条目（blog + 5 个内容板块）。
///
/// sitemap 与 feed 共用：按 `site.json::modules.<id>.enabled` 过滤，
/// 新增内容板块只需在这里加一行。
async fn collect_content_entries(is_on: impl Fn(&str) -> bool) -> Vec<ContentEntry> {
  let mut entries: Vec<ContentEntry> = Vec::new();

  if is_on("blog") {
    let posts = module_blog::server::list_blog_posts().await.unwrap_or_default();
    entries.extend(posts.into_iter().map(|p| ContentEntry {
      url_path: format!("/blog/{}", p.slug),
      title: p.title,
      description: p.description,
      date: p.date,
      tags: p.tags,
    }));
  }

  // 内容板块条目，按开关收录。
  for b in [
    module_embedded::BOARD,
    module_ai::BOARD,
    module_web3::BOARD,
    module_wasm::BOARD,
    module_cli::BOARD,
  ] {
    if !is_on(b.id) {
      continue;
    }
    for a in board::server::list_board_articles(b.id.to_string()).await.unwrap_or_default() {
      entries.push(ContentEntry {
        url_path: format!("{}/{}", b.route, a.slug),
        title: a.title,
        description: a.description,
        date: a.date,
        tags: a.tags,
      });
    }
  }

  entries
}

/// 没有日期的详情页（文档 / 案例 / 课程详情）：只进 sitemap（不带 lastmod），不进 feed。
/// 课时是付费内容，不收录。
async fn collect_detail_paths(is_on: impl Fn(&str) -> bool) -> Vec<String> {
  let mut paths = Vec::new();
  if is_on("docs") {
    paths.extend(doc_paths(&module_docs::server::list_doc_tree().await.unwrap_or_default()));
  }
  if is_on("cases") {
    let cases = module_cases::server::list_cases(None, None, None).await.unwrap_or_default();
    paths.extend(cases.into_iter().map(|c| format!("/case/{}", c.slug)));
  }
  #[cfg(feature = "course")]
  if is_on("course") {
    let courses = module_course::server::list_courses().await.unwrap_or_default();
    paths.extend(courses.into_iter().map(|c| format!("/course/{}", c.slug)));
  }
  paths
}

/// 文档树里有正文的节点 → `/docs/<path>`（深度优先，父在子前）。
fn doc_paths(nodes: &[module_docs::server::DocTreeNode]) -> Vec<String> {
  let mut out = Vec::new();
  for n in nodes {
    if n.has_content && !n.path.is_empty() {
      out.push(format!("/docs/{}", n.path));
    }
    out.extend(doc_paths(&n.children));
  }
  out
}

fn xml_response(content_type: &str, cache_control: &str, body: String) -> axum::response::Response {
  axum::response::Response::builder()
    .header("content-type", content_type)
    .header("cache-control", cache_control)
    .body(axum::body::Body::from(body))
    .unwrap_or_else(|_| axum::response::Response::new(axum::body::Body::empty()))
}

async fn sitemap_handler(base: String) -> axum::response::Response {
  use widgets::build_sitemap_xml;

  // Phase 3.4：按模块开关过滤静态路径与内容条目。
  let module_engine = app_core::engines::module::default_module_engine();
  let enabled = module_engine.enabled_ids();
  let is_on = |id: &str| enabled.iter().any(|s| s == id);

  let entries = collect_content_entries(is_on).await;

  // 静态路径：首页恒收录；其它模块从 ModuleSpec.static_path 自动获取。
  // Phase 8.7：加新模块只动 ModuleSpec.static_path 即可在 sitemap 出现。
  let mut static_paths: Vec<String> = vec!["/".to_string()];
  for spec in module_engine.enabled_modules() {
    if let Some(p) = spec.static_path.as_deref() {
      static_paths.push(p.to_string());
    }
  }
  static_paths.extend(collect_detail_paths(is_on).await);
  let static_paths_ref: Vec<&str> = static_paths.iter().map(String::as_str).collect();

  let xml = build_sitemap_xml(&entries, &static_paths_ref, &base);
  // Phase 8.5：sitemap 1 小时 Cache-Control，让爬虫与 CDN 不至于
  // 每次请求都打满 list_* hot path。`public` 表示中间代理也能缓存。
  xml_response("application/xml; charset=utf-8", "public, max-age=3600", xml)
}

async fn feed_handler(base: String) -> axum::response::Response {
  use widgets::build_atom_feed;

  // Phase 3.4：blog 关闭时输出空 feed，但保留站点元信息。
  let module_engine = app_core::engines::module::default_module_engine();
  let enabled = module_engine.enabled_ids();
  let is_on = |id: &str| enabled.iter().any(|s| s == id);

  let mut entries = collect_content_entries(is_on).await;

  // 全站按日期降序，取最近 50 篇。
  entries.sort_by(|a, b| b.date.cmp(&a.date));
  entries.truncate(50);

  // 取站点元信息：如取不到 site.json 则走默认。S10：mtime 缓存读取。
  let cfg = app_core::settings::SiteConfig::load_cached(
    app_core::utils::get_asset_root().join("site.json").to_str().unwrap_or_default(),
  )
  .unwrap_or_default();
  let xml = build_atom_feed(&entries, &cfg.site_name, &cfg.site_description, &base);
  // Phase 8.5：同 sitemap，feed 1 小时 Cache-Control 缓解 RSS reader 轮询压力。
  xml_response("application/atom+xml; charset=utf-8", "public, max-age=3600", xml)
}

async fn robots_handler(base: String) -> axum::response::Response {
  let body = widgets::build_robots_txt(&base);
  // robots.txt 几乎不变，6h 缓存。
  xml_response("text/plain; charset=utf-8", "public, max-age=21600", body)
}

/// 挂载 SEO 路由。`base_url` 为站点对外 URL（无尾斜杠）。
pub fn mount(router: Router, base_url: &str) -> Router {
  let sitemap_base = base_url.to_string();
  let feed_base = base_url.to_string();
  let robots_base = base_url.to_string();
  router
    .route(
      "/sitemap.xml",
      get(move || {
        let base = sitemap_base.clone();
        async move { sitemap_handler(base).await }
      }),
    )
    .route(
      "/feed.xml",
      get(move || {
        let base = feed_base.clone();
        async move { feed_handler(base).await }
      }),
    )
    .route(
      "/robots.txt",
      get(move || {
        let base = robots_base.clone();
        async move { robots_handler(base).await }
      }),
    )
}

#[cfg(test)]
mod tests {
  use super::*;
  use module_docs::server::DocTreeNode;

  fn node(path: &str, has_content: bool, children: Vec<DocTreeNode>) -> DocTreeNode {
    DocTreeNode {
      slug: path.rsplit('/').next().unwrap_or_default().to_string(),
      title: path.to_string(),
      path: path.to_string(),
      has_content,
      description: String::new(),
      children,
    }
  }

  #[test]
  fn doc_paths_keeps_only_nodes_with_content() {
    let tree = vec![
      node(
        "basics",
        true,
        vec![node("basics/ownership", true, vec![]), node("basics/wip", false, vec![])],
      ),
      node("empty-dir", false, vec![node("empty-dir/leaf", true, vec![])]),
    ];
    assert_eq!(
      doc_paths(&tree),
      vec!["/docs/basics", "/docs/basics/ownership", "/docs/empty-dir/leaf"]
    );
  }

  /// 验收（O4）：sitemap 的详情页条目 = 各来源计数之和，用真实 assets 独立计数比对。
  #[tokio::test]
  async fn detail_paths_cover_every_doc_case_and_course() {
    let root = app_core::utils::get_asset_root();
    let count_dirs = |sub: &str| {
      std::fs::read_dir(root.join(sub))
        .map(|rd| {
          rd.flatten()
            .filter(|e| {
              e.path().is_dir() && !e.file_name().to_string_lossy().starts_with(['_', '.'])
            })
            .count()
        })
        .unwrap_or(0)
    };
    let docs = doc_paths(&module_docs::server::list_doc_tree().await.unwrap_or_default()).len();
    let index_files = walk_count(&root.join("docs"));
    assert!(docs > 0);
    assert_eq!(docs, index_files, "文档树漏掉了磁盘上的文档（深度上限 / _meta.json 过滤？）");

    let paths = collect_detail_paths(|_| true).await;
    let courses = if cfg!(feature = "course") { count_dirs("courses") } else { 0 };
    assert_eq!(paths.len(), docs + count_dirs("cases") + courses);
    assert_eq!(collect_detail_paths(|_| false).await, Vec::<String>::new());
  }

  /// 非根目录下 index.md(x) 的数量（根 index 是 /docs 本身，已在静态路径里）。
  fn walk_count(dir: &std::path::Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten()
      .map(|e| {
        let p = e.path();
        if p.is_dir() {
          let own = ["index.md", "index.mdx"].iter().any(|f| p.join(f).is_file()) as usize;
          own + walk_count(&p)
        } else {
          0
        }
      })
      .sum()
  }
}
