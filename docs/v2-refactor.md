# V2 重构设计 — 去容器化、本地 Agent 编排

> **核心变更**：从"容器内运行 agent"转为"调用用户电脑上已安装的 agent"。
> 砍掉 Docker/Bollard 依赖，新增 Agent Adapter 层 + git worktree 工作区管理。
> 前端、DB、API 框架保留，按需改造。

---

## 一、架构对比

### 旧版（容器化）

```
Frontend → Backend → Docker Container (agent + entrypoint.sh)
                         └── 文件系统隔离、全包
```

### 新版（本地 agent）

```
Frontend/CLI → Backend → Adapter → 用户电脑上的 agent（CC / Codex / ...）
                  │
                  └── git worktree 管理工作区
```

---

## 二、核心概念

### 2.1 Stage（原子执行单位）

Stage 是最小可运行单位。一个 stage = 一个 agent 执行一个提示词。

```rust
struct Stage {
    id: String,              // UUID
    name: String,            // "coding", "testing", ...
    agent_type: AgentType,   // ClaudeCode / Codex / ...
    prompt_template: String, // 支持 ${input} 变量
    needs_branch: bool,      // 是否需要 git 分支
    context_from: Option<String>,  // 继承哪个 stage 的产出作为上下文
}
```

**Stage 可以独立运行**（单次调用模式），也可以作为 TaskType 编排的一部分运行。

### 2.2 TaskType（编排模板）

TaskType 定义 stage 的流转顺序和失败策略。用 YAML 或 DB 存储。

```yaml
# 内置模板：feature-dev
name: feature-dev
description: "开发新功能：编码 → 测试 → PR"
inputs:
  - name: requirement
    description: "功能需求描述"
    required: true

stages:
  - name: coding
    agent: claude-code
    needs_branch: true
    prompt: |
      基于以下需求进行开发：
      ${requirement}

  - name: testing
    agent: claude-code
    needs_branch: false
    context_from: coding
    prompt: |
      对当前代码改动进行测试，确保功能正确。
      如果发现问题，输出详细的错误报告。
    on_fail:
      goto: coding
      carry: error_report

  - name: pr
    agent: none  # 平台自动执行，不需要 agent
    auto: true
    action: create_pr
```

### 2.3 Task（任务实例）

用户选择 TaskType + 填写 inputs → 创建 Task 实例。

```rust
struct Task {
    id: String,
    project_id: String,
    task_type: String,        // "feature-dev"
    inputs: HashMap<String, String>,  // {"requirement": "实现用户注册..."}
    status: TaskStatus,       // pending / running / success / failed
    current_stage: String,    // 当前在哪个 stage
    created_at: DateTime<Utc>,
}
```

### 2.4 StageRun（Stage 执行记录）

每次 stage 执行产生一条记录，包含产出物。

```rust
struct StageRun {
    id: String,
    task_id: String,
    stage_name: String,
    run_number: i32,          // 第几次运行（重试时递增）
    agent_type: AgentType,
    status: StageRunStatus,   // running / success / failed
    workspace_path: String,   // 工作目录路径
    branch: Option<String>,   // git 分支名（如果 needs_branch）
    agent_exit_code: Option<i32>,
    agent_log: Option<String>,
    diff_patch: Option<String>,
    summary: Option<String>,
    duration_seconds: Option<i32>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}
```

---

## 三、工作区管理

### 3.1 目录结构

```
~/.ccodebox/                          ← CCodeBox 根目录
  config.toml                         ← 全局配置
  data/
    ccodebox.db                       ← SQLite
  workspaces/
    {project-name}/
      {task-id}--{stage-name}--{run}/  ← 每个 stage run 一个目录
```

示例：

```
~/.ccodebox/workspaces/
  my-app/
    t001--coding--1/                  ← task-001 第一次 coding（git worktree）
    t001--testing--1/                 ← task-001 第一次 testing（普通目录或复用 coding 的 worktree）
    t001--coding--2/                  ← task-001 coding 重试（新 worktree，基于上次分支继续）
    t002--coding--1/                  ← task-002 并行执行
```

