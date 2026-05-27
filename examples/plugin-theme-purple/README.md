# plugin-theme-purple (Example)

> Phase 5.2.1 示例。配合 [`docs/PLUGIN_DEV.md`](../../docs/PLUGIN_DEV.md) 阅读。

紫罗兰主题插件示例：~30 行 Rust 实现一个主题，演示完整的 WASM 插件
开发 / 构建 / 部署流程。

## 构建

```bash
# 从仓库根目录执行
CARGO_TARGET_DIR=/Users/hal/.target cargo build \
  -p plugin-theme-purple \
  --target wasm32-unknown-unknown \
  --release
```

## 部署

```bash
cp /Users/hal/.target/wasm32-unknown-unknown/release/plugin_theme_purple.wasm \
   assets/plugins/
```

然后编辑 `assets/site.json`，把它加进主题栈：

```jsonc
{
  "themes": ["plugin_theme_purple.wasm"]
}
```

刷新页面即可看到紫色主题生效。ThemePicker 下拉也会自动列出本插件
（manifest capability=`theme` 被识别）。

## 单测

```bash
cargo test -p plugin-theme-purple
```

在 host 环境验证 manifest 字段、调色板、必需 CSS 变量，与 wasm runtime
解耦。

## 卸载

```bash
rm assets/plugins/plugin_theme_purple.wasm
# 然后从 site.json::themes 中移除该项
```
