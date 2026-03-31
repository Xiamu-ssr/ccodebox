//! # projects 表 — 项目注册
//!
//! ## 业务规则
//! - 一个 project 代表一个 git 仓库（本地路径或远程 URL）
//! - name 唯一，用于 CLI 引用
//! - local_path 和 repo_url 至少填一个
//!
//! ## 关联
//! - tasks 通过 project_id 关联到 project

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    /// 主键 UUID，由后端生成
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// 项目名称，唯一标识，用于 CLI 引用
    #[sea_orm(unique)]
    pub name: String,

    /// GitHub 远程仓库 URL
    pub repo_url: Option<String>,

    /// 本地已有 repo 路径
    pub local_path: Option<String>,

    /// 默认使用的 agent 类型
    pub default_agent: Option<String>,

    /// 创建时间 UTC
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
