# Phase 1 Exec Plan — CCodeBoX

> 目标：用户可以通过 Web UI 提交编码任务，平台在容器中调度 Claude Code 执行，实时展示状态，任务完成后查看报告和产出。

## 数据模型

### Task (SQLite)

```sql
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,  -- UUID
    title       TEXT NOT NULL,
    prompt      TEXT NOT NULL,     -- 用户的任务描述
    repo_url    TEXT,              -- 可选，要操作的 git 仓库
    branch      TEXT,              -- 可选，分支名
    agent_type  TEXT NOT NULL DEFAULT 'claude-code',  -- 'claude-code' | 'codex'
    model       TEXT NOT NULL DEFAULT 'claude-sonnet-4-20250514',
    max_rounds  INTEGER NOT NULL DEFAULT 3,
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending|running|success|failed|cancelled
    container_id TEXT,             -- Docker/Podman container ID
    rounds_used INTEGER DEFAULT 0,
    lint_status TEXT,              -- pass|fail|skipped
    test_status TEXT,              -- pass|fail|skipped
    lines_added INTEGER DEFAULT 0,
    lines_removed INTEGER DEFAULT 0,
    files_changed TEXT,            -- 逗号分隔
    summary     TEXT,              -- agent 生成的摘要 (summary.md 内容)
    diff_patch  TEXT,              -- git diff 内容
    error       TEXT,              -- 失败时的错误信息
    created_at  TEXT NOT NULL,     -- ISO8601
    started_at  TEXT,
    finished_at TEXT
);
```

### AgentConfig (运行时配置，不入库)

```rust
struct AgentConfig {
    agent_type: AgentType,      // ClaudeCode | Codex
    model: String,
    api_base_url: String,       // 从环境变量或平台配置读取
    api_key: String,            // 从环境变量读取，不入库
    image: String,              // Docker 镜像名
}
```

## API 设计

### POST /api/tasks
创建任务并启动容器执行。

Request:
```json
{
    "title": "实现计算器模块",
    "prompt": "创建 calc.py ...",
    "repo_url": "https://github.com/user/repo",  // 可选
    "branch": "feat/calc",                         // 可选
    "agent_type": "claude-code",                   // 可选，默认 claude-code
    "model": "claude-opus-4-6",                    // 可选，默认 sonnet
    "max_rounds": 3                                // 可选，默认 3
}
```

Response: `201 Created`
```json
{
    "id": "uuid",
    "status": "pending",
    "created_at": "2026-03-26T10:00:00Z"
}
```

创建后立即异步启动容器（tokio::spawn）。

### GET /api/tasks
列出所有任务。

Query: `?status=running&limit=20&offset=0`

Response:
```json
{
    "tasks": [{ ... }],
    "total": 42
}
```

### GET /api/tasks/:id
获取任务详情。

Response: 完整 Task 对象。

### GET /api/tasks/:id/logs
获取 agent 日志（各轮次拼接）。

Response:
```json
{
    "logs": "Round 1 output...\n---\nRound 2 output...",
    "rounds": 2
}
```

### POST /api/tasks/:id/cancel
取消运行中的任务（kill 容器）。

### GET /api/settings
获取平台配置（可用 agent、模型列表等）。

Response:
```json
{
    "agents": [
        {
            "type": "claude-code",
            "name": "Claude Code",
            "image": "ccodebox-cc:latest",
            "models": ["claude-opus-4-6", "claude-sonnet-4-20250514"]
        }
    ],
    "default_model": "claude-sonnet-4-20250514",
    "max_rounds_limit": 5
}
```

## 容器编排流程

```
POST /api/tasks
    │
    ├─ 1. 写入 SQLite (status=pending)
    │
    ├─ 2. tokio::spawn 异步任务
    │     │
    │     ├─ 3. bollard: create container
    │     │     image: ccodebox-cc:latest
    │     │     env: [ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL, CC_MODEL,
    │     │           TASK_PROMPT, MAX_ROUNDS, REPO_URL, BRANCH]
    │     │     host_config: memory_limit, cpu_quota
    │     │
    │     ├─ 4. 更新 DB (status=running, container_id, started_at)
    │     │
    │     ├─ 5. bollard: start container
    │     │
    │     ├─ 6. bollard: wait container (阻塞直到退出)
    │     │
    │     ├─ 7. bollard: cp from container  (/workspace/.loop/*)
    │     │     - report.json → 解析并更新 DB
    │     │     - summary.md → task.summary
    │     │     - diff.patch → task.diff_patch
    │     │     - agent-round-*.log → 存日志
    │     │
    │     ├─ 8. 更新 DB (status=success|failed, finished_at, ...)
    │     │
    │     └─ 9. bollard: remove container (cleanup)
    │
    └─ 返回 201 (task.id)
```

