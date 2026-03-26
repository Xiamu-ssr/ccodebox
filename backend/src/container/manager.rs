use anyhow::Result;
use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;

use crate::models::settings::PlatformConfig;
use crate::models::task::{AgentType, ContainerReport, Task};

/// Abstraction over container runtime for testability.
#[async_trait]
pub trait ContainerRuntime: Send + Sync + 'static {
    /// Create, start, wait, collect results, remove container.
    /// Returns (exit_code, report, summary, diff_patch, logs).
    async fn run_task(
        &self,
        task: &Task,
        config: &PlatformConfig,
    ) -> Result<TaskRunResult>;

    /// Kill and remove a running container.
    async fn cancel_container(&self, container_id: &str) -> Result<()>;
}

#[derive(Debug)]
pub struct TaskRunResult {
    pub container_id: String,
    pub exit_code: i64,
    pub report: Option<ContainerReport>,
    pub summary: Option<String>,
    pub diff_patch: Option<String>,
    pub logs: Vec<String>,
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
        task: &Task,
        config: &PlatformConfig,
    ) -> Result<TaskRunResult> {
        let container_name = format!("ccodebox-{}", task.id);

        let image = match task.agent_type {
            AgentType::ClaudeCode => &config.cc_image,
            AgentType::Codex => &config.cc_image, // TODO: codex image
        };

        let mut env = vec![
            format!("AGENT_TYPE={}", task.agent_type.as_str()),
            format!("TASK_PROMPT={}", task.prompt),
            format!("MAX_ROUNDS={}", task.max_rounds),
            format!("CC_MODEL={}", task.model),
            format!("ANTHROPIC_BASE_URL={}", config.cc_api_base_url),
            format!("ANTHROPIC_AUTH_TOKEN={}", config.cc_api_key),
        ];

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

        self.docker.start_container::<String>(&container_id, None).await?;

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
        let report = self.copy_file_from_container(&container_id, "/workspace/.loop/report.json").await
            .ok()
            .and_then(|s| serde_json::from_str::<ContainerReport>(&s).ok());

        let summary = self.copy_file_from_container(&container_id, "/workspace/.loop/summary.md").await.ok();
        let diff_patch = self.copy_file_from_container(&container_id, "/workspace/.loop/diff.patch").await.ok();

        // Collect round logs
        let mut logs = Vec::new();
        for round in 1..=task.max_rounds {
            let path = format!("/workspace/.loop/agent-round-{round}.log");
            if let Ok(log) = self.copy_file_from_container(&container_id, &path).await {
                logs.push(log);
            }
        }

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
            logs,
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
}

impl BollardRuntime {
    async fn copy_file_from_container(&self, container_id: &str, path: &str) -> Result<String> {
        let bytes = self
            .docker
            .download_from_container(container_id, Some(bollard::container::DownloadFromContainerOptions { path: path.to_string() }))
            .collect::<Vec<_>>()
            .await;

        // bollard returns a tar archive — extract the file content
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
    use std::sync::Mutex;
    use uuid::Uuid;

    /// Mock implementation for testing
    pub struct MockRuntime {
        pub run_results: Mutex<Vec<TaskRunResult>>,
        pub cancel_calls: Mutex<Vec<String>>,
    }

    impl MockRuntime {
        pub fn new() -> Self {
            Self {
                run_results: Mutex::new(Vec::new()),
                cancel_calls: Mutex::new(Vec::new()),
            }
        }

        pub fn with_result(mut self, result: TaskRunResult) -> Self {
            self.run_results.get_mut().unwrap().push(result);
            self
        }
    }

    #[async_trait]
    impl ContainerRuntime for MockRuntime {
        async fn run_task(
            &self,
            _task: &Task,
            _config: &PlatformConfig,
        ) -> Result<TaskRunResult> {
            let mut results = self.run_results.lock().unwrap();
            if results.is_empty() {
                Ok(TaskRunResult {
                    container_id: format!("mock-container-{}", Uuid::new_v4()),
                    exit_code: 0,
                    report: Some(ContainerReport {
                        verify_passed: true,
                        rounds: 1,
                        max_rounds: 3,
                        agent_type: "claude-code".into(),
                        lint_status: "pass".into(),
                        test_status: "pass".into(),
                        files_changed: "main.py".into(),
                        lines_added: 10,
                        lines_removed: 2,
                    }),
                    summary: Some("Mock task completed.".into()),
                    diff_patch: Some("mock diff".into()),
                    logs: vec!["Round 1: mock output".into()],
                })
            } else {
                Ok(results.remove(0))
            }
        }

        async fn cancel_container(&self, container_id: &str) -> Result<()> {
            self.cancel_calls.lock().unwrap().push(container_id.to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn mock_runtime_returns_default_result() {
        let mock = MockRuntime::new();
        let config = PlatformConfig {
            cc_image: "test:latest".into(),
            cc_api_base_url: "http://localhost".into(),
            cc_api_key: "test-key".into(),
            container_memory_limit: 1024,
            container_cpu_quota: 100000,
            default_model: "test-model".into(),
            max_rounds_limit: 3,
        };

        let task = Task {
            id: Uuid::new_v4(),
            title: "test".into(),
            prompt: "do stuff".into(),
            repo_url: None,
            branch: None,
            agent_type: AgentType::ClaudeCode,
            model: "test-model".into(),
            max_rounds: 3,
            status: crate::models::task::TaskStatus::Pending,
            container_id: None,
            rounds_used: 0,
            lint_status: None,
            test_status: None,
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

        let result = mock.run_task(&task, &config).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.report.is_some());
        assert!(result.report.unwrap().verify_passed);
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
