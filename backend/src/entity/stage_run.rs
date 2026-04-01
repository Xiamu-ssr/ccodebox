//! # stage_runs 表 — Stage 执行记录
//!
//! ## 业务规则
//! - 每次 stage 执行产生一条记录
//! - 一个 task 可以有多条 stage_run（多 stage 编排或重试）
//! - run_number 从 1 开始，重试时递增
//!
//! ## 状态流转
//! Pending → Running → Success/Failed
//!
//! ## 关联
//! - task_id 关联 tasks 表

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::contracts::StageRunStatus;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stage_runs")]
pub struct Model {
    /// 主键 UUID
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// 关联的任务 ID
    pub task_id: String,

    /// stage 名称（如 "coding", "testing"）
    pub stage_name: String,

    /// 第几次运行（重试时递增）
    #[sea_orm(default_value = 1)]
    pub run_number: i32,

    /// 使用的 agent 类型
    pub agent_type: String,

    /// 执行状态
    #[sea_orm(default_value = "pending")]
    pub status: StageRunStatus,

    /// 工作目录路径
    pub workspace_path: Option<String>,

    /// git 分支名（如果 needs_branch）
    pub branch: Option<String>,

    /// agent 进程 PID（用于取消时 kill）
    pub agent_pid: Option<i32>,

    /// 实际发给 agent 的 prompt
    #[sea_orm(column_type = "Text", nullable)]
    pub prompt_used: Option<String>,

    /// agent 进程退出码
    pub agent_exit_code: Option<i32>,

    /// agent 输出日志
    #[sea_orm(column_type = "Text", nullable)]
    pub agent_log: Option<String>,

    /// git diff 内容
    #[sea_orm(column_type = "Text", nullable)]
    pub diff_patch: Option<String>,

    /// 执行摘要
    #[sea_orm(column_type = "Text", nullable)]
    pub summary: Option<String>,

    /// 失败时的错误报告（传给下一轮 coding）
    #[sea_orm(column_type = "Text", nullable)]
    pub error_report: Option<String>,

    /// 执行耗时（秒）
    pub duration_seconds: Option<i32>,

    /// 创建时间 UTC
    pub created_at: DateTimeUtc,

    /// 完成时间 UTC
    pub finished_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
