# Phase 1 环境工程设计 — Environment Engineering

> Agent 的能力上限由环境决定。镜像不只是"装了什么软件"，而是"agent 能做什么事"。

---

## 一、镜像四层架构

```
┌─────────────────────────────────────────────┐
│ L4: 任务上下文（运行时注入）                  │
│     system-rules.md, AGENTS.md, prompt       │
│     环境变量（API keys, task config）         │
├─────────────────────────────────────────────┤
│ L3: Agent CLI（构建时固定）                   │
│     claude-code 或 codex                     │
├─────────────────────────────────────────────┤
│ L2: 可选能力（构建时内置，默认全开）           │
│     浏览器（agent-browser CLI + Chromium）    │
│     外搜（tavily-search CLI）                │
│     未来可扩展：rust, go, database ...        │
├─────────────────────────────────────────────┤
│ L1: 基础运行时（构建时固定）                  │
│     node:22-bookworm + git + python3 + pip   │
│     curl + wget + jq + build-essential       │
└─────────────────────────────────────────────┘
```

### 各层职责

| 层 | 变更频率 | 谁决定 | 怎么进入容器 |
|---|---------|--------|-------------|
| L1 | 极低（基础 OS 升级） | 平台维护者 | Dockerfile FROM + apt |
| L2 | 低（新增能力时） | 平台维护者 | Dockerfile RUN 安装 |
| L3 | 低（新 agent 版本） | 平台维护者 | Dockerfile npm install |
| L4 | 每次任务都不同 | 用户 + 平台 | 环境变量 + entrypoint 拼接 |

---

## 二、L1 基础镜像

从 `node:22-slim` 升级到 `node:22-bookworm`，给 agent 一个正经的开发环境。

```dockerfile
# images/base/Dockerfile
FROM node:22-bookworm

# 国内镜像加速
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources 2>/dev/null || true

# L1: 基础开发工具
RUN apt-get update && apt-get install -y \
    git python3 python3-pip python3-venv \
    curl wget jq build-essential \
    && rm -rf /var/lib/apt/lists/*

# npm 国内镜像
RUN npm config set registry https://registry.npmmirror.com

# pip 国内镜像
RUN pip3 install --break-system-packages \
    -i https://mirrors.aliyun.com/pypi/simple/ \
    --trusted-host mirrors.aliyun.com \
    ruff pytest
```

---

## 三、L2 内置能力

### 3.1 浏览器操作（agent-browser）

