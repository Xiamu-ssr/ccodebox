use crate::consts;
use crate::contracts::{AgentInfo, AgentType, ConfigItem, SettingsResponse};

/// 平台配置（静态，从环境变量读取）
/// 动态配置（API keys 等）存储在 platform_config DB 表
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub default_model: String,
}

impl PlatformConfig {
    pub fn from_env() -> Self {
        Self {
            default_model: std::env::var("CC_DEFAULT_MODEL")
                .unwrap_or_else(|_| consts::DEFAULT_MODEL.into()),
        }
    }

    pub fn settings_response(
        &self,
        config_items: Vec<ConfigItem>,
        agent_status: Vec<(AgentType, bool)>,
    ) -> SettingsResponse {
        let agents = agent_status
            .into_iter()
            .map(|(agent_type, installed)| AgentInfo {
                name: match agent_type {
                    AgentType::ClaudeCode => "Claude Code".into(),
                    AgentType::Codex => "Codex".into(),
                },
                agent_type,
                installed,
            })
            .collect();

        SettingsResponse {
            agents,
            default_model: self.default_model.clone(),
            config: config_items,
        }
    }
}
