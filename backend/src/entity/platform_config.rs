//! # platform_config 表 — 平台配置（配置中心）
//!
//! ## 业务规则
//! - key/value 存储，key 为点分路径（如 'agent.claude-code.api_key'）
//! - 敏感字段 encrypted=true，API 返回时脱敏
//! - upsert 语义：key 存在则更新，不存在则插入

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "platform_config")]
pub struct Model {
    /// 点分路径 key，如 'agent.claude-code.api_key'
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,

    /// 值（敏感字段加密存储）
    #[sea_orm(column_type = "Text")]
    pub value: String,

    /// 是否为敏感字段
    #[sea_orm(default_value = false)]
    pub encrypted: bool,

    /// 更新时间 UTC
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
