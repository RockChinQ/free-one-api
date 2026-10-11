# DS_WebAgent 容器镜像:容器里跑一个真实(有头)Chromium,
# 用 Xvfb 提供虚拟显示,再用 noVNC 给你一个能在浏览器里点的手动登录入口。
#
# 单独构建/运行:
#   docker build -t ds-webagent ./ds_web_agent
#   docker run -d --name ds-webagent -p 9178:9178 -p 6080:6080 \
#     -e DSWA_API_TOKEN=local-dev-token -v dswebagent-data:/data ds-webagent
#
# 首次使用:打开 http://localhost:6080/vnc.html 在容器里的 Chromium 中手动登录 DeepSeek,
# 登录态存在 /data/browser-profile,之后重启容器不用再登。

# ---------- 构建阶段 ----------
FROM rust:1-bookworm AS builder

WORKDIR /build

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests
COPY docs ./docs

RUN cargo build --release

# ---------- 运行阶段 ----------
FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive \
    DISPLAY=:99 \
    DSWA_CONFIG=/app/config.docker.toml \
    DSWA_API_TOKEN=local-dev-token \
    DSWA_NO_SANDBOX=true \
    DSWA_CHROME_EXECUTABLE=/usr/bin/chromium \
    DSWA_USER_DATA_DIR=/data/browser-profile \
    DSWA_STORAGE_DIR=/data/logs \
    DSWA_SQLITE_PATH=/data/dswa.db

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        chromium \
        xvfb \
        x11vnc \
        novnc \
        websockify \
        fonts-noto-cjk \
        tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/ds_webagent /app/ds_webagent
COPY config.docker.toml /app/config.docker.toml
COPY docker-entrypoint.sh /app/docker-entrypoint.sh

RUN chmod +x /app/docker-entrypoint.sh \
    && mkdir -p /data/logs /data/browser-profile

EXPOSE 9178 6080

ENTRYPOINT ["/usr/bin/tini", "--", "/app/docker-entrypoint.sh"]
