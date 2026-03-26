# CCodeBoX — Base Image
# Shared foundation for all agent images
FROM node:22-slim

# China mirror for apt
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources 2>/dev/null || \
    sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list 2>/dev/null || true

# System tools
RUN apt-get update && apt-get install -y \
    git python3 python3-pip python3-venv curl wget jq \
    && rm -rf /var/lib/apt/lists/*

# Python dev tools (verification)
RUN pip3 install --break-system-packages \
    -i https://mirrors.aliyun.com/pypi/simple/ \
    --trusted-host mirrors.aliyun.com \
    ruff pytest

# npm China mirror
RUN npm config set registry https://registry.npmmirror.com

# Non-root user (required by Claude Code)
RUN useradd -m -s /bin/bash agent && \
    mkdir -p /workspace && chown agent:agent /workspace

WORKDIR /workspace