## 前端页面

### 1. 任务列表页 (/)
- 顶部：「新建任务」按钮
- 列表：卡片式，每张卡片显示 title + status badge + agent + model + 创建时间
- 状态筛选：全部 / 运行中 / 成功 / 失败
- 轮询刷新：运行中的任务每 3 秒刷新

### 2. 新建任务页 (/tasks/new)
- 表单字段：
  - Title (必填)
  - Prompt (必填，多行 textarea)
  - Repo URL (选填)
  - Branch (选填)
  - Agent 选择 (下拉，从 /api/settings 获取)
  - Model 选择 (下拉，联动 Agent)
  - Max Rounds (滑块，1-5)
- 提交后跳转到任务详情页

### 3. 任务详情页 (/tasks/[id])
- 顶部：标题 + 状态 badge + 时间线（创建→开始→完成）
- Tab 切换：
  - **概览**: 配置信息 + report 数据（轮次、lint/test 状态、代码变更量）
  - **日志**: 滚动日志查看器，实时刷新
  - **Diff**: 代码 diff 展示（syntax highlighted）
  - **摘要**: agent 生成的 summary.md（markdown 渲染）
- 运行中时显示「取消」按钮

### 样式
- 暗色主题（参考 GitHub Dark）
- 主色：#3B82F6 (蓝) 用于主操作
- 状态色：pending=灰, running=蓝(动画), success=绿, failed=红, cancelled=黄
- 字体：JetBrains Mono (代码), Inter (UI)
- 响应式：桌面优先，最小宽度 768px

## 后端配置

通过环境变量配置（.env 文件）：

```env
# 服务
CCODEBOX_HOST=0.0.0.0
CCODEBOX_PORT=3000
DATABASE_URL=sqlite:./data/ccodebox.db

# Agent: Claude Code
CC_IMAGE=ccodebox-cc:latest
CC_API_BASE_URL=https://zenmux.ai/api/anthropic
CC_API_KEY=sk-...  # 平台级 API key

# 容器资源限制
CONTAINER_MEMORY_LIMIT=4294967296  # 4GB
CONTAINER_CPU_QUOTA=200000         # 2 cores

# Docker socket (podman 兼容)
DOCKER_HOST=unix:///var/run/docker.sock
```

## 实施步骤

按以下顺序执行，每步完成后验证：

### Step 1: 项目脚手架
- `cargo init backend`
- Cargo.toml 加依赖: axum, tokio, serde, sqlx(sqlite), bollard, anyhow, thiserror, uuid, chrono, tracing, tracing-subscriber, tower-http(cors)
- `npx create-next-app@latest frontend` (TypeScript, Tailwind, App Router)
- 验证: `cd backend && cargo check` + `cd frontend && npm run build`

### Step 2: 数据库层
- 创建 SQLite 迁移文件 (tasks 表)
- 实现 db 模块: init_db(), create_task(), get_task(), list_tasks(), update_task()
- 验证: `cargo check`

### Step 3: 容器管理器
- 实现 container::manager 模块
- ContainerManager: new(), run_task() → 封装 create/start/wait/cp/remove
- 解析 report.json, 读取 summary.md 和 diff.patch
- 验证: `cargo check`

### Step 4: API 层
- 实现所有 HTTP handlers (tasks CRUD + settings + cancel)
- 接入 DB 和 ContainerManager
- 启动异步任务的 tokio::spawn 逻辑
- 验证: `cargo check && cargo clippy`

### Step 5: 集成测试
- `cargo run` 启动后端
- 手动 curl 测试所有 API
- 确认容器能启动、report 能收集
- 验证: 端到端一个任务跑通

### Step 6: 前端 — 布局 + 任务列表
- 全局 layout (暗色主题, 侧边栏/顶栏)
- 任务列表页: 从 API 获取，卡片展示
- StatusBadge 组件
- 验证: `npm run build`

### Step 7: 前端 — 新建任务
- TaskForm 组件 (全部表单字段)
- 从 /api/settings 获取 agent/model 选项
- 提交 → POST /api/tasks → 跳转详情
- 验证: `npm run build`

### Step 8: 前端 — 任务详情
- TaskDetail: 概览 tab (report 数据)
- LogViewer: 日志滚动 + 轮询刷新
- Diff 展示 (用 react-diff-viewer 或类似库)
- Summary: markdown 渲染
- 取消按钮
- 验证: `npm run build`

### Step 9: 收尾
- README.md (项目介绍 + 快速开始)
- .env.example
- 检查所有 `cargo clippy` + `npm run build` 通过
- 清理临时代码