使用 [agent-browser](https://github.com/vercel-labs/agent-browser)——专为 AI agent 设计的浏览器 CLI。基于 Playwright，提供 AX Tree（snapshot）、截图、点击/输入等完整操作，全部通过 CLI 命令完成，无需写脚本。

**安装**（Dockerfile）：

```dockerfile
# L2: 浏览器
RUN npm install -g agent-browser && agent-browser install --with-deps
```

**核心命令**：

```bash
agent-browser open <url>           # 导航到页面
agent-browser snapshot -i          # AX Tree（只含可交互元素，带 @ref 引用）
agent-browser snapshot -i --json   # AX Tree JSON 格式
agent-browser screenshot page.png  # 截图
agent-browser screenshot --full    # 全页截图
agent-browser click @e1            # 点击元素（ref 来自 snapshot）
agent-browser fill @e2 "text"      # 输入文本
agent-browser wait --load networkidle  # 等待页面加载
agent-browser close                # 关闭浏览器
```

**为什么选 agent-browser 而不是 Playwright CLI**：
- Playwright CLI 无 AX Tree 命令，需要写脚本；agent-browser 一行 `snapshot -i` 搞定
- agent-browser 的 `@ref` 系统专为 agent 设计：snapshot 返回元素引用，后续操作直接用引用
- 所有操作都是 CLI 命令，agent 通过 bash 调用，不需要写代码
- 支持 `--json` 输出，方便 agent 解析

### 3.2 网页搜索（Tavily）

**scripts/tavily-search**（纯 Python，零外部依赖，装到镜像的 /usr/local/bin/）：

已存在于 `scripts/tavily-search`，支持多种输出格式：

```bash
# 基本搜索（JSON 输出）
tavily-search --query "playwright headless browser"

# Markdown 格式（适合 agent 阅读）
tavily-search --query "React hydration error" --format md

# 包含 AI 摘要
tavily-search --query "Rust vs Go performance" --include-answer --format md

# 深度搜索
tavily-search --query "SeaORM entity first" --search-depth advanced
```

API key 从环境变量 `TAVILY_API_KEY` 读取，由平台配置中心注入，不硬编码到脚本。

**Dockerfile 安装**：

```dockerfile
COPY scripts/tavily-search /usr/local/bin/tavily-search
RUN chmod +x /usr/local/bin/tavily-search
```

---

## 四、L3 Agent 镜像

每个 agent 基于同一个 base 镜像，只加自己的 CLI：

```dockerfile
# images/claude-code/Dockerfile
FROM ccodebox-base:latest
RUN npm install -g @anthropic-ai/claude-code
COPY scripts/entrypoint.sh /entrypoint.sh
COPY scripts/system-rules.md /system-rules.md
RUN chmod 755 /entrypoint.sh
USER agent
ENTRYPOINT ["/entrypoint.sh"]
```

```dockerfile
# images/codex/Dockerfile
FROM ccodebox-base:latest
RUN npm install -g @openai/codex
COPY scripts/entrypoint.sh /entrypoint.sh
COPY scripts/system-rules.md /system-rules.md
RUN chmod 755 /entrypoint.sh
USER agent
ENTRYPOINT ["/entrypoint.sh"]
```

构建命令：

```bash
# 先构建 base（含 L1 + L2）
docker build -t ccodebox-base:latest -f images/base/Dockerfile .

# 再构建各 agent 镜像（L3）
docker build -t ccodebox-cc:latest -f images/claude-code/Dockerfile .
docker build -t ccodebox-codex:latest -f images/codex/Dockerfile .
```

---

## 五、L4 任务上下文

运行时通过环境变量和 prompt 拼接注入，见 phase1-supplement.md。

---

## 六、Agent 如何发现工具

**不用 MCP，不用 agent 特定配置。统一通过 prompt 告知 + CLI 调用。**

system-rules.md 中的工具说明段落：

```markdown
## 可用工具

### 浏览器操作（agent-browser）
容器内已安装 agent-browser + Chromium（headless 模式）。
- 打开页面：`agent-browser open <url>`
- 获取页面结构：`agent-browser snapshot -i`（返回可交互元素列表，每个元素有 @ref 引用）
- 截图：`agent-browser screenshot <filename>`
- 点击：`agent-browser click @e1`（@e1 来自 snapshot 输出）
- 输入：`agent-browser fill @e2 "text"`
- 等待加载：`agent-browser wait --load networkidle`
- 关闭：`agent-browser close`
- 完整文档：`agent-browser --help`

### 网页搜索（tavily-search）
- 基本搜索：`tavily-search --query "你的查询"` 返回 JSON
- 可读格式：`tavily-search --query "你的查询" --format md`
- 含 AI 摘要：`tavily-search --query "你的查询" --include-answer --format md`
- 深度搜索：`tavily-search --query "你的查询" --search-depth advanced`
```

所有 agent（CC、Codex、未来的 OpenCode 等）都看同一段文字，都通过 bash 调用。零配置差异。

---

## 七、配置中心

API key 和平台配置统一管理，用户配一次，所有任务自动继承。

### 数据模型

```sql
CREATE TABLE platform_config (
    key        TEXT PRIMARY KEY,    -- 点分路径，如 'agent.claude-code.api_key'
    value      TEXT NOT NULL,       -- 值（敏感字段加密存储）
    encrypted  BOOLEAN DEFAULT false,
    updated_at TEXT NOT NULL
);
```

### 配置项

| Key | 说明 | 示例 |
|-----|------|------|
| `agent.claude-code.api_key` | CC 的 API Key | sk-ss-v1-... |
| `agent.claude-code.api_base_url` | CC 的 API 代理地址 | https://zenmux.ai/api/anthropic |
| `agent.claude-code.default_model` | CC 默认模型 | claude-sonnet-4-20250514 |
| `agent.codex.api_key` | Codex 的 API Key | sk-... |
| `agent.codex.default_model` | Codex 默认模型 | codex-mini |
| `tool.tavily.api_key` | Tavily 搜索 API Key | tvly-... |
| `git.github_token` | GitHub Token（push/PR 用） | ghp_... |

### API

```
GET  /api/settings              → 返回所有配置（敏感值脱敏: sk-***38b3）
PUT  /api/settings              → 批量更新配置
POST /api/settings/test-agent   → 测试 agent API key 是否有效
POST /api/settings/test-tool    → 测试工具 API key 是否有效
```

### 前端 Settings 页面

新增 `/settings` 页面：

- **Agent 配置区**：每个 agent 一张卡片（API Key 输入框 + Base URL + 默认模型 + "测试连接"按钮）
- **工具配置区**：Tavily API Key + "测试"按钮
- **Git 配置区**：GitHub Token + "测试"按钮
- 所有敏感输入框用 password 类型，显示时脱敏

### 容器环境变量注入

后端创建容器时，从 platform_config 读取配置，注入为环境变量：

```rust
// container/manager.rs
async fn build_env_vars(&self, task: &Task) -> Vec<String> {
    let config = self.db.get_all_config().await;
    let mut env = vec![
        format!("AGENT_TYPE={}", task.agent_type),
        format!("TASK_PROMPT={}", task.prompt),
    ];

    match task.agent_type {
        AgentType::ClaudeCode => {
            env.push(format!("ANTHROPIC_AUTH_TOKEN={}", config["agent.claude-code.api_key"]));
            env.push(format!("ANTHROPIC_BASE_URL={}", config["agent.claude-code.api_base_url"]));
            env.push(format!("CC_MODEL={}", task.model));
        }
        AgentType::Codex => {
            env.push(format!("OPENAI_API_KEY={}", config["agent.codex.api_key"]));
        }
    }

    // 工具 key（所有 agent 共享）
    if let Some(key) = config.get("tool.tavily.api_key") {
        env.push(format!("TAVILY_API_KEY={}", key));
    }
    if let Some(token) = config.get("git.github_token") {
        env.push(format!("GITHUB_TOKEN={}", token));
    }

    env
}
```

用户创建任务时只需选 agent + 填 prompt，key 全部自动注入。

---

## 八、CC 执行顺序

1. 重写 `images/base/Dockerfile`（bookworm + L2 能力：agent-browser + tavily-search）
2. 创建 `scripts/tavily-search`（搜索 CLI 封装）
4. 更新 `images/claude-code/Dockerfile`（基于新 base）
5. 创建 `images/codex/Dockerfile`
6. 更新 `scripts/system-rules.md`（加入可用工具说明）
7. 新增 `platform_config` 表 + DB 操作方法
8. 新增 `GET/PUT /api/settings` + `POST /api/settings/test-*`
9. 更新 `container/manager.rs`（从 config 读 key 注入环境变量）
10. 新增前端 `/settings` 页面
11. 更新前端创建任务表单（去掉 API key 输入，从 settings 自动读取）
