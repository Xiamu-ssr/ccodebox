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
│     浏览器（Playwright + Chromium）           │
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

### 3.1 浏览器操作（Playwright + Chromium）

**安装**（Dockerfile）：

```dockerfile
# L2: 浏览器
RUN npx playwright install --with-deps chromium
```

**Agent 怎么用**：Agent 直接写 Playwright 脚本执行。所有 agent 都能跑 bash，所以都能调 Playwright。

此外，提供两个薄封装 CLI 脚本简化常用操作：

**scripts/ccodebox-browser**（装到镜像的 /usr/local/bin/）：

```javascript
#!/usr/bin/env node
// ccodebox-browser — 浏览器快捷操作 CLI
// 用法:
//   ccodebox-browser snapshot <url>      → 输出 AX Tree（JSON）
//   ccodebox-browser screenshot <url>    → 截图保存到 .loop/screenshots/

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const [,, command, url] = process.argv;

(async () => {
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    await page.goto(url, { waitUntil: 'networkidle' });

    if (command === 'snapshot') {
        const tree = await page.accessibility.snapshot();
        console.log(JSON.stringify(tree, null, 2));
    } else if (command === 'screenshot') {
        const dir = '.loop/screenshots';
        fs.mkdirSync(dir, { recursive: true });
        const file = path.join(dir, `${Date.now()}.png`);
        await page.screenshot({ path: file, fullPage: true });
        console.log(`Screenshot saved: ${file}`);
    }

    await browser.close();
})();
```

**Dockerfile 安装**：

```dockerfile
COPY scripts/ccodebox-browser /usr/local/bin/ccodebox-browser
RUN chmod +x /usr/local/bin/ccodebox-browser
```

### 3.2 网页搜索（Tavily）

**scripts/tavily-search**（装到镜像的 /usr/local/bin/）：

```bash
#!/bin/bash
# tavily-search — 网页搜索 CLI
# 用法: tavily-search "your query"
# 需要环境变量: TAVILY_API_KEY

set -euo pipefail

if [ -z "${1:-}" ]; then
    echo "用法: tavily-search \"your query\""
    exit 1
fi

if [ -z "${TAVILY_API_KEY:-}" ]; then
    echo "错误: TAVILY_API_KEY 未设置"
    exit 1
fi

curl -s https://api.tavily.com/search \
    -H "Content-Type: application/json" \
    -d "{\"query\":\"$1\",\"api_key\":\"$TAVILY_API_KEY\",\"max_results\":5}" \
    | python3 -c "
import sys, json
data = json.load(sys.stdin)
for i, r in enumerate(data.get('results', []), 1):
    print(f'{i}. {r[\"title\"]}')
    print(f'   {r[\"url\"]}')
    print(f'   {r[\"content\"][:200]}')
    print()
"
```

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
- 浏览器操作：容器内已安装 Playwright + Chromium（headless 模式）
  - 写 Node.js 或 Python 脚本调用 Playwright API 进行完整的浏览器自动化
  - 快捷命令：`ccodebox-browser snapshot <url>` 获取页面 AX Tree（JSON 格式）
  - 快捷命令：`ccodebox-browser screenshot <url>` 截图保存到 .loop/screenshots/
- 网页搜索：`tavily-search "你的查询"` 返回 Top 5 搜索结果
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

1. 重写 `images/base/Dockerfile`（bookworm + L2 能力 + CLI 工具脚本）
2. 创建 `scripts/ccodebox-browser`（浏览器 CLI 封装）
3. 创建 `scripts/tavily-search`（搜索 CLI 封装）
4. 更新 `images/claude-code/Dockerfile`（基于新 base）
5. 创建 `images/codex/Dockerfile`
6. 更新 `scripts/system-rules.md`（加入可用工具说明）
7. 新增 `platform_config` 表 + DB 操作方法
8. 新增 `GET/PUT /api/settings` + `POST /api/settings/test-*`
9. 更新 `container/manager.rs`（从 config 读 key 注入环境变量）
10. 新增前端 `/settings` 页面
11. 更新前端创建任务表单（去掉 API key 输入，从 settings 自动读取）
