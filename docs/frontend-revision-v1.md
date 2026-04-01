# CCodeBoX 前端修订 v1

> 基于已完成的 Gemini 重绘结果，本文档列出需要修正的问题和优化项。
> 优先级：🔴 必须修 → 🟡 建议改 → 🟢 锦上添花

---

## 新增 API

后端新增了一个端点，前端需要使用：

| Method | Path | 说明 |
|--------|------|------|
| GET | /api/agents | 返回可用 agent 列表（含安装状态） |

**响应格式**：
```json
[
  { "type": "claude-code", "name": "claude-code", "installed": true },
  { "type": "codex", "name": "codex", "installed": true }
]
```

TS 类型已导出到 `src/lib/generated/AgentInfo.ts`，执行 `npm run sync-types` 更新。

---

## 🔴 Task 详情页 — 顶级 Tab 精简

**当前**：`Stages (N)` | `Overview` | `Diff` | `Summary` 四个顶级 tab

**改为**：只保留 `Stages (N)` | `Overview` 两个顶级 tab

- `Stages` tab：展示所有 stage runs（已有功能）
- `Overview` tab：显示任务概览（模板、输入、时间轴等元信息）
- ~~Diff~~：删除。用户在每个 stage run 内看 Diff 即可
- ~~Summary~~：删除。用户在每个 stage run 内看 Summary 即可

**原因**：顶级 Diff/Summary 是所有 stage 的聚合数据，但对 single-stage 任务完全冗余，多 stage 场景下用户也更习惯逐个 stage 查看。

---

## 🔴 Task Cancel + Stage Stop 按钮

### Task 级别
- Task 状态为 `running` 或 `pending` 时，标题栏显示 **[Cancel Task]** 按钮（红色）
- 点击后调用 `POST /api/tasks/{id}/cancel`
- 取消后刷新页面，状态变为 `cancelled`

### Stage Run 级别
- 每个 **running** 状态的 stage run 卡片上，显示 **[Stop]** 按钮（红色小按钮，放在状态 badge 旁边）
- 点击后调用 `POST /api/stage-runs/{id}/stop`（新增 API，见下方）
- 停止后该 stage run 状态变为 `cancelled`，但不影响整个 task（task 可以继续其他 stage）

### 新增 API
| Method | Path | 说明 |
|--------|------|------|
| POST | /api/stage-runs/{id}/stop | 停止单个 stage run（kill agent 进程） |

---

## 🔴 Stage Run 布局 — 占满全宽

**当前问题**：Stages tab 下，stage run 卡片只占右半边，左侧大面积留白。

**改为**：stage run 列表/卡片占满整个内容区宽度。每个 stage run 卡片是一个可展开的 accordion，展开后内部显示 Log/Diff/Summary/Prompt 子 tab。

---

## 🔴 New Task Step 2 — 表单字段调整

**当前问题**：`agent_type` 和 `model` 直接作为普通输入框暴露在 Task Details 表单里，与 Title/Requirement 同级。

**改为**：

### 必填区域（始终显示）
- **Task Title**（文本输入框，必填）
- **Requirement**（textarea，必填）

### Advanced Options（默认折叠，点击展开）
- **Agent Type**（**下拉选择框**，非文本输入）
  - 数据源：`GET /api/agents`
  - 只显示 `installed: true` 的 agent
  - 默认值：`claude-code`（或项目的 `default_agent`）
- **Model**（文本输入框）
  - placeholder: `"Leave empty for default (e.g. sonnet, opus, o3)"`
  - 不填则使用 Settings 中该 agent 的 default_model

---

## 🟡 Confirm 页面 — 大小写统一

**当前问题**：确认页显示 `REQUIREMENT` 全大写，但 `Project`、`Template`、`Title` 是正常大小写。

**改为**：统一用 Title Case —— `Requirement`。

---

## 🟡 Settings 页面 — 大幅简化

**当前**：每个 Agent 卡片有 API Key + API Base URL + Default Model

**改为**：

### Agent Configuration
每个 Agent 卡片只保留：
- **Agent 名称** + `Installed` 绿色标记（已有）
- **Default Model**（文本输入框）
  - placeholder: `"e.g. sonnet, opus, o3"`
  - 说明文字: `"Model to use when not specified per-task"`
