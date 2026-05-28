# syntax=docker/dockerfile:1.7-labs
#
# Phase 7.4.1：多阶段 Debian (glibc) 镜像。
#
# 设计：
# - **builder** 阶段在 `rust:1-trixie` 中完成 Tailwind CSS 编译 + WASM 主题
#   插件构建 + `dx bundle --platform web --release` 的产物输出。
# - **runtime** 阶段是 `debian:trixie-slim` + `ca-certificates`，只装 dx bundle
#   产物。builder 与 runtime 同为 trixie，glibc 版本对齐，避免动态链接的 server
#   二进制在运行阶段报 "version `GLIBC_x.xx' not found"。
# - 通过 `CARGO_TARGET_DIR=/tmp/target` 覆盖仓库根 `.cargo/config.toml` 中开
#   发者本地路径（`/Users/hal/.target`），避免在容器内尝试写不可达路径。
#
# 为什么是 Debian 而非 Alpine：`dx bundle` 在 release 模式下会下载 GitHub 上
# **预编译的 glibc** 二进制（wasm-bindgen-cli / wasm-opt）来生成 wasm 胶水。
# 这些 glibc ELF 在 musl(Alpine) 上执行时因缺少 `ld-linux` 解释器而以
# "No such file or directory (os error 2)" 失败（"Failed to generate
# wasm-bindgen bindings"）。glibc 基础镜像可直接运行这些工具，绕开整类问题。
# Tailwind v4 需要 Node ≥ 20，trixie 自带 nodejs 20，亦满足。
#
# 构建：
#   docker build -t rustineverything:latest .
#
# 运行（最小）：
#   docker run --rm -p 8080:8080 \
#     -e DATABASE_URL=postgres://user:pass@postgres:5432/app \
#     -e JWT_SECRET=$(openssl rand -hex 32) \
#     -e BASE_URL=https://example.com \
#     rustineverything:latest
#
# `docker-compose.yml` 会把 postgres + 本 app 串起来。

# ─────────────────────────────────────────────────────────────
# Stage 1：builder
# ─────────────────────────────────────────────────────────────
FROM rust:1-trixie AS builder

# 关闭 .cargo/config.toml 中的开发者本地 target 路径
ENV CARGO_TARGET_DIR=/tmp/target \
    CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    RUST_BACKTRACE=short \
    DEBIAN_FRONTEND=noninteractive

# 系统依赖（rust:1-trixie 基于 buildpack-deps，已含 gcc/make/git/perl/bash）：
# - nodejs/npm：Tailwind CSS v4 编译 (`npm run build`)，trixie nodejs 为 20.x
# - pkg-config + libssl-dev：dioxus-cli 依赖 openssl-sys；glibc 下动态链接，
#   只需头文件 + 动态库，无需静态库。仅 builder 阶段用，runtime 不含。
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      nodejs \
      npm \
      pkg-config \
      libssl-dev \
 && rm -rf /var/lib/apt/lists/*

# 添加 wasm 目标，用于插件编译
RUN rustup target add wasm32-unknown-unknown

# 安装 Dioxus CLI（fullstack bundle 工具）。
# **必须与 Cargo.lock 中 dioxus 库版本精确一致**（当前 0.7.5）：dx CLI 负责生成
# wasm-bindgen 胶水，版本不匹配会报 "dx and dioxus versions are incompatible!"
# 并在 bundle 阶段以 "Failed to generate wasm-bindgen bindings" 失败。`^0.7` 会
# 解析到最新 0.7.9，与 0.7.5 库不兼容；故锁定 `=0.7.5`。升级 dioxus 时同步改这里。
RUN cargo install dioxus-cli --locked --version "=0.7.5" --no-default-features

WORKDIR /workspace

# 第一遍：仅 COPY manifest，预热依赖缓存。
# （Cargo workspace + 多 crate；逐个 stub 较繁琐，这里只复制 manifest 与
# lock，把 src 留到下一层，让 Docker 把 deps 缓存与代码层分开。）
COPY Cargo.toml Cargo.lock ./
COPY rustfmt.toml ./
COPY .cargo ./.cargo
COPY crates/sdk/Cargo.toml crates/sdk/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/llm/Cargo.toml crates/llm/Cargo.toml
COPY crates/widgets/Cargo.toml crates/widgets/Cargo.toml
COPY crates/app/Cargo.toml crates/app/Cargo.toml
COPY crates/app/build.rs crates/app/build.rs
COPY crates/app/Dioxus.toml crates/app/Dioxus.toml
COPY crates/app/package.json crates/app/package.json
COPY crates/app/package-lock.json crates/app/package-lock.json
COPY crates/app/tailwind-input.css crates/app/tailwind-input.css
COPY crates/migration/Cargo.toml crates/migration/Cargo.toml
COPY crates/modules ./crates/modules
COPY crates/plugins ./crates/plugins
COPY examples ./examples
# build.rs 引用 ../../assets，提供占位避免预热阶段 panic
RUN mkdir -p assets crates/app/assets

# 安装 Tailwind 工具链（独立缓存层；package-lock 不变时跳过重装）
RUN cd crates/app && npm ci --no-audit --no-fund

# COPY 剩余源码
COPY crates/sdk/src crates/sdk/src
COPY crates/core/src crates/core/src
COPY crates/llm/src crates/llm/src
COPY crates/widgets/src crates/widgets/src
COPY crates/migration/src crates/migration/src
COPY crates/app/src crates/app/src
COPY scripts ./scripts
COPY assets ./assets
# `build.rs` 期望根 assets 同步到 crates/app/assets
RUN cp -r assets/* crates/app/assets/ 2>/dev/null || true

# 1. 编译 Tailwind CSS（产物落到 crates/app/assets/tailwind.css）
RUN cd crates/app && npm run build

# 2. 构建全部主题 WASM 插件，输出到 assets/plugins/
RUN bash scripts/build_themes.sh

# 3. dx bundle：web 全栈 + release 优化，
#    产物在 /tmp/target/dx/rustineverything-app/release/web/{public,server}
RUN cd crates/app && dx bundle --platform web --release --package rustineverything-app

# 4. 收敛产物到统一目录，方便 runtime 阶段单层 COPY
RUN mkdir -p /out \
 && cp -r /tmp/target/dx/rustineverything-app/release/web/public /out/public \
 && cp /tmp/target/dx/rustineverything-app/release/web/server /out/server \
 && cp -r assets /out/assets


# ─────────────────────────────────────────────────────────────
# Stage 2：runtime
# ─────────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

# 最小运行时依赖：
# - ca-certificates：HTTPS（OAuth / 上游 API）
# - tini：PID 1 信号转发，避免 zombies / Ctrl-C 不响应
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tini \
 && rm -rf /var/lib/apt/lists/*

# 创建专用非 root 用户
RUN groupadd --system app && useradd --system --gid app --no-create-home app

WORKDIR /app

COPY --from=builder --chown=app:app /out/server /app/server
COPY --from=builder --chown=app:app /out/public /app/public
COPY --from=builder --chown=app:app /out/assets /app/assets

USER app

# Dioxus fullstack 默认监听 0.0.0.0:8080
ENV PORT=8080 \
    IP=0.0.0.0 \
    RUST_LOG=info

EXPOSE 8080

# tini 处理信号；server 自身就是入口可执行文件
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/app/server"]
