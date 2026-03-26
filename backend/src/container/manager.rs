use anyhow::Result;
use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;

use crate::config::PlatformConfig;
use crate::contracts::AgentType;
use crate::entity::task;

/// report.json produced by entrypoint.sh inside the container (new format)
#[derive(Debug, serde::Deserialize)]
pub struct ContainerReport {
    pub agent_exit_code: i32,
    pub has_summary: bool,
    pub files_changed: Vec<String>,
    pub branch: String,
    pub duration_seconds: i32,
    pub lines_added: i32,
    pub lines_removed: i32,
    pub model: String,
    #[serde(default)]
    pub pushed: bool,
}

/// Abstraction over container runtime for testability.
/// `env_config` carries dynamic config from platform_config DB table.
#[async_trait]
pub trait ContainerRuntime: Send + Sync + 'static {
    async fn run_task(
        &self,
        task: &task::Model,
        config: &PlatformConfig,
        env_config: &std::collections::HashMap<String, String>,
    ) -> Result<TaskRunResult>;

    async fn cancel_container(&self, container_id: &str) -> Result<()>;

    async fn check_image_status(
        &self,
        config: &PlatformConfig,
    ) -> Result<Vec<super::images::ImageStatus>>;

    async fn build_all_images(&self, config: &PlatformConfig) -> Result<()>;

    async fn ensure_images(&self, config: &PlatformConfig) -> Result<()>;
}

#[derive(Debug)]
pub struct TaskRunResult {
    pub container_id: String,
    pub exit_code: i64,
    pub report: Option<ContainerReport>,
    pub summary: Option<String>,
    pub diff_patch: Option<String>,
    pub agent_log: Option<String>,
}

// ── Bollard implementation ──

pub struct BollardRuntime {
    docker: Docker,
}

impl BollardRuntime {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self { docker })
    }
}

#[async_trait]
impl ContainerRuntime for BollardRuntime {
    async fn run_task(
        &self,
        task: &task::Model,
        config: &PlatformConfig,
        env_config: &std::collections::HashMap<String, String>,
    ) -> Result<TaskRunResult> {
        let container_name = format!("ccodebox-{}", task.id);

        let image = match task.agent_type {
            AgentType::ClaudeCode => &config.cc_image,
            AgentType::Codex => &config.codex_image,
        };

        let mut env = vec![
            format!("AGENT_TYPE={}", task.agent_type.as_str()),
            format!("TASK_PROMPT={}", task.prompt),
            format!("TASK_ID={}", task.id),
        ];

        // Agent-specific env vars from DB config
        match task.agent_type {
            AgentType::ClaudeCode => {
                env.push(format!("CC_MODEL={}", task.model));
                if let Some(v) = env_config.get("agent.claude-code.api_base_url") {
                    env.push(format!("ANTHROPIC_BASE_URL={v}"));
                }
                if let Some(v) = env_config.get("agent.claude-code.api_key") {
                    env.push(format!("ANTHROPIC_AUTH_TOKEN={v}"));
                }
            }
            AgentType::Codex => {
                env.push(format!("CODEX_MODEL={}", task.model));
                if let Some(v) = env_config.get("agent.codex.api_key") {
                    env.push(format!("OPENAI_API_KEY={v}"));
                }
                if let Some(v) = env_config.get("agent.codex.api_base_url") {
                    env.push(format!("OPENAI_BASE_URL={v}"));
                }
            }
        }

        // Shared tool keys
        if let Some(v) = env_config.get("tool.tavily.api_key") {
            env.push(format!("TAVILY_API_KEY={v}"));
        }
        if let Some(v) = env_config.get("git.github_token") {
            env.push(format!("GITHUB_TOKEN={v}"));
        }

        if let Some(ref repo_url) = task.repo_url {
            env.push(format!("REPO_URL={repo_url}"));
        }
        if let Some(ref branch) = task.branch {
            env.push(format!("BRANCH={branch}"));
        }

        let host_config = bollard::models::HostConfig {
            memory: Some(config.container_memory_limit),
            cpu_quota: Some(config.container_cpu_quota),
            ..Default::default()
        };

        let container_config = Config {
            image: Some(image.clone()),
            env: Some(env),
            host_config: Some(host_config),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: &container_name,
            platform: None,
        };

        let created = self
            .docker
            .create_container(Some(options), container_config)
            .await?;

        let container_id = created.id.clone();

        self.docker
            .start_container::<String>(&container_id, None)
            .await?;

        // Wait for container to finish
        let mut wait_stream = self.docker.wait_container(
            &container_id,
            Some(WaitContainerOptions {
                condition: "not-running",
            }),
        );

        let mut exit_code: i64 = -1;
        while let Some(result) = wait_stream.next().await {
            match result {
                Ok(response) => {
                    exit_code = response.status_code;
                }
                Err(e) => {
                    tracing::error!("Error waiting for container: {e}");
                }
            }
        }

        // Collect results from container
        let report = self
            .copy_file_from_container(&container_id, "/workspace/.loop/report.json")
            .await
            .ok()
            .and_then(|s| serde_json::from_str::<ContainerReport>(&s).ok());

        let summary = self
            .copy_file_from_container(&container_id, "/workspace/.loop/summary.md")
            .await
            .ok();
        let diff_patch = self
            .copy_file_from_container(&container_id, "/workspace/.loop/diff.patch")
            .await
            .ok();

        // Collect single agent log
        let agent_log = self
            .copy_file_from_container(&container_id, "/workspace/.loop/agent.log")
            .await
            .ok();

        // Remove container
        self.docker
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .ok();

        Ok(TaskRunResult {
            container_id,
            exit_code,
            report,
            summary,
            diff_patch,
            agent_log,
        })
    }

