# Phase 1 补充设计 — 架构修正

> 基于 POC 验证和设计讨论，对 Phase 1 Exec Plan 的修正和补充。
> CC 执行时以本文为准，与原 exec plan 冲突的部分以本文为准。

---

## 一、Prompt 三层架构

Agent 收到的 prompt 由 entrypoint.sh 拼接三层内容：

```
层 A: 平台系统规则（scripts/system-rules.md）
     └ CCodeBoX 固定注入，所有任务共享
     └ 内容：容器行为规范、产出要求、禁止事项
     └ 用户不需要知道这些存在

层 B: 项目规范（仓库的 AGENTS.md，可选）
     └ 如果 clone 下来的 repo 有 AGENTS.md，读取并拼入
     └ 内容：项目特定的 build/test/lint 命令、架构说明
     └ 不含任何 CCodeBoX 特定的东西

层 C: 用户任务（TASK_PROMPT 环境变量）
     └ 用户在 UI 上提交的 prompt
```

拼接结果就是 `claude --print` 的参数（一个字符串），是 agent 收到的第一句也是唯一一句话。

**system-rules.md 已存在于 `scripts/system-rules.md`，直接读取使用。**

---

## 二、entrypoint.sh 重写

当前 entrypoint.sh 有过重的 wrapper 逻辑（硬编码 ruff/pytest、多轮重试循环）。需要重写为精简的 orchestrator + collector。

### 新的四阶段

```
Phase 1: Setup
  ├── 读环境变量（AGENT_TYPE, TASK_PROMPT, REPO_URL, BRANCH, CC_MODEL, ...）
  ├── git clone repo（如果有 REPO_URL）或 git init
  ├── git checkout -b task-${TASK_ID:-$(date +%s)}
  ├── 自动装依赖（检测 requirements.txt / package.json / Cargo.toml）
  └── mkdir -p .loop

Phase 2: Prompt Assembly
  ├── 读 scripts/system-rules.md → SYSTEM_RULES
  ├── 读 AGENTS.md（如果存在）→ PROJECT_RULES
  ├── 读 TASK_PROMPT 环境变量 → USER_TASK
  └── 拼接: FULL_PROMPT = SYSTEM_RULES + PROJECT_RULES + USER_TASK

Phase 3: Run Agent（一次调用，agent 自己迭代）
  ├── claude-code: claude --print --dangerously-skip-permissions --model $CC_MODEL "$FULL_PROMPT"
  ├── codex: codex exec --full-auto --approval-policy never "$FULL_PROMPT"
  └── 捕获 stdout → .loop/agent.log，记录退出码

Phase 4: Collect（agent 退出后，平台收集产物）
  ├── git add -A（排除 .loop/）
  ├── git diff --cached → .loop/diff.patch
  ├── 统计变更：files_changed, lines_added, lines_removed
  ├── 读 .loop/summary.md（agent 写的）
  ├── 写 .loop/report.json（结构化报告）
  ├── git commit -m "task: $TASK_TITLE"
  └── git push origin $BRANCH（如果有 REPO_URL）
```

### 删除的逻辑
- ❌ 多轮重试 loop（agent 自己迭代，不需要外部驱动）
- ❌ 硬编码 ruff check / pytest / npm test（测试是 agent 的事）
- ❌ Wrapper 判断测试通过/失败（agent 自行决定何时完成）

### report.json 新格式

```json
{
    "agent_type": "claude-code",
    "model": "claude-opus-4-6",
    "agent_exit_code": 0,
    "has_summary": true,
    "files_changed": ["calc.py", "tests/test_calc.py"],
    "lines_added": 119,
    "lines_removed": 0,
    "branch": "task-abc123",
    "duration_seconds": 87
}
```

不再包含 lint_status / test_status / verify_passed / rounds——这些是旧 wrapper 验证的产物，现在测试由 agent 负责。

---

## 三、Codex CLI 支持

entrypoint.sh 的 Phase 3 需要支持两种 agent：

```bash
if [ "$AGENT_TYPE" = "claude-code" ]; then
    claude --print \
        --dangerously-skip-permissions \
        --model "${CC_MODEL:-claude-sonnet-4-20250514}" \
        "$FULL_PROMPT" > "$LOOP_DIR/agent.log" 2>&1

elif [ "$AGENT_TYPE" = "codex" ]; then
    codex exec \
        --full-auto \
        --approval-policy never \
        "$FULL_PROMPT" > "$LOOP_DIR/agent.log" 2>&1
fi
```

对应需要新增 Codex agent 镜像：

```
images/
├── base/Dockerfile           # 基础镜像（node + python + git + tools）
├── claude-code/Dockerfile    # base + @anthropic-ai/claude-code
└── codex/Dockerfile          # base + @openai/codex (NEW)
```

Codex Dockerfile:
```dockerfile
FROM ccodebox-base:latest
RUN npm install -g @openai/codex
COPY scripts/entrypoint.sh /entrypoint.sh
COPY scripts/system-rules.md /system-rules.md
RUN chmod 755 /entrypoint.sh
USER agent
ENTRYPOINT ["/entrypoint.sh"]
```

