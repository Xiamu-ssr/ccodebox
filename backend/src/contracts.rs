/// All ts(export) types live in this file — single source of truth for frontend types.
/// API 边界用到的枚举也定义在此文件，内部 SeaORM DeriveActiveEnum 一并附带。
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Enums (shared by API + entity) ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[ts(export)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
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

// ── Task (API response) ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub repo_url: Option<String>,
    pub branch: Option<String>,
    pub agent_type: AgentType,
    pub model: String,
    pub status: TaskStatus,
    pub container_id: Option<String>,
    pub agent_exit_code: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub pushed: bool,
    pub lines_added: i32,
    pub lines_removed: i32,
    pub files_changed: Option<String>,
    pub summary: Option<String>,
    pub diff_patch: Option<String>,
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
            repo_url: m.repo_url,
            branch: m.branch,
            agent_type: m.agent_type,
            model: m.model,
            status: m.status,
            container_id: m.container_id,
            agent_exit_code: m.agent_exit_code,
            duration_seconds: m.duration_seconds,
            pushed: m.pushed,
            lines_added: m.lines_added,
            lines_removed: m.lines_removed,
            files_changed: m.files_changed,
            summary: m.summary,
            diff_patch: m.diff_patch,
            error: m.error,
            created_at: m.created_at,
            started_at: m.started_at,
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
    pub repo_url: Option<String>,
    pub branch: Option<String>,
    pub agent_type: Option<AgentType>,
    pub model: Option<String>,
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
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskLogsResponse {
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentInfo {
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub name: String,
    pub image: String,
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

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct TestAgentRequest {
    pub agent_type: AgentType,
}

#[derive(Debug, Deserialize)]
pub struct TestToolRequest {
    pub tool: String,
}
