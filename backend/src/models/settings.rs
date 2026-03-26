use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::task::AgentType;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentInfo {
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub name: String,
    pub image: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsResponse {
    pub agents: Vec<AgentInfo>,
    pub default_model: String,
    pub max_rounds_limit: i32,
}

#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub cc_image: String,
    pub cc_api_base_url: String,
    pub cc_api_key: String,
    pub container_memory_limit: i64,
    pub container_cpu_quota: i64,
    pub default_model: String,
    pub max_rounds_limit: i32,
}

impl PlatformConfig {
    pub fn from_env() -> Self {
        Self {
            cc_image: std::env::var("CC_IMAGE").unwrap_or_else(|_| "ccodebox-cc:latest".into()),
            cc_api_base_url: std::env::var("CC_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".into()),
            cc_api_key: std::env::var("CC_API_KEY").unwrap_or_default(),
            container_memory_limit: std::env::var("CONTAINER_MEMORY_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4_294_967_296), // 4GB
            container_cpu_quota: std::env::var("CONTAINER_CPU_QUOTA")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200_000), // 2 cores
            default_model: std::env::var("CC_DEFAULT_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".into()),
            max_rounds_limit: std::env::var("CC_MAX_ROUNDS_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
        }
    }

    pub fn settings_response(&self) -> SettingsResponse {
        SettingsResponse {
            agents: vec![AgentInfo {
                agent_type: AgentType::ClaudeCode,
                name: "Claude Code".into(),
                image: self.cc_image.clone(),
                models: vec![
                    "claude-opus-4-6".into(),
                    "claude-sonnet-4-20250514".into(),
                ],
            }],
            default_model: self.default_model.clone(),
            max_rounds_limit: self.max_rounds_limit,
        }
    }
}