Codex 需要的环境变量：`OPENAI_API_KEY`、`OPENAI_BASE_URL`（可选）。

---

## 四、system-rules.md 打包到镜像

system-rules.md 需要在容器内可读。两种方式：

**方式 A（推荐）：构建时 COPY 到镜像**
```dockerfile
COPY scripts/system-rules.md /system-rules.md
```
entrypoint.sh 读 `/system-rules.md`。

**方式 B：运行时挂载**
```bash
podman run -v ./scripts/system-rules.md:/system-rules.md:ro ...
```

推荐方式 A，规则随镜像版本走，不会出现版本不一致。

---

## 五、Git 操作完全由平台管理

agent prompt 中明确禁止执行 git 命令。entrypoint.sh 在 agent 退出后处理：

```
Setup 阶段：
  git clone → git checkout -b task-{id}

Collect 阶段：
  git add -A
  git diff --cached → .loop/diff.patch
  git commit -m "task: {title}"
  git push origin {branch}  （仅当 REPO_URL 存在时）
```

push 需要认证。通过环境变量注入 token：
```bash
git clone https://${GITHUB_TOKEN}@github.com/user/repo.git
```

---

## 六、后端 API 变更

### 6.1 Task 模型变更

删除旧的 wrapper 验证字段，新增字段：

```diff
- lint_status TEXT,
- test_status TEXT,
- rounds_used INTEGER,
+ agent_exit_code INTEGER,
+ branch TEXT,
+ duration_seconds INTEGER,
+ pushed BOOLEAN DEFAULT false,
```

### 6.2 Settings API 新增 Codex

GET /api/settings 返回值新增 codex agent：

```json
{
    "agents": [
        {
            "type": "claude-code",
            "name": "Claude Code",
            "image": "ccodebox-cc:latest",
            "models": ["claude-opus-4-6", "claude-sonnet-4-20250514"]
        },
        {
            "type": "codex",
            "name": "Codex CLI",
            "image": "ccodebox-codex:latest",
            "models": ["codex-mini", "o4-mini"]
        }
    ]
}
```

### 6.3 容器环境变量注入

后端 `container/manager.rs` 创建容器时，根据 agent_type 注入不同的环境变量：

| 环境变量 | claude-code | codex |
|---------|-------------|-------|
| AGENT_TYPE | claude-code | codex |
| TASK_PROMPT | ✅ | ✅ |
| REPO_URL | ✅ | ✅ |
| BRANCH | ✅ | ✅ |
| CC_MODEL | ✅ | — |
| ANTHROPIC_AUTH_TOKEN | ✅ | — |
| ANTHROPIC_BASE_URL | ✅ | — |
| OPENAI_API_KEY | — | ✅ |
| GITHUB_TOKEN | ✅（push 用） | ✅ |

---

## 七、前端变更

### 7.1 任务详情页

- 删除 lint_status / test_status 展示
- 新增 agent_exit_code 展示（0=正常退出, 非0=异常）
- 新增 branch 展示
- 新增 duration 展示
- 如果 pushed=true，显示"查看分支"链接

### 7.2 新建任务表单

- Agent 下拉新增 "Codex CLI" 选项
- 选择 Codex 时 Model 下拉切换为 codex-mini / o4-mini

---

## 八、镜像构建

需要构建三个镜像：

```bash
# 1. 基础镜像
docker build -t ccodebox-base:latest -f images/base/Dockerfile images/base/

# 2. Claude Code agent
docker build -t ccodebox-cc:latest -f images/claude-code/Dockerfile .

# 3. Codex agent
docker build -t ccodebox-codex:latest -f images/codex/Dockerfile .
```

注意：claude-code 和 codex 的 Dockerfile 需要 COPY entrypoint.sh 和 system-rules.md，所以 build context 是项目根目录。

---

## 九、Phase 2 预告（本期不做）

以下功能在 Phase 1 不实现，记录在此供后续参考：

1. **GitHub Issue → Task 自动填充**：webhook 监听 issue 创建，自动填入 Task
2. **PR 自动创建**：push branch 后调 GitHub API 创建 PR
3. **Review Agent**：PR 创建后启动新容器 + review agent 审核代码
4. **git worktree**：支持同一 repo 多任务并发（各自独立 worktree）
5. **Browser 测试镜像**：base-browser 镜像含 Playwright + Chromium
6. **实时日志流**：WebSocket 推送容器日志到前端

---

## 十、CC 执行顺序

1. 重写 `scripts/entrypoint.sh`（四阶段：Setup → Prompt Assembly → Run Agent → Collect）
2. 新建 `images/codex/Dockerfile`
3. 修改 `images/claude-code/Dockerfile`（COPY system-rules.md）
4. 修改 `images/base/Dockerfile`（如有需要）
5. 修改后端 Task 模型（删旧字段、加新字段）
6. 修改后端 container/manager.rs（新的环境变量注入逻辑）
7. 修改后端 settings API（新增 codex agent）
8. 修改前端任务详情页（适配新字段）
9. 修改前端新建任务表单（新增 codex 选项）
10. 全部改完后 `cargo check && cargo clippy` + `cd frontend && npm run build`
