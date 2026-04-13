use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // 获取当前 crate 的目录 (crates/app)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let app_assets_path = Path::new(&manifest_dir).join("assets");
    
    // 根目录下的 assets 路径 (../../assets)
    let root_assets_path = Path::new(&manifest_dir).join("../..").join("assets");

    println!("cargo:rerun-if-changed=../../assets");

    if root_assets_path.exists() {
        // 如果 crates/app/assets 已经存在且不是软链接，先删除它（避免冲突）
        if app_assets_path.exists() && !app_assets_path.is_symlink() {
            let _ = fs::remove_dir_all(&app_assets_path);
        }

        // 如果软链接不存在，则创建它
        if !app_assets_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                if let Err(e) = symlink(&root_assets_path, &app_assets_path) {
                    eprintln!("警告: 无法创建 Unix 软链接: {}", e);
                }
            }

            #[cfg(windows)]
            {
                use std::os::windows::fs::symlink_dir;
                if let Err(e) = symlink_dir(&root_assets_path, &app_assets_path) {
                    eprintln!("警告: 无法创建 Windows 软链接: {}. 请尝试以管理员身份运行或开启开发者模式。", e);
                }
            }
        }
    }
}
