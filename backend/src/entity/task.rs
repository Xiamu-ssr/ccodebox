//! # tasks 表 — 编码任务
//!
//! ## 业务规则
//! - 一个 task 对应一次容器执行，容器内 agent 自主完成编码
//! - prompt 是用户提交的原始需求，不可修改
//!
//! ## 状态流转
//! Pending → Running → Success/Failed，Failed → Pending（重试）
//! Running → Cancelled（需 kill 容器）
//!
//! ## 关联
//! - container_id 关联 Docker/Podman 容器（逻辑外键，不建物理外键）
//! - task_logs 表存储日志（has_one）

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::contracts::{AgentType, TaskStatus};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    /// 主键 UUID，由后端生成
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// 任务标题，用于列表展示
    pub title: String,

    /// 用户提交的原始 prompt，创建后不可修改
    #[sea_orm(column_type = "Text")]
    pub prompt: String,

    /// 可选，要操作的 git 仓库地址
    pub repo_url: Option<String>,

    /// 可选，git 分支名
    pub branch: Option<String>,

    /// Agent 类型：ClaudeCode / Codex
    #[sea_orm(default_value = "claude-code")]
    pub agent_type: AgentType,

    /// LLM 模型名称
    pub model: String,

    /// 任务状态，变更前必须调用 TaskStatus::can_transition_to()
    #[sea_orm(default_value = "pending")]
    pub status: TaskStatus,

    /// Docker/Podman 容器 ID
    pub container_id: Option<String>,

    /// agent 进程退出码
    pub agent_exit_code: Option<i32>,

    /// 任务执行耗时（秒）
    pub duration_seconds: Option<i32>,

    /// 是否已推送到远程仓库
    #[sea_orm(default_value = false)]
    pub pushed: bool,

    /// 新增代码行数
    #[sea_orm(default_value = 0)]
    pub lines_added: i32,

    /// 删除代码行数
    #[sea_orm(default_value = 0)]
    pub lines_removed: i32,

    /// 变更的文件列表（逗号分隔）
    pub files_changed: Option<String>,

    /// agent 生成的摘要（summary.md 内容）
    #[sea_orm(column_type = "Text", nullable)]
    pub summary: Option<String>,

    /// git diff 内容
    #[sea_orm(column_type = "Text", nullable)]
    pub diff_patch: Option<String>,

    /// 失败时的错误信息
    pub error: Option<String>,

    /// 创建时间 UTC
    pub created_at: DateTimeUtc,

    /// 开始执行时间 UTC
    pub started_at: Option<DateTimeUtc>,

    /// 完成时间 UTC（成功/失败/取消时写入）
    pub finished_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::task_log::Entity")]
    TaskLog,
}

impl Related<super::task_log::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TaskLog.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
