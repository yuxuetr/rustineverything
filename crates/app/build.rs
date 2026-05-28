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
    sync_dir(&root_assets, &app_assets);
  }

  // Reverse sync: 若 crates/app/assets/tailwind.css 与 root 内容不同（Tailwind
  // 编译产物），回写到 root/assets/tailwind.css 让 SoT 保持最新。
  let app_tw = app_assets.join("tailwind.css");
  let root_tw = root_assets.join("tailwind.css");
  if app_tw.exists() {
    copy_if_changed(&app_tw, &root_tw);
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

fn sync_dir(src: &Path, dst: &Path) {
  if !src.exists() {
    return;
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
      sync_dir(&path, &dest_path);
    } else {
      // 过滤掉超大文件
      if let Ok(metadata) = fs::metadata(&path) {
        if metadata.len() > 10 * 1024 * 1024 {
          // > 10MB
          continue;
        }
      }

      // 内容变化才拷贝（幂等），避免无谓 mtime 刷新触发 dx 重建循环。
      copy_if_changed(&path, &dest_path);
    }
  }
}
