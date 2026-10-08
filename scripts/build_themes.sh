#!/usr/bin/env bash
# 一键构建 wasm 插件并复制到 assets/plugins/。主题已内置进宿主（R2），
# 这里只剩 content-transformer 示例插件。
#
# 使用方式：
#   ./scripts/build_themes.sh                 # 构建全部
#   ./scripts/build_themes.sh content-toc     # 仅构建 content-toc
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

# 短名匹配规则：去掉 `-plugin` 后缀（content-toc-plugin → content-toc）。
ALL_CONTENT_TRANSFORMERS=(
    "content-toc-plugin:content_toc_plugin.wasm"
)

# 合并所有支持的插件入口。`-` 前缀短名仍按子类别匹配。
ALL_PLUGINS=("${ALL_CONTENT_TRANSFORMERS[@]}")

# 解析参数：缺省构建全部，否则按短名匹配。
SELECTED=()
if [ "$#" -eq 0 ]; then
    SELECTED=("${ALL_PLUGINS[@]}")
else
    for arg in "$@"; do
        matched=0
        for entry in "${ALL_PLUGINS[@]}"; do
            crate="${entry%%:*}"
            short="${crate%-plugin}"
            if [ "$arg" = "$short" ] || [ "$arg" = "$crate" ]; then
                SELECTED+=("$entry")
                matched=1
                break
            fi
        done
        if [ "$matched" -eq 0 ]; then
            echo "错误：未识别的插件参数: $arg" >&2
            echo "可用 content-transformer：content-toc" >&2
            exit 1
        fi
    done
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
