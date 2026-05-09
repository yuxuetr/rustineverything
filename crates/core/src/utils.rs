//! 公共工具函数，跨 app / module 重用。

use std::path::PathBuf;

/// 自动探测资产根目录。
///
/// 启动方式不同（cargo run / dx serve / cargo test），运行目录可能是
/// 项目根目录（`./assets` 存在）或 crate 子目录（需上溯 `../../assets`）。
/// 调用方无需关心：本函数会先查相对路径 `assets`，否则退回到 `../../assets`。
///
/// 返回的 `PathBuf` 不保证存在；调用方需根据上下文做存在性检查。
pub fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        path = PathBuf::from("../../assets");
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_a_path_buf() {
        // 不能假设具体内容（依赖运行目录），只验证返回 PathBuf 即可
        let p = get_asset_root();
        // 一定是 "assets" 或 "../../assets" 之一
        let s = p.to_string_lossy();
        assert!(s == "assets" || s == "../../assets");
    }
}
