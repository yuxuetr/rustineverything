# rie-gateway 镜像（阶段 V）：在 CI 里构建，cu 上只拉镜像。
# 用 `network_mode: host` 运行：监听宿主的 80 / 443，反代到宿主 127.0.0.1:8080（app 容器绑定的端口）。
FROM rust:1-trixie AS builder
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev cmake \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:trixie-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3t64 tini \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/rie-gateway /usr/local/bin/rie-gateway
ENV RUST_LOG=info
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/rie-gateway"]
