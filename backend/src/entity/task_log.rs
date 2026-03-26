//! # task_logs 表 — Agent 执行日志
//!
//! ## 业务规则
//! - 一个 task 最多有一条日志记录（各轮次拼接存储）
//! - logs 由后端在容器完成后从 agent-round-*.log 合并写入
//! - rounds 记录实际执行轮次数
//!
//! ## 关联
//! - task_id 关联 tasks 表主键（belongs_to）

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "task_logs")]
pub struct Model {
    /// 关联的任务 ID（同时为主键）
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: String,

    /// 各轮次日志拼接内容
    #[sea_orm(column_type = "Text")]
    pub logs: String,

    /// 实际执行轮次数
    #[sea_orm(default_value = 0)]
    pub rounds: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::task::Entity",
        from = "Column::TaskId",
        to = "super::task::Column::Id"
    )]
    Task,
}

impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
