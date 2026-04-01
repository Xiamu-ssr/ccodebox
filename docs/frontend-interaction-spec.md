# CCodeBoX 前端交互规范

> 本文档定义所有页面的布局、交互流程和数据来源。
> 设计师/前端开发者据此实现 UI。

---

## 导航结构

```
CCodeBoX（Logo，点击回首页 /projects）
  ├── Projects        /projects          ← 首页/入口
  ├── Templates       /templates         ← 全局编排模板
  ├── Playground      /playground        ← 单 stage 测试
  └── Settings        /settings          ← 全局配置
```

**嵌套路由**：
- `/projects/{id}` → 项目详情（含 task 列表）
- `/projects/{id}/tasks/new` → 创建任务
- `/projects/{id}/tasks/{taskId}` → 任务详情（含 stage runs）

**Task 不再作为顶级导航**，归属于 Project 下。

---

## 页面详情

### 1. Projects 页面 `/projects`

**布局**：项目卡片网格（或列表）

**每张卡片**：
- 项目名（大字）
- 路径或 GitHub URL（小字灰色）
- 任务统计（如 "3 running, 12 completed"）
- 点击 → 进入 `/projects/{id}`

**操作**：
- 右上角 [+ New Project] 按钮
- 每张卡片右下角 [Delete] 小按钮

**New Project 弹窗/表单**：
- Name（必填，输入框）
- GitHub URL（选填，输入框）
- 提交后后端自动处理 local_path
- 不需要用户填本地路径

**数据源**：`GET /api/projects`

---

### 2. Project 详情 `/projects/{id}`

**布局**：

```
┌─ 项目信息栏 ──────────────────────────────────┐
│ 项目名          GitHub URL          创建时间     │
└───────────────────────────────────────────────┘

┌─ 操作栏 ──────────────────────────────────────┐
│ [+ New Task]                     状态筛选 [All ▼] │
└───────────────────────────────────────────────┘

┌─ 任务列表 ──────────────────────────────────────┐
│ task-001  "实现用户注册"  [running]  coding→●testing  │
│ task-002  "修复登录bug"   [success]  ✓coding→✓testing │
│ task-003  "添加日志"      [pending]                    │
└───────────────────────────────────────────────┘
```

**Task 卡片信息**：
- Title
- Status badge（pending/running/success/failed）
- Task Type 名
- Stage 进度条（显示当前在哪个 stage，已完成的打 ✓）
- Duration
- 点击 → 进入任务详情

**数据源**：`GET /api/projects/{id}` + `GET /api/tasks?project_id={id}`
（注意：后端 task 列表需要支持 project_id 筛选，目前可能需要补一个 query param）

---

### 3. New Task `/projects/{id}/tasks/new`

**流程**：3 步

**Step 1：选择编排模板**

显示所有可用模板（卡片选择）：
```
┌──────────────┐  ┌──────────────────────────────┐
│ single-stage │  │ feature-dev                    │
│ 单次执行       │  │ 编码 → 测试 → 完成              │
│              │  │ [coding] → [testing]           │
│              │  │      ↑__fail(max 3)__↓         │
└──────────────┘  └──────────────────────────────┘
```

选中后高亮，显示该模板的流程预览。

**Step 2：填写任务信息**

只显示**必要的**输入：
- Title（必填）
- Requirement / Prompt（必填，textarea，根据模板的 required input 动态显示）

**高级选项**（默认折叠）：
- Agent 类型覆盖（默认用全局 default）
- Model 覆盖（默认用 Settings 里配的 default_model）

**Step 3：确认提交**

显示预览：
- Project: my-app
- Template: feature-dev
- Stages: coding → testing
- Requirement: "..."
- [Create Task]

**数据源**：`GET /api/templates` → `POST /api/tasks`

---

### 4. Task 详情 `/projects/{id}/tasks/{taskId}`

**布局**：

```
┌─ 标题栏 ──────────────────────────────────────┐
│ "实现用户注册"    [running]    Duration: 2m 30s  │
│ feature-dev | Created: 2026-03-31 14:00       │
│                                    [Cancel]    │
└───────────────────────────────────────────────┘

┌─ Stage 流转可视化 ────────────────────────────┐
│ [coding ✓] ──→ [testing ●] ──→ [done]         │
│       ↑                  ↓                     │
│       └── retry #1 ─────┘                     │
└───────────────────────────────────────────────┘

┌─ Stage Runs 时间线 ───────────────────────────┐
│ ● coding run#1    [success]  45s              │
│   └ 展开：Prompt | Log | Diff | Summary       │
│                                               │
│ ● testing run#1   [failed]   30s              │
│   └ 展开：Prompt | Log | Error Report         │
│                                               │
│ ● coding run#2    [success]  60s  (retry)     │
│   └ 展开：Prompt | Log | Diff                  │
│                                               │
│ ● testing run#2   [running]  ...              │
└───────────────────────────────────────────────┘
```

