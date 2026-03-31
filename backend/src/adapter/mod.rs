pub mod claude_code;
pub mod codex;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use tokio::process::Child;

use crate::contracts::AgentType;

/// 发给 agent 的执行请求
pub struct AgentRequest {
    /// 最终组装好的 prompt
    pub prompt: String,
    /// agent 工作目录
    pub working_dir: PathBuf,
    /// 可选模型覆盖
    pub model: Option<String>,
    /// 环境变量（API keys 等）
    pub env: HashMap<String, String>,
}

/// agent 子进程句柄
pub struct AgentHandle {
    pub child: Child,
    pub log_path: PathBuf,
}

/// agent 执行结果
pub struct AgentResult {
    pub exit_code: i32,
    pub log: String,
    pub duration_seconds: i32,
}

/// Agent 适配器 trait — 每种 agent CLI 实现一个
#[async_trait]
pub trait AgentAdapter: Send + Sync {
    /// 启动 agent 执行任务，返回子进程 handle
    async fn execute(&self, request: AgentRequest) -> Result<AgentHandle>;

    /// 检查 agent 是否已安装
    async fn check_installed(&self) -> Result<bool>;

    /// agent 名称
    fn name(&self) -> &str;
}

/// Agent 适配器注册表 — 按 AgentType 查找适配器
pub struct AdapterRegistry {
    adapters: HashMap<AgentType, Box<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    pub fn from_map(adapters: HashMap<AgentType, Box<dyn AgentAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn new() -> Self {
        let mut adapters: HashMap<AgentType, Box<dyn AgentAdapter>> = HashMap::new();
        adapters.insert(
            AgentType::ClaudeCode,
            Box::new(claude_code::ClaudeCodeAdapter),
        );
        adapters.insert(AgentType::Codex, Box::new(codex::CodexAdapter));
        Self { adapters }
    }

    pub fn get(&self, agent_type: &AgentType) -> Option<&dyn AgentAdapter> {
        self.adapters.get(agent_type).map(|a| a.as_ref())
    }

    pub fn all(&self) -> Vec<(&AgentType, &dyn AgentAdapter)> {
        self.adapters
            .iter()
            .map(|(k, v)| (k, v.as_ref()))
            .collect()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// 测试用 MockAdapter
    pub struct MockAdapter {
        pub name: String,
        pub installed: bool,
    }

    impl MockAdapter {
        pub fn new(name: &str, installed: bool) -> Self {
            Self {
                name: name.into(),
                installed,
            }
        }
    }

    #[async_trait]
    impl AgentAdapter for MockAdapter {
        async fn execute(&self, request: AgentRequest) -> Result<AgentHandle> {
            // 在工作目录写一个假日志文件
            let log_path = request.working_dir.join(".ccodebox-agent.log");
            tokio::fs::write(&log_path, "mock agent output").await?;

            // 用 echo 作为假子进程
            let child = tokio::process::Command::new("echo")
                .arg("mock")
                .stdout(std::process::Stdio::null())
                .spawn()?;

            Ok(AgentHandle { child, log_path })
        }

        async fn check_installed(&self) -> Result<bool> {
            Ok(self.installed)
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn registry_finds_adapters() {
        let registry = AdapterRegistry::new();
        assert!(registry.get(&AgentType::ClaudeCode).is_some());
        assert!(registry.get(&AgentType::Codex).is_some());
    }

    #[test]
    fn registry_adapter_names() {
        let registry = AdapterRegistry::new();
        assert_eq!(
            registry.get(&AgentType::ClaudeCode).unwrap().name(),
            "claude-code"
        );
        assert_eq!(registry.get(&AgentType::Codex).unwrap().name(), "codex");
    }

    #[test]
    fn registry_lists_all() {
        let registry = AdapterRegistry::new();
        let all = registry.all();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn mock_adapter_execute() {
        let adapter = MockAdapter::new("test", true);
        let dir = tempfile::tempdir().unwrap();
        let req = AgentRequest {
            prompt: "test prompt".into(),
            working_dir: dir.path().to_path_buf(),
            model: None,
            env: HashMap::new(),
        };
        let mut handle = adapter.execute(req).await.unwrap();
        let status = handle.child.wait().await.unwrap();
        assert!(status.success());
        assert!(handle.log_path.exists());
    }

    #[tokio::test]
    async fn mock_adapter_check_installed() {
        let installed = MockAdapter::new("test", true);
        assert!(installed.check_installed().await.unwrap());

        let not_installed = MockAdapter::new("test", false);
        assert!(!not_installed.check_installed().await.unwrap());
    }
}
