//! # tasks 表 — 编码任务
//!
//! ## 业务规则
//! - 一个 task 对应一次或多次 stage 执行
//! - prompt 是用户提交的原始需求，不可修改
//!
//! ## 状态流转
//! Pending → Running → Success/Failed，Failed → Pending（重试）
//! Running → Cancelled

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::contracts::TaskStatus;

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

    /// 关联项目 ID
    pub project_id: Option<String>,

    /// 任务类型模板名称
    #[sea_orm(default_value = "single-stage")]
    pub task_type: String,

    /// 任务输入参数（JSON）
    #[sea_orm(column_type = "Text", nullable)]
    pub inputs: Option<String>,

    /// 当前执行的 stage 名称
    pub current_stage: Option<String>,

    /// 任务状态，变更前必须调用 TaskStatus::can_transition_to()
    #[sea_orm(default_value = "pending")]
    pub status: TaskStatus,

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
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