**Stage Run 展开后**：
- Prompt Used（代码块，显示实际发给 agent 的完整 prompt）
- Agent Log（代码块，可滚动）
- Diff（语法高亮的 diff viewer）
- Summary（markdown 渲染）
- Error Report（红色代码块，如果有）
- 元信息：Agent Type、Branch、Workspace Path

**Running 时自动轮询**（每 3 秒 refresh）

**数据源**：`GET /api/tasks/{id}` + `GET /api/tasks/{id}/stages`

---

### 5. Templates 页面 `/templates`

**布局**：模板卡片列表

**每张卡片**：
- 模板名
- 描述
- Stage 流程预览（可视化）
- Builtin 标记（如果是内置的）
- 操作：[View] [Edit]（builtin 的也可以编辑描述，但不能删除）[Delete]（非 builtin）

**操作**：
- [+ New Template] 按钮
- 点 Edit → 打开编辑器（monaco/textarea，编辑 YAML）
- 点 View → 显示 YAML + 流程可视化

**New / Edit Template**：
- Name（创建时填，编辑时只读）
- Description（输入框）
- Definition（YAML 编辑器，大 textarea 或 monaco editor）
- 右侧实时预览解析后的流程

**数据源**：`GET /api/templates` → `POST/PUT/DELETE /api/templates/{name}`

---

### 6. Playground 页面 `/playground`

**用途**：不走编排，直接测试单个 stage 执行

**表单**：
- 选择 Project（下拉）
- 选择 Agent（下拉：Claude Code / Codex）
- Model（输入框，可选）
- Prompt（textarea）
- [Run]

**结果**：
- 执行状态
- Agent Log
- Diff
- Duration + Exit Code

**数据源**：`POST /api/run` → poll `GET /api/tasks/{id}` + `GET /api/tasks/{id}/stages`

---

### 7. Settings 页面 `/settings`

**布局**：分区卡片

**Agent Configuration**：
- 每个 Agent 一张卡片
- 标题 + Installed 状态（绿色/灰色圆点）
- API Key（密码输入框，带 Show/Hide）
- API Base URL
- Default Model
- [Test Connection] 按钮

**Tools**：
- Tavily Search 卡片
- API Key

**Git**：
- GitHub Personal Access Token

**全局操作**：
- [Save Changes] 按钮（有变更时高亮）

**数据源**：`GET /api/settings` → `PUT /api/settings`

---

## 后端 API 汇总

| Method | Path | 说明 |
|--------|------|------|
| GET | /api/projects | 项目列表 |
| POST | /api/projects | 创建项目 |
| GET | /api/projects/{id} | 项目详情 |
| DELETE | /api/projects/{id} | 删除项目 |
| GET | /api/templates | 模板列表 |
| POST | /api/templates | 创建模板 |
| GET | /api/templates/{name} | 模板详情 |
| PUT | /api/templates/{name} | 更新模板 |
| DELETE | /api/templates/{name} | 删除模板（非 builtin） |
| GET | /api/task-types | 任务类型列表（同 templates，兼容） |
| POST | /api/tasks | 创建任务 |
| GET | /api/tasks?project_id=xxx | 任务列表（支持 project_id 筛选） |
| GET | /api/tasks/{id} | 任务详情 |
| GET | /api/tasks/{id}/stages | 任务的 stage runs |
| POST | /api/tasks/{id}/cancel | 取消任务 |
| POST | /api/run | 单 stage 快捷运行（Playground） |
| GET | /api/stage-runs/{id} | 单个 stage run 详情 |
| GET | /api/settings | 全局配置 |
| PUT | /api/settings | 更新配置 |
| POST | /api/settings/test-agent | 测试 Agent 连接 |
| POST | /api/settings/test-tool | 测试工具 |

---

## 设计要求

- 深色主题（当前已有）
- 响应式，最小宽度 1024px
- Stage 流程可视化用简单的节点连线图（不需要复杂的 DAG 编辑器）
- 状态 badge 颜色：pending=灰，running=蓝（带 pulse 动画），success=绿，failed=红
- 代码/日志用等宽字体
- Diff 高亮：增加行绿色背景，删除行红色背景
