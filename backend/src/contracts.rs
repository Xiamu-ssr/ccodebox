/// All ts(export) types live in this file — single source of truth for frontend types.
/// API 边界用到的枚举也定义在此文件，内部 SeaORM DeriveActiveEnum 一并附带。
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Enums (shared by API + entity) ──

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS,
)]
#[ts(export)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    #[sea_orm(string_value = "claude-code")]
    ClaudeCode,
    #[sea_orm(string_value = "codex")]
    Codex,
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS,
)]
#[ts(export)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

impl TaskStatus {
    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Running)
                | (Self::Running, Self::Success)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
                | (Self::Failed, Self::Pending)
        )
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS,
)]
#[ts(export)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "snake_case")]
pub enum StageRunStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

impl StageRunStatus {
    pub fn can_transition_to(&self, next: &StageRunStatus) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Running)
                | (Self::Running, Self::Success)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
                | (Self::Pending, Self::Cancelled)
        )
    }
}

// ── Task (API response) ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub project_id: Option<String>,
    pub task_type: String,
    pub inputs: Option<String>,
    pub current_stage: Option<String>,
    pub status: TaskStatus,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl From<crate::entity::task::Model> for Task {
    fn from(m: crate::entity::task::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            prompt: m.prompt,
            project_id: m.project_id,
            task_type: m.task_type,
            inputs: m.inputs,
            current_stage: m.current_stage,
            status: m.status,
            error: m.error,
            created_at: m.created_at,
            started_at: m.started_at,
            finished_at: m.finished_at,
        }
    }
}

// ── Project (API response) ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub repo_url: Option<String>,
    pub local_path: Option<String>,
    pub default_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<crate::entity::project::Model> for Project {
    fn from(m: crate::entity::project::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            repo_url: m.repo_url,
            local_path: m.local_path,
            default_agent: m.default_agent,
            created_at: m.created_at,
        }
    }
}

// ── StageRun (API response) ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageRun {
    pub id: String,
    pub task_id: String,
    pub stage_name: String,
    pub run_number: i32,
    pub agent_type: String,
    pub status: StageRunStatus,
    pub workspace_path: Option<String>,
    pub branch: Option<String>,
    pub agent_pid: Option<i32>,
    pub agent_exit_code: Option<i32>,
    pub prompt_used: Option<String>,
    pub agent_log: Option<String>,
    pub diff_patch: Option<String>,
    pub summary: Option<String>,
    pub error_report: Option<String>,
    pub duration_seconds: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl From<crate::entity::stage_run::Model> for StageRun {
    fn from(m: crate::entity::stage_run::Model) -> Self {
        Self {
            id: m.id,
            task_id: m.task_id,
            stage_name: m.stage_name,
            run_number: m.run_number,
            agent_type: m.agent_type,
            status: m.status,
            workspace_path: m.workspace_path,
            branch: m.branch,
            agent_pid: m.agent_pid,
            agent_exit_code: m.agent_exit_code,
            prompt_used: m.prompt_used,
            agent_log: m.agent_log,
            diff_patch: m.diff_patch,
            summary: m.summary,
            error_report: m.error_report,
            duration_seconds: m.duration_seconds,
            created_at: m.created_at,
            finished_at: m.finished_at,
        }
    }
}

// ── API DTOs ──

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateTaskRequest {
    pub title: String,
    pub prompt: String,
    pub project_id: Option<String>,
    pub task_type: Option<String>,
    pub inputs: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateTaskResponse {
    pub id: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
    pub total: i32,
}

#[derive(Debug, Deserialize)]
pub struct TaskListQuery {
    pub status: Option<String>,
    pub project_id: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateProjectRequest {
    pub name: String,
    pub repo_url: Option<String>,
    pub local_path: Option<String>,
    pub default_agent: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
}

/// POST /api/run — 单次 stage 执行请求
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct RunStageRequest {
    pub project_id: String,
    pub agent_type: AgentType,
    pub prompt: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentInfo {
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub name: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConfigItem {
    pub key: String,
    pub value: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsResponse {
    pub agents: Vec<AgentInfo>,
    pub default_model: String,
    pub config: Vec<ConfigItem>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateSettingsRequest {
    pub config: Vec<ConfigItem>,
}

// ── Task Type definitions (for /api/task-types) ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskTypeInfo {
    pub name: String,
    pub description: String,
    pub inputs: Vec<TaskTypeInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskTypeInput {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskTypeListResponse {
    pub task_types: Vec<TaskTypeInfo>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
}

// ── Template (API response) ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub definition: String,
    pub builtin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<crate::entity::template::Model> for Template {
    fn from(m: crate::entity::template::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            description: m.description,
            definition: m.definition,
            builtin: m.builtin,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TemplateListResponse {
    pub templates: Vec<Template>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub description: String,
    pub definition: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateTemplateRequest {
    pub description: Option<String>,
    pub definition: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestAgentRequest {
    pub agent_type: AgentType,
}

#[derive(Debug, Deserialize)]
pub struct TestToolRequest {
    pub tool: String,
}
