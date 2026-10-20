// S9 豁免：build script 内 panic = 构建失败时响亮报错，是可接受惯例。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::env;
use std::fs;
use std::path::Path;

fn main() {
  let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
  let app_assets = Path::new(&manifest_dir).join("assets");
  let root_assets = Path::new(&manifest_dir).join("../..").join("assets");

  println!("cargo:rerun-if-changed=../../assets");

  if root_assets.exists() {
    // 确保物理目录存在
    if !app_assets.exists() {
      let _ = fs::create_dir_all(&app_assets);
    }

    // 同步 UI 必需的静态文件：root/assets → crates/app/assets
    sync_dir(&root_assets, &app_assets, true);

    // audio/ 体积大，sync_dir 不复制；dev 下（cwd = crates/app）`/audio` 静态挂载读
    // 的是这里，没有它就 404。软链指回 root（Docker 构建前已 cp 真实目录，不受影响）。
    #[cfg(unix)]
    {
      let link = app_assets.join("audio");
      if let Ok(target) = root_assets.join("audio").canonicalize() {
        if fs::symlink_metadata(&link).is_err() {
          let _ = std::os::unix::fs::symlink(target, &link);
        }
      }
    }
  }

  // Reverse sync: 若 crates/app/assets/tailwind.css 与 root 内容不同（Tailwind
  // 编译产物），回写到 root/assets/tailwind.css 让 SoT 保持最新。
  let app_tw = app_assets.join("tailwind.css");
  let root_tw = root_assets.join("tailwind.css");
  if app_tw.exists() {
    copy_if_changed(&app_tw, &root_tw);
  }

  // Reverse sync uploads/: dev 时 `dx serve` 可能从 `crates/app/` 启动，写入
  // 落到 `crates/app/assets/uploads/`；下次从仓库根启动 `get_asset_root()`
  // 返回 `assets/`，原本可能找不到那张图（404）。把 app→root 反向同步进来，
  // 保证两份路径下都能读到。两端都 `.gitignore` 该目录，不污染 git。
  let app_uploads = app_assets.join("uploads");
  let root_uploads = root_assets.join("uploads");
  if app_uploads.exists() {
    let _ = fs::create_dir_all(&root_uploads);
    sync_dir(&app_uploads, &root_uploads, false);
  }
}

/// 仅在内容确实变化时拷贝（dest 缺失或字节不同）。
///
/// 这一点很关键：`dx serve` 监听 `crates/app/assets/`，而 build.rs 每次构建
/// 都会跑。如果无条件 `fs::copy`，即便内容没变也会刷新 dest 的 mtime →
/// dx 的文件监听认为「文件改了」→ 触发又一次重建 → build.rs 再跑 → 死循环
/// （表现为开发态频繁弹「app is being rebuilt」）。内容比较让拷贝幂等，
/// 真正变化（如更新插件 wasm / 主题 CSS）仍会同步，从而打破抖动循环。
fn copy_if_changed(src: &Path, dst: &Path) {
  let same = match (fs::read(src), fs::read(dst)) {
    (Ok(s), Ok(d)) => s == d,
    _ => false, // dst 缺失或读失败 → 视为需要拷贝
  };
  if !same {
    let _ = fs::copy(src, dst);
  }
}

/// `prune`：删除 dst 中 src 已不存在的条目（root 里删掉的文档 / 文件不该还被
/// dev server 读到）。`uploads/` 与 `tailwind.css` 的 SoT 在 app 侧，永不删除；
/// 反向同步（app→root uploads）不 prune，否则会删掉 root 侧的用户上传。
fn sync_dir(src: &Path, dst: &Path, prune: bool) {
  if !src.exists() {
    return;
  }

  if prune {
    if let Ok(entries) = fs::read_dir(dst) {
      for entry in entries.flatten() {
        let name = entry.file_name();
        if name == "uploads" || name == "tailwind.css" || src.join(&name).exists() {
          continue;
        }
        let path = entry.path();
        let _ = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
      }
    }
  }

  for entry in fs::read_dir(src).unwrap() {
    let entry = entry.unwrap();
    let path = entry.path();
    let dest_path = dst.join(entry.file_name());

    if path.is_dir() {
      // 跳过一些不需要同步的大型文件夹，由后端 get_content_root 动态处理
      let dir_name = path.file_name().unwrap().to_str().unwrap();
      if dir_name == "audio" || dir_name == "target" || dir_name == "node_modules" {
        continue;
      }

      let _ = fs::create_dir_all(&dest_path);
      sync_dir(&path, &dest_path, prune);
    } else {
      // 过滤掉超大文件
      if let Ok(metadata) = fs::metadata(&path) {
        if metadata.len() > 10 * 1024 * 1024 {
          // > 10MB
          continue;
        }
      }

      // tailwind.css 的 SoT 是 crates/app/assets（`npm run build` 产物），已存在时只做
      // 下方的 app→root 反向同步；否则正向同步会先拿 root 的旧产物盖掉刚编译的新类。
      if entry.file_name() == "tailwind.css" && dest_path.exists() {
        continue;
      }

      // 内容变化才拷贝（幂等），避免无谓 mtime 刷新触发 dx 重建循环。
      copy_if_changed(&path, &dest_path);
    }
  }
}
