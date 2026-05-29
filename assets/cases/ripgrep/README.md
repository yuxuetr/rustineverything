# ripgrep
ripgrep（`rg`）是用 Rust 写的递归文本搜索工具，以惊人的速度成为众多开发者 grep 的默认替代。

## 看点
- 基于 Rust `regex` crate 的有限自动机引擎，避免灾难性回溯
- 默认尊重 `.gitignore` 并跳过二进制文件
- 并行目录遍历，多核满载
- 已集成进 VS Code 的全局搜索

## 适合参考
想学「如何用 Rust 写出又快又正确的 CLI」——内存映射、并行遍历、正则优化——ripgrep 是教科书级范例。
