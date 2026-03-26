use crate::consts;
use crate::contracts::{AgentInfo, AgentType, ConfigItem, SettingsResponse};

/// Static platform configuration (container resources, image names).
/// Dynamic config (API keys, tokens) lives in platform_config DB table.
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub cc_image: String,
    pub codex_image: String,
    pub container_memory_limit: i64,
    pub container_cpu_quota: i64,
    pub default_model: String,
}

impl PlatformConfig {
    pub fn from_env() -> Self {
        Self {
            cc_image: std::env::var("CC_IMAGE")
                .unwrap_or_else(|_| consts::DEFAULT_CC_IMAGE.into()),
            codex_image: std::env::var("CODEX_IMAGE")
                .unwrap_or_else(|_| consts::DEFAULT_CODEX_IMAGE.into()),
            container_memory_limit: std::env::var("CONTAINER_MEMORY_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(consts::DEFAULT_CONTAINER_MEMORY),
            container_cpu_quota: std::env::var("CONTAINER_CPU_QUOTA")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(consts::DEFAULT_CONTAINER_CPU_QUOTA),
            default_model: std::env::var("CC_DEFAULT_MODEL")
                .unwrap_or_else(|_| consts::DEFAULT_MODEL.into()),
        }
    }

    pub fn settings_response(&self, config_items: Vec<ConfigItem>) -> SettingsResponse {
        SettingsResponse {
            agents: vec![
                AgentInfo {
                    agent_type: AgentType::ClaudeCode,
                    name: "Claude Code".into(),
                    image: self.cc_image.clone(),
                },
                AgentInfo {
                    agent_type: AgentType::Codex,
                    name: "Codex".into(),
                    image: self.codex_image.clone(),
                },
            ],
            default_model: self.default_model.clone(),
            config: config_items,
        }
    }
}