### 3.2 git worktree 管理

```rust
/// 工作区管理器
struct WorkspaceManager {
    base_dir: PathBuf,  // ~/.ccodebox/workspaces
}

impl WorkspaceManager {
    /// 为 stage run 创建工作目录
    /// needs_branch=true → git worktree add
    /// needs_branch=false → 创建普通目录（或 symlink 到 context_from 的 worktree）
    async fn create_workspace(&self, stage_run: &StageRun, project: &Project) -> Result<PathBuf>;

    /// 清理工作目录
    async fn cleanup_workspace(&self, stage_run: &StageRun) -> Result<()>;
}
```

**git worktree 流程（needs_branch=true）**：

```bash
# 1. 确保有 base repo
#    如果用户配了本地路径 → 直接用
#    如果只有 GitHub URL → clone 到 ~/.ccodebox/repos/{project-name}/
base_repo = project.local_path or clone(project.repo_url)

# 2. 创建 worktree + 分支
cd $base_repo
git worktree add ~/.ccodebox/workspaces/{project}/{task}--{stage}--{run} -b {task-id}

# 3. agent 在 worktree 目录里工作

# 4. 完成后收集 diff、push
cd worktree_dir
git add -A && git diff --cached > diff.patch
git commit -m "task: {title} [{stage}]"
git push origin {task-id}  # 可选
```

**不需要分支的 stage**：

```bash
# context_from 指定了上游 stage → 直接在上游的 worktree 目录里工作
# 或者创建普通目录，把需要的文件复制/链接过去
```

### 3.3 Project 注册

```rust
struct Project {
    id: String,
    name: String,
    repo_url: Option<String>,       // GitHub URL
    local_path: Option<String>,     // 本地已有 repo 路径
    default_agent: Option<AgentType>,
    created_at: DateTime<Utc>,
}
```

```bash
# CLI 注册项目
ccodebox project add --name my-app --path ~/Code/my-app
ccodebox project add --name my-app --repo https://github.com/xxx/my-app
```

---

## 四、Agent Adapter 层

### 4.1 Adapter trait

```rust
#[async_trait]
trait AgentAdapter: Send + Sync {
    /// 启动 agent 执行任务，返回子进程 handle
    async fn execute(&self, request: AgentRequest) -> Result<AgentHandle>;

    /// 检查 agent 是否已安装
    async fn check_installed(&self) -> Result<bool>;

    /// agent 名称
    fn name(&self) -> &str;
}

struct AgentRequest {
    prompt: String,
    working_dir: PathBuf,
    model: Option<String>,
    env: HashMap<String, String>,  // API keys etc.
}

struct AgentHandle {
    child: tokio::process::Child,
    log_path: PathBuf,
}

struct AgentResult {
    exit_code: i32,
    stdout: String,
    duration_seconds: i32,
}
```

### 4.2 Claude Code Adapter

```rust
struct ClaudeCodeAdapter;

#[async_trait]
impl AgentAdapter for ClaudeCodeAdapter {
    async fn execute(&self, req: AgentRequest) -> Result<AgentHandle> {
        let mut cmd = tokio::process::Command::new("claude");
        cmd.args([
            "--print",
            "--output-format", "json",
            "--permission-mode", "bypassPermissions",
        ]);
        if let Some(model) = &req.model {
            cmd.args(["--model", model]);
        }
        cmd.arg(&req.prompt);
        cmd.current_dir(&req.working_dir);
        cmd.envs(&req.env);

        // stdout → log file
        let log_path = req.working_dir.join(".ccodebox-agent.log");
        let log_file = File::create(&log_path).await?;
        cmd.stdout(log_file);
        cmd.stderr(Stdio::piped());

        let child = cmd.spawn()?;
        Ok(AgentHandle { child, log_path })
    }

    async fn check_installed(&self) -> Result<bool> {
        Ok(Command::new("claude").arg("--version").status().await?.success())
    }

    fn name(&self) -> &str { "claude-code" }
}
```