    async fn cancel_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .kill_container::<String>(container_id, None)
            .await
            .ok();

        self.docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        Ok(())
    }

    async fn check_image_status(
        &self,
        config: &PlatformConfig,
    ) -> Result<Vec<super::images::ImageStatus>> {
        super::images::check_image_status(&self.docker, config).await
    }

    async fn build_all_images(&self, config: &PlatformConfig) -> Result<()> {
        super::images::build_all_images(&self.docker, config).await
    }

    async fn ensure_images(&self, config: &PlatformConfig) -> Result<()> {
        super::images::ensure_images(&self.docker, config).await
    }
}

impl BollardRuntime {
    async fn copy_file_from_container(&self, container_id: &str, path: &str) -> Result<String> {
        let bytes = self
            .docker
            .download_from_container(
                container_id,
                Some(bollard::container::DownloadFromContainerOptions {
                    path: path.to_string(),
                }),
            )
            .collect::<Vec<_>>()
            .await;

        let mut all_bytes = Vec::new();
        for chunk in bytes {
            all_bytes.extend_from_slice(&chunk?);
        }

        let mut archive = tar::Archive::new(&all_bytes[..]);
        if let Some(entry) = archive.entries()?.next() {
            let mut entry = entry?;
            let mut content = String::new();
            std::io::Read::read_to_string(&mut entry, &mut content)?;
            return Ok(content);
        }

        anyhow::bail!("file not found in tar archive: {path}")
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Mock implementation for testing
    pub struct MockRuntime {
        pub run_results: std::sync::Mutex<Vec<TaskRunResult>>,
        pub cancel_calls: std::sync::Mutex<Vec<String>>,
    }

    impl MockRuntime {
        pub fn new() -> Self {
            Self {
                run_results: std::sync::Mutex::new(Vec::new()),
                cancel_calls: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl ContainerRuntime for MockRuntime {
        async fn run_task(
            &self,
            _task: &task::Model,
            _config: &PlatformConfig,
            _env_config: &std::collections::HashMap<String, String>,
        ) -> Result<TaskRunResult> {
            let mut results = self.run_results.lock().unwrap();
            if results.is_empty() {
                Ok(TaskRunResult {
                    container_id: format!("mock-container-{}", uuid::Uuid::new_v4()),
                    exit_code: 0,
                    report: Some(ContainerReport {
                        agent_exit_code: 0,
                        has_summary: true,
                        files_changed: vec!["main.py".into()],
                        branch: "task-branch".into(),
                        duration_seconds: 60,
                        lines_added: 10,
                        lines_removed: 2,
                        model: "claude-sonnet-4-20250514".into(),
                        pushed: false,
                    }),
                    summary: Some("Mock task completed.".into()),
                    diff_patch: Some("mock diff".into()),
                    agent_log: Some("Mock agent output".into()),
                })
            } else {
                Ok(results.remove(0))
            }
        }

        async fn cancel_container(&self, container_id: &str) -> Result<()> {
            self.cancel_calls
                .lock()
                .unwrap()
                .push(container_id.to_string());
            Ok(())
        }

        async fn check_image_status(
            &self,
            config: &PlatformConfig,
        ) -> Result<Vec<crate::container::images::ImageStatus>> {
            Ok(vec![
                crate::container::images::ImageStatus { name: "ccodebox-base:latest".into(), ready: true },
                crate::container::images::ImageStatus { name: config.cc_image.clone(), ready: true },
                crate::container::images::ImageStatus { name: config.codex_image.clone(), ready: true },
            ])
        }

        async fn build_all_images(&self, _config: &PlatformConfig) -> Result<()> {
            Ok(())
        }

        async fn ensure_images(&self, _config: &PlatformConfig) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn mock_runtime_returns_default_result() {
        use crate::contracts::TaskStatus;

        let mock = MockRuntime::new();
        let config = PlatformConfig {
            cc_image: "test:latest".into(),
            codex_image: "test-codex:latest".into(),
            container_memory_limit: 1024,
            container_cpu_quota: 100000,
            default_model: "test-model".into(),
        };

        let task_model = task::Model {
            id: uuid::Uuid::new_v4().to_string(),
            title: "test".into(),
            prompt: "do stuff".into(),
            repo_url: None,
            branch: None,
            agent_type: AgentType::ClaudeCode,
            model: "test-model".into(),
            status: TaskStatus::Pending,
            container_id: None,
            agent_exit_code: None,
            duration_seconds: None,
            pushed: false,
            lines_added: 0,
            lines_removed: 0,
            files_changed: None,
            summary: None,
            diff_patch: None,
            error: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
        };

        let env_config = std::collections::HashMap::new();
        let result = mock.run_task(&task_model, &config, &env_config).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.report.is_some());
        assert_eq!(result.report.unwrap().agent_exit_code, 0);
    }

    #[tokio::test]
    async fn mock_runtime_cancel_records_call() {
        let mock = MockRuntime::new();
        mock.cancel_container("test-container").await.unwrap();
        let calls = mock.cancel_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "test-container");
    }
}
