use anyhow::Result;
use async_trait::async_trait;
use std::process::Stdio;

use super::{AgentAdapter, AgentHandle, AgentRequest};

pub struct ClaudeCodeAdapter;

/// Max prompt length that can safely fit in a CLI argument.
/// macOS ARG_MAX is ~1MB; we leave headroom for env vars and other args.
const MAX_CLI_ARG_BYTES: usize = 512 * 1024; // 512KB

#[async_trait]
impl AgentAdapter for ClaudeCodeAdapter {
    async fn execute(&self, req: AgentRequest) -> Result<AgentHandle> {
        let mut cmd = tokio::process::Command::new("claude");
        cmd.args([
            "--print",
            "--output-format",
            "json",
            "--permission-mode",
            "bypassPermissions",
        ]);
        if let Some(model) = &req.model {
            cmd.args(["--model", model]);
        }

        // If prompt is too large for CLI arg, write to file and instruct agent to read it
        if req.prompt.len() > MAX_CLI_ARG_BYTES {
            let prompt_path = req.working_dir.join(".ccodebox-prompt.md");
            std::fs::write(&prompt_path, &req.prompt)?;
            cmd.arg("Read and follow ALL instructions in .ccodebox-prompt.md — that file contains your complete task specification.");
        } else {
            cmd.arg(&req.prompt);
        }

        cmd.current_dir(&req.working_dir);
        cmd.envs(&req.env);

        let log_path = req.working_dir.join(".ccodebox-agent.log");
        let log_file = std::fs::File::create(&log_path)?;
        cmd.stdout(log_file);
        cmd.stderr(Stdio::piped());

        let child = cmd.spawn()?;
        Ok(AgentHandle { child, log_path })
    }

    async fn check_installed(&self) -> Result<bool> {
        Ok(tokio::process::Command::new("claude")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "claude-code"
    }
}