### 4.3 Codex Adapter

```rust
struct CodexAdapter;

#[async_trait]
impl AgentAdapter for CodexAdapter {
    async fn execute(&self, req: AgentRequest) -> Result<AgentHandle> {
        let mut cmd = tokio::process::Command::new("codex");
        cmd.args(["exec"]);
        if let Some(model) = &req.model {
            cmd.args(["-c", &format!("model=\"{model}\"")]);
        }
        cmd.arg(&req.prompt);
        cmd.current_dir(&req.working_dir);
        cmd.envs(&req.env);

        let log_path = req.working_dir.join(".ccodebox-agent.log");
        let log_file = File::create(&log_path).await?;
        cmd.stdout(log_file);

        let child = cmd.spawn()?;
        Ok(AgentHandle { child, log_path })
    }

    async fn check_installed(&self) -> Result<bool> {
        Ok(Command::new("codex").arg("--version").status().await?.success())
    }

    fn name(&self) -> &str { "codex" }
}
```

---

## 五、Stage 执行引擎

### 5.1 单 Stage 执行流程

```
1. 创建 StageRun 记录（DB，status=running）
2. 创建工作目录（WorkspaceManager）
3. 组装 prompt（模板 + inputs + context_from 上游产出）
4. 调用 AgentAdapter.execute()
5. 等待 agent 完成（async wait）
6. 收集产出：
   - git diff（如果 needs_branch）
   - agent log
   - exit code
7. 更新 StageRun 记录（status=success/failed）
8. 返回结果
```

### 5.2 Task 编排执行流程

```
1. 创建 Task 记录（DB，status=running）
2. 读取 TaskType 定义
3. 按 stages 顺序执行：
   for stage in task_type.stages:
     result = execute_stage(stage)
     if result.failed:
       if stage.on_fail.goto:
         jump to stage.on_fail.goto（携带 error_report）
         retry_count++
         if retry_count > max_retries: task.failed
       else:
         task.failed
     else:
       continue to next stage
4. 所有 stages 通过 → task.success
5. 执行 auto actions（如 create_pr）
```

---

## 六、DB Schema 变更

### 6.1 新增表

```sql
-- 项目注册
CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    repo_url    TEXT,
    local_path  TEXT,
    default_agent TEXT,
    created_at  TEXT NOT NULL
);

-- 任务类型模板（内置 + 用户自定义）
CREATE TABLE task_types (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    definition  TEXT NOT NULL,  -- YAML 内容
    builtin     BOOLEAN DEFAULT false,
    created_at  TEXT NOT NULL
);

-- Stage 执行记录
CREATE TABLE stage_runs (
    id              TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL REFERENCES tasks(id),
    stage_name      TEXT NOT NULL,
    run_number      INTEGER NOT NULL DEFAULT 1,
    agent_type      TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    workspace_path  TEXT,
    branch          TEXT,
    prompt_used     TEXT,          -- 实际发给 agent 的 prompt
    agent_exit_code INTEGER,
    agent_log       TEXT,
    diff_patch      TEXT,
    summary         TEXT,
    error_report    TEXT,          -- 失败时的报告（传给下一轮 coding）
    duration_seconds INTEGER,
    created_at      TEXT NOT NULL,
    finished_at     TEXT
);
```

### 6.2 修改 tasks 表

```sql
-- 新增字段
ALTER TABLE tasks ADD COLUMN project_id TEXT REFERENCES projects(id);
ALTER TABLE tasks ADD COLUMN task_type TEXT NOT NULL DEFAULT 'single-stage';
ALTER TABLE tasks ADD COLUMN inputs TEXT;          -- JSON: {"requirement": "..."}
ALTER TABLE tasks ADD COLUMN current_stage TEXT;

-- 保留的字段：id, title, prompt, status, created_at, started_at, finished_at, error
-- 移到 stage_runs 的字段（任务级不再需要）：
--   container_id → 删除
--   agent_exit_code → stage_runs
--   duration_seconds → stage_runs（任务级可算总和）
--   pushed → stage_runs
--   lines_added/removed → stage_runs
--   files_changed → stage_runs
--   summary → stage_runs
--   diff_patch → stage_runs
--   agent_type → 每个 stage 可能用不同 agent
--   model → 同上
--   repo_url / branch → 移到 projects
```

