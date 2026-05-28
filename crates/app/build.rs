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

  // Reverse sync: if crates/app/assets/tailwind.css is newer (Tailwind compile output),
  // copy it back to root/assets/tailwind.css so the SoT stays up-to-date.
  let app_tw = app_assets.join("tailwind.css");
  let root_tw = root_assets.join("tailwind.css");
  if app_tw.exists() {
    let should_copy = if root_tw.exists() {
      let app_mod = fs::metadata(&app_tw).and_then(|m| m.modified()).ok();
      let root_mod = fs::metadata(&root_tw).and_then(|m| m.modified()).ok();
      match (app_mod, root_mod) {
        (Some(a), Some(r)) => a > r,
        _ => true,
      }
    } else {
      true
    };
    if should_copy {
      let _ = fs::copy(&app_tw, &root_tw);
    }
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
      // 拷贝小文件
      let _file_name = path.file_name().unwrap().to_str().unwrap();
      // 过滤掉超大文件
      if let Ok(metadata) = fs::metadata(&path) {
        if metadata.len() > 10 * 1024 * 1024 {
          // > 10MB
          continue;
        }
      }

      // 始终执行拷贝，确保插件更新能实时反映
      let _ = fs::copy(&path, &dest_path);
    }
  }
}
