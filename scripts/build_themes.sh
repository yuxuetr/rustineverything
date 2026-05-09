#!/usr/bin/env bash
# Phase 3.2: 一键构建全部主题插件 wasm 并复制到 assets/plugins/
#
# 使用方式：
#   ./scripts/build_themes.sh             # 构建全部内置主题
#   ./scripts/build_themes.sh sunset      # 仅构建 sunset
#
# 约束：
#   - 构建产物路径强制 /Users/hal/.target（用户规则）
#   - 自动检测并安装 wasm32-unknown-unknown target
#   - 任一插件构建失败立即退出，不掩盖错误

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-/Users/hal/.target}"
WASM_TRIPLE="wasm32-unknown-unknown"
PROFILE="release"
PLUGINS_OUT="$REPO_ROOT/assets/plugins"

# 全部内置主题列表（crate 名 → 输出文件名）
ALL_THEMES=(
    "theme-ocean-plugin:theme_ocean_plugin.wasm"
    "theme-sunset-plugin:theme_sunset_plugin.wasm"
    "theme-catppuccin-plugin:theme_catppuccin_plugin.wasm"
)

# 解析参数：缺省构建全部，否则按短名（去掉 theme- 前缀和 -plugin 后缀）匹配
SELECTED=()
if [ "$#" -eq 0 ]; then
    SELECTED=("${ALL_THEMES[@]}")
else
    for arg in "$@"; do
        for entry in "${ALL_THEMES[@]}"; do
            crate="${entry%%:*}"
            short="${crate#theme-}"
            short="${short%-plugin}"
            if [ "$arg" = "$short" ] || [ "$arg" = "$crate" ]; then
                SELECTED+=("$entry")
                break
            fi
        done
    done
    if [ "${#SELECTED[@]}" -eq 0 ]; then
        echo "错误：未识别的主题参数: $*" >&2
        echo "可用主题: ocean / sunset / catppuccin" >&2
        exit 1
    fi
fi

# 1) 确保 wasm32 target 已安装
if ! rustup target list --installed | grep -q "$WASM_TRIPLE"; then
    echo "[build_themes] 安装 $WASM_TRIPLE target ..."
    rustup target add "$WASM_TRIPLE"
fi

mkdir -p "$PLUGINS_OUT"

# 2) 逐个构建并拷贝
for entry in "${SELECTED[@]}"; do
    crate="${entry%%:*}"
    out_name="${entry##*:}"

    echo "[build_themes] 构建 $crate ..."
    (
        cd "$REPO_ROOT"
        CARGO_TARGET_DIR="$TARGET_DIR" cargo build \
            -p "$crate" \
            --target "$WASM_TRIPLE" \
            --release
    )

    # 注意：cargo build 会把产物放到 $TARGET_DIR/$WASM_TRIPLE/release/<crate_underscored>.wasm
    # crate 名转下划线
    crate_uscore="${crate//-/_}"
    src_wasm="$TARGET_DIR/$WASM_TRIPLE/$PROFILE/${crate_uscore}.wasm"
    dst_wasm="$PLUGINS_OUT/$out_name"

    if [ ! -f "$src_wasm" ]; then
        echo "错误：构建产物不存在: $src_wasm" >&2
        exit 2
    fi
    cp -f "$src_wasm" "$dst_wasm"
    size_bytes=$(stat -f%z "$dst_wasm" 2>/dev/null || stat -c%s "$dst_wasm")
    echo "[build_themes] -> $dst_wasm ($size_bytes 字节)"
done

echo "[build_themes] 完成。"
