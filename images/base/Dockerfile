# CCodeBoX — Base Image (L1 + L2)
# Shared foundation for all agent images
#
# Build from repo root:
#   docker build -t ccodebox-base:latest -f images/base/Dockerfile .
FROM node:22-bookworm

# China mirror for apt
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources 2>/dev/null || \
    sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list 2>/dev/null || true

# L1: 基础开发工具
RUN apt-get update && apt-get install -y \
    git python3 python3-pip python3-venv \
    curl wget jq build-essential \
    && rm -rf /var/lib/apt/lists/*

# npm China mirror
RUN npm config set registry https://registry.npmmirror.com

# pip China mirror
RUN pip3 install --break-system-packages \
    -i https://mirrors.aliyun.com/pypi/simple/ \
    --trusted-host mirrors.aliyun.com \
    ruff pytest

# L2: 浏览器操作（agent-browser + Chromium）
RUN npm install -g agent-browser && agent-browser install --with-deps

# L2: 网页搜索（tavily-search CLI）
COPY scripts/tavily-search /usr/local/bin/tavily-search
RUN chmod +x /usr/local/bin/tavily-search

# Non-root user (required by Claude Code)
RUN useradd -m -s /bin/bash agent && \
    mkdir -p /workspace && chown agent:agent /workspace

WORKDIR /workspace