---

## 七、API 变更

### 7.1 新增

```
# 项目
POST   /api/projects                     创建/注册项目
GET    /api/projects                     列出项目
GET    /api/projects/:id                 项目详情
DELETE /api/projects/:id                 删除项目

# 任务类型
GET    /api/task-types                   列出任务类型模板
GET    /api/task-types/:name             模板详情（含 inputs 定义）

# Stage 单独运行
POST   /api/run                          单次 stage 运行（最简模式）
  Body: { project_id, agent_type, prompt, model? }

# Stage 执行记录
GET    /api/tasks/:id/stages             获取 task 的所有 stage runs
GET    /api/stage-runs/:id               单个 stage run 详情
GET    /api/stage-runs/:id/log           stage agent 日志
```

### 7.2 修改

```
POST /api/tasks
  旧 Body: { title, prompt, repo_url, agent_type, model }
  新 Body: { title, project_id, task_type, inputs: { requirement: "..." } }

GET /api/tasks/:id
  返回值新增 stages: StageRun[] 字段
```

### 7.3 删除

```
GET /api/settings/images         ← 不再需要镜像管理
POST /api/settings/images/build  ← 同上
```

---

## 八、CLI 界面

```bash
# 项目管理
ccodebox project add --name my-app --path ~/Code/my-app
ccodebox project add --name my-app --repo https://github.com/xxx/my-app
ccodebox project list

# 单 stage 运行（最简模式，v1 核心）
ccodebox run --project my-app --agent claude-code "实现用户注册功能"
ccodebox run --project my-app --agent codex --model o3 "修复 #123 bug"

# 任务编排运行（v1.5）
ccodebox task create --project my-app --type feature-dev
  > requirement: "实现用户注册功能，支持邮箱和手机号"
ccodebox task list --project my-app
ccodebox task status <task-id>

# Agent 检查
ccodebox agent list          # 列出已安装的 agent
ccodebox agent check         # 检查所有 adapter 可用性

# 服务
ccodebox serve               # 启动 Web 服务（前端 + API）
ccodebox version
```

---

## 九、要砍掉的代码

| 文件/模块 | 动作 | 原因 |
|-----------|------|------|
| `container/manager.rs` | **重写** → `adapter/` 模块 | 不再用 Docker |
| `container/images.rs` | **删除** | 不再管镜像 |
| `container/mod.rs` | **删除** | 整个 container 模块替换为 adapter |
| `images/` 目录 | **删除** | Dockerfile 不需要了 |
| `scripts/entrypoint.sh` | **删除** | 不再有容器入口脚本 |
| `scripts/system-rules.md` | **保留改造** → prompt 模板 | 仍需给 agent prompt 注入规范 |
| `scripts/tavily-search` | **保留** | 可作为本地工具，agent 通过 CLI 调用 |
| `Cargo.toml` 中 `bollard` | **删除** | 不再依赖 Docker SDK |

---

## 十、要保留/改造的代码

| 文件/模块 | 动作 |
|-----------|------|
| `main.rs` | 改造：去掉 BollardRuntime，换成 AdapterRegistry |
| `config.rs` | 改造：去掉容器相关配置，加 workspace/project 配置 |
| `contracts.rs` | 改造：新增 Project/TaskType/StageRun 类型，修改 Task/CreateTaskRequest |
| `entity/task.rs` | 改造：字段变更（见 DB Schema） |
| `entity/` | 新增：project.rs, task_type.rs, stage_run.rs |
| `api/tasks.rs` | 改造：适配新模型 |
| `api/` | 新增：projects.rs, stage_runs.rs |
| `db/mod.rs` | 改造：新增表的 CRUD |
| `frontend.rs` | 保留（单二进制打包逻辑不变） |
| 前端 | 改造 UI 适配新数据模型（后做） |

