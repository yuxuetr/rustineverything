//! SEC-07 / B1：站点 CSP 不含 `'unsafe-eval'`，`document::eval` 与
//! `document::Title`（其客户端 `set_title` 内部也是 eval）一调用 wasm 即崩溃。
//! 浏览器操作统一走 `widgets::browser`（web-sys），这里防止它们回到源码里。

use std::path::Path;

const FORBIDDEN: &[&str] = &["document::eval", "document::Title"];
/// `browser.rs` 的 `PageTitle` 只在服务端渲染 `document::Title`；本文件列着模式本身。
const ALLOWED_FILES: &[&str] = &["widgets/src/browser.rs", "app/tests/csp_no_eval.rs"];

fn scan(dir: &Path, hits: &mut Vec<String>) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      scan(&path, hits);
    } else if path.extension().is_some_and(|e| e == "rs")
      && !ALLOWED_FILES.iter().any(|f| path.ends_with(f))
    {
      let src = std::fs::read_to_string(&path).unwrap_or_default();
      for (n, line) in src.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for pat in FORBIDDEN {
          if code.contains(pat) {
            hits.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
          }
        }
      }
    }
  }
}

#[test]
fn no_eval_based_document_apis_in_site_crates() {
  let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
  let mut hits = Vec::new();
  scan(&crates, &mut hits);
  assert!(hits.is_empty(), "use widgets::browser instead:\n{}", hits.join("\n"));
}
