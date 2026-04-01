//! # templates 表 — 任务编排模板
//!
//! ## 业务规则
//! - name 唯一，全局共享，所有项目创建任务时可选
//! - definition 存储 YAML 内容
//! - builtin=true 表示系统默认提供，不可删除
//!
//! ## 关联
//! - tasks 通过 task_type 字段引用 template.name

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "templates")]
pub struct Model {
    /// 主键 UUID
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// 模板名称，唯一标识
    #[sea_orm(unique)]
    pub name: String,

    /// 描述
    pub description: String,

    /// YAML 定义内容
    #[sea_orm(column_type = "Text")]
    pub definition: String,

    /// 是否为内置模板
    #[sea_orm(default_value = false)]
    pub builtin: bool,

    /// 创建时间 UTC
    pub created_at: DateTimeUtc,

    /// 更新时间 UTC
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