---

## 十一、新增模块

```
backend/src/
  adapter/
    mod.rs           ← AdapterRegistry, AgentAdapter trait
    claude_code.rs   ← CC adapter
    codex.rs         ← Codex adapter
  workspace/
    mod.rs           ← WorkspaceManager
    worktree.rs      ← git worktree 操作封装
  engine/
    mod.rs           ← StageExecutor, TaskOrchestrator
    stage.rs         ← 单 stage 执行逻辑
    task.rs          ← 多 stage 编排逻辑（v1.5）
  entity/
    project.rs       ← NEW
    stage_run.rs     ← NEW
```

---

## 十二、Prompt 组装

保留三层架构思想，但适配本地运行：

```
层 A: 平台规范（scripts/system-rules.md，精简版）
     └ 不再含容器相关规则
     └ 保留：产出规范、git 操作禁止（平台管 git）

层 B: 项目规范（项目 repo 中的 AGENTS.md / CLAUDE.md）
     └ 从 worktree 目录中读取

层 C: 用户需求（task.inputs + stage.prompt_template 渲染结果）
     └ 模板变量替换后的最终 prompt

拼接：final_prompt = A + B + C
```

如果 stage 有 `context_from`，还会拼入上游 stage 的产出摘要：

```
层 D: 上游 Stage 产出（context_from stage 的 diff/summary/error_report）
拼接：final_prompt = A + B + D + C
```

---

## 十三、CC 执行计划

### Phase A：基础重构（优先级最高）

1. 删除 `container/` 模块、`images/` 目录、`scripts/entrypoint.sh`
2. Cargo.toml 删除 `bollard` 依赖
3. 新建 `adapter/` 模块（trait + CC adapter + Codex adapter）
4. 新建 `workspace/` 模块（WorkspaceManager + git worktree 操作）
5. 修改 `main.rs`：AppState 从 `ContainerRuntime` 泛型改为持有 `AdapterRegistry`
6. 确保 `cargo check` 通过

### Phase B：数据模型

7. 新增 entity：project.rs, stage_run.rs
8. 修改 entity/task.rs（新增字段、删除容器相关字段）
9. 修改 contracts.rs（新增 DTO）
10. 修改 db/mod.rs（新表 CRUD + migration）
11. `cargo check && cargo test`

### Phase C：执行引擎

12. 新建 `engine/` 模块
13. 实现 `engine/stage.rs`（单 stage 执行流程）
14. 修改 `api/tasks.rs`（适配新模型）
15. 新增 `api/projects.rs`
16. 新增 `POST /api/run`（单 stage 运行 API）
17. `cargo check && cargo test`

### Phase D：CLI

18. 新增 CLI 子命令解析（project add/list, run, agent list/check）
19. CLI 直接调用 engine 执行（不走 HTTP API）
20. 验证：`ccodebox run --project my-app --agent claude-code "test prompt"` 跑通

### Phase E：前端适配（可后做）

21. 修改任务创建页面（project 选择 + inputs 表单）
22. 修改任务详情页（显示 stage runs）
23. 新增项目管理页面
24. `npm run build`

---

## 十四、验收标准

### v1（单 stage）

- [ ] `ccodebox agent check` 显示已安装的 agent
- [ ] `ccodebox project add --name test --path ~/Code/test-repo` 成功
- [ ] `ccodebox run --project test --agent claude-code "add a hello world function"` 成功
  - [ ] 自动创建 git worktree
  - [ ] agent 在 worktree 中执行
  - [ ] 完成后收集 diff、log
  - [ ] worktree 目录保留供查看
- [ ] `ccodebox run --project test --agent codex "fix the bug"` 成功
- [ ] `ccodebox serve` 启动后，Web UI 可创建和查看任务
- [ ] 两个任务可以并行执行（不同 worktree，互不干扰）

### v1.5（task workflow，本期不做）

- [ ] `ccodebox task create --project test --type feature-dev` 触发多 stage 编排
- [ ] coding 失败后自动回退重试
- [ ] 全部通过后自动 PR
