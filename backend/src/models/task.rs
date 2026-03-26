use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// All ts(export) types live in this file — single source of truth for frontend types.

// ── Enums ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    ClaudeCode,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum VerifyStatus {
    Pass,
    Fail,
    Skipped,
}

impl VerifyStatus {
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }
}

// ── Task entity ──

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub prompt: String,
    pub repo_url: Option<String>,
    pub branch: Option<String>,
    pub agent_type: AgentType,
    pub model: String,
    pub max_rounds: i32,
    pub status: TaskStatus,
    pub container_id: Option<String>,
    pub rounds_used: i32,
    pub lint_status: Option<VerifyStatus>,
    pub test_status: Option<VerifyStatus>,
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
    pub max_rounds: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateTaskResponse {
    pub id: Uuid,
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
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskLogsResponse {
    pub logs: String,
    pub rounds: i32,
}

/// report.json produced by entrypoint.sh inside the container
#[derive(Debug, Deserialize)]
pub struct ContainerReport {
    pub verify_passed: bool,
    pub rounds: i32,
    pub max_rounds: i32,
    pub agent_type: String,
    pub lint_status: String,
    pub test_status: String,
    pub files_changed: String,
    pub lines_added: i32,
    pub lines_removed: i32,
}