- ~~API Key~~ 删除
- ~~API Base URL~~ 删除
- ~~Test Connection~~ 删除

**原因**：CC/Codex 的 API Key 和 Base URL 应由用户在自己电脑上配置（`~/.claude`、环境变量等），CCodeBox 不应介入管理。唯一有价值的覆盖是 model。

### Tools
- **Tavily Search**
  - API Key（密码输入框，保留）
  - 说明文字: `"Optional — used by platform steward for web search"`

### Git
- **GitHub Personal Access Token**（密码输入框，保留）
  - 说明文字: `"Required for cloning private repos and pushing branches"`

### 整体
- [Save Changes] 按钮保留

---

## 🟡 Diff 高亮 — 使用 react-diff-viewer

**当前问题**：Diff 显示为纯绿色文本，没有增加行/删除行的颜色区分。

**改为**：使用已安装的 `react-diff-viewer-continued` 渲染 diff，红绿高亮区分增删行。

适用于：
1. 每个 Stage Run 内的 Diff tab
2. （如果保留）顶级 Diff tab

解析逻辑：后端返回的是 `git diff --cached` 的标准 unified diff 格式，传给 `react-diff-viewer` 的 `oldValue`/`newValue` 需要从 unified diff 中分离。或者直接用 `splitView=false` + 原始 diff 文本高亮渲染。

---

## 🟡 Agent Log 格式化

**当前问题**：Log tab 直接显示 CC 的 JSON 原始输出（`{"type":"system","subtype":"init",...}`），不可读。

**改为**：
- 如果 log 内容是 JSON lines 格式（每行一个 JSON），解析并只展示关键字段：
  - `type: "assistant"` → 显示 `message.content`
  - `type: "result"` → 高亮显示最终结果
  - `type: "tool_use"` → 显示工具调用信息
- 提供 "Raw" 切换按钮，可查看原始 JSON
- 如果不是 JSON（比如 Codex 的纯文本输出），直接等宽显示

---

## 🟢 Prompt 分层展示

**当前**：Prompt tab 显示完整的三层 prompt（平台规则 + AGENTS.md + 用户需求），内容很长。

**改为**：
- 默认只展开 **"任务"**（用户需求）部分
- **"平台规范"** 和 **"项目规范 (AGENTS.md)"** 默认折叠
- 用 `---` 分隔符识别三层（后端已用 `---` 分隔）
- 每层标题可点击展开/折叠
- 提供 "Show Full Prompt" 按钮查看完整内容

---

## 🟢 Playground — Agent 下拉

Playground 页面的 Agent 选择已经是下拉框，但数据目前是硬编码的。

**改为**：从 `GET /api/agents` 动态获取，只显示 installed=true 的。

---

## 后端 API 完整列表（更新）

| Method | Path | 说明 |
|--------|------|------|
| GET | /api/projects | 项目列表 |
| POST | /api/projects | 创建项目（Name + GitHub URL） |
| GET | /api/projects/{id} | 项目详情 |
| DELETE | /api/projects/{id} | 删除项目 |
| **GET** | **/api/agents** | **可用 Agent 列表（新增）** |
| GET | /api/templates | 模板列表 |
| POST | /api/templates | 创建模板 |
| GET | /api/templates/{name} | 模板详情 |
| PUT | /api/templates/{name} | 更新模板 |
| DELETE | /api/templates/{name} | 删除模板（非 builtin） |
| GET | /api/task-types | 任务类型列表 |
| POST | /api/tasks | 创建任务 |
| GET | /api/tasks?project_id=xxx | 任务列表 |
| GET | /api/tasks/{id} | 任务详情 |
| GET | /api/tasks/{id}/stages | Stage runs |
| POST | /api/tasks/{id}/cancel | 取消任务 |
| POST | /api/run | Playground 运行 |
| GET | /api/stage-runs/{id} | Stage run 详情 |
| GET | /api/settings | 全局配置 |
| PUT | /api/settings | 更新配置 |

---

## 不需要改的（已经很好）

- ✅ 导航结构 Projects | Templates | Playground | Settings
- ✅ 深色主题一致性
- ✅ 3 步创建任务向导（Select Template → Task Details → Confirm）
- ✅ 项目卡片展示
- ✅ 模板页面的 Builtin 标记和流程预览
- ✅ Stage Run 展开有 Log/Diff/Summary/Prompt 四个子 tab
