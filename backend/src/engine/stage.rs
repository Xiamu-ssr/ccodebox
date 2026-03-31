use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

use crate::adapter::{AdapterRegistry, AgentRequest};
use crate::contracts::{AgentType, StageRunStatus};
use crate::db::{Database, StageRunReport};
use crate::workspace::WorkspaceManager;

const SYSTEM_RULES: &str = include_str!("../../../scripts/system-rules.md");

/// 单 Stage 执行器
pub struct StageExecutor {
    pub db: Database,
    pub adapter_registry: AdapterRegistry,
    pub workspace_manager: WorkspaceManager,
}

/// 执行参数
pub struct StageExecParams {
    pub task_id: String,
    pub project_name: String,
    pub repo_path: String,
    pub stage_name: String,
    pub agent_type: AgentType,
    pub prompt: String,
    pub model: Option<String>,
    pub env_config: HashMap<String, String>,
    pub needs_branch: bool,
}

impl StageExecutor {
    /// 执行单个 stage，完整流程：
    /// 1. 创建 StageRun 记录
    /// 2. 创建工作目录
    /// 3. 组装 prompt
    /// 4. 调用 agent
    /// 5. 收集产出
    /// 6. 更新记录
    pub async fn execute(&self, params: StageExecParams) -> Result<String> {
        // 1. 确定 run_number
        let existing = self
            .db
            .list_stage_runs_by_task(&params.task_id)
            .await?;
        let run_number = existing
            .iter()
            .filter(|r| r.stage_name == params.stage_name)
            .count() as i32
            + 1;

        // 2. 创建 StageRun 记录
        let stage_run = self
            .db
            .create_stage_run(
                &params.task_id,
                &params.stage_name,
                run_number,
                params.agent_type.as_str(),
            )
            .await?;

        let stage_run_id = stage_run.id.clone();

        // 3. 更新 task.current_stage
        self.db
            .update_task_current_stage(&params.task_id, &params.stage_name)
            .await?;

        // 4. 创建工作目录
        let repo_path = std::path::PathBuf::from(&params.repo_path);
        let workspace = self
            .workspace_manager
            .create_workspace(
                &params.project_name,
                &params.task_id,
                &params.stage_name,
                run_number,
                &repo_path,
                params.needs_branch,
            )
            .await;

        let workspace = match workspace {
            Ok(ws) => ws,
            Err(e) => {
                self.db
                    .update_stage_run_status(
                        &stage_run_id,
                        StageRunStatus::Failed,
                        None,
                        None,
                    )
                    .await?;
                self.db
                    .update_stage_run_report(
                        &stage_run_id,
                        &StageRunReport {
                            exit_code: None,
                            duration: None,
                            agent_log: None,
                            diff_patch: None,
                            summary: None,
                            error_report: Some(format!("工作目录创建失败: {e}")),
                            prompt_used: None,
                        },
                    )
                    .await?;
                return Err(e.context("创建工作目录失败"));
            }
        };

        // 5. 更新状态为 running + 工作目录信息
        self.db
            .update_stage_run_status(
                &stage_run_id,
                StageRunStatus::Running,
                Some(workspace.path.to_str().unwrap_or("")),
                workspace.branch.as_deref(),
            )
            .await?;

        // 6. 组装 prompt
        let final_prompt = assemble_prompt(&workspace.path, &params.prompt).await;

        // 7. 构建环境变量
        let env = build_agent_env(&params.agent_type, &params.env_config);

        // 8. 获取 adapter 并执行
        let adapter = self
            .adapter_registry
            .get(&params.agent_type)
            .context("未找到对应的 agent adapter")?;

        let start = Instant::now();

        let request = AgentRequest {
            prompt: final_prompt.clone(),
            working_dir: workspace.path.clone(),
            model: params.model,
            env,
        };

        let exec_result = adapter.execute(request).await;

        let mut handle = match exec_result {
            Ok(h) => h,
            Err(e) => {
                let duration = start.elapsed().as_secs() as i32;
                self.db
                    .update_stage_run_report(
                        &stage_run_id,
                        &StageRunReport {
                            exit_code: None,
                            duration: Some(duration),
                            agent_log: None,
                            diff_patch: None,
                            summary: None,
                            error_report: Some(format!("Agent 启动失败: {e}")),
                            prompt_used: Some(final_prompt),
                        },
                    )
                    .await?;
                self.db
                    .update_stage_run_status(
                        &stage_run_id,
                        StageRunStatus::Failed,
                        None,
                        None,
                    )
                    .await?;
                return Err(e.context("Agent 启动失败"));
            }
        };

        // 9. 等待 agent 完成
        let status = handle.child.wait().await?;
        let duration = start.elapsed().as_secs() as i32;
        let exit_code = status.code().unwrap_or(-1);

        // 10. 收集产出
        let agent_log = tokio::fs::read_to_string(&handle.log_path)
            .await
            .ok();

        let diff_patch = if params.needs_branch {
            WorkspaceManager::collect_diff(&workspace.path).await.unwrap_or(None)
        } else {
            None
        };

        let summary = read_summary(&workspace.path).await;

        // 11. 更新报告
        let final_status = if exit_code == 0 {
            StageRunStatus::Success
        } else {
            StageRunStatus::Failed
        };

        self.db
            .update_stage_run_report(
                &stage_run_id,
                &StageRunReport {
                    exit_code: Some(exit_code),
                    duration: Some(duration),
                    agent_log,
                    diff_patch,
                    summary,
                    error_report: if exit_code != 0 {
                        Some(format!("Agent exited with code {exit_code}"))
                    } else {
                        None
                    },
                    prompt_used: Some(final_prompt),
                },
            )
            .await?;

        self.db
            .update_stage_run_status(&stage_run_id, final_status, None, None)
            .await?;

        Ok(stage_run_id)
    }
}

/// 组装最终 prompt：system-rules + 项目 AGENTS.md + 用户 prompt
async fn assemble_prompt(workspace_path: &Path, user_prompt: &str) -> String {
    let mut parts = Vec::new();

    // Layer A: 平台规范
    parts.push(SYSTEM_RULES.to_string());

    // Layer B: 项目规范（AGENTS.md 或 CLAUDE.md）
    for filename in &["AGENTS.md", "CLAUDE.md"] {
        let path = workspace_path.join(filename);
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            parts.push(format!("## 项目规范（{filename}）\n\n{content}"));
            break;
        }
    }

    // Layer C: 用户需求
    parts.push(format!("## 任务\n\n{user_prompt}"));

    parts.join("\n\n---\n\n")
}

/// 读取 agent 产出的 summary
async fn read_summary(workspace_path: &Path) -> Option<String> {
    let path = workspace_path.join(".ccodebox/summary.md");
    tokio::fs::read_to_string(&path).await.ok()
}

/// 根据 agent 类型构建环境变量
fn build_agent_env(
    agent_type: &AgentType,
    config: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut env = HashMap::new();

    match agent_type {
        AgentType::ClaudeCode => {
            if let Some(key) = config.get("agent.claude-code.api_key") {
                env.insert("ANTHROPIC_API_KEY".into(), key.clone());
            }
            if let Some(base) = config.get("agent.claude-code.api_base_url") {
                env.insert("ANTHROPIC_BASE_URL".into(), base.clone());
            }
        }
        AgentType::Codex => {
            if let Some(key) = config.get("agent.codex.api_key") {
                env.insert("OPENAI_API_KEY".into(), key.clone());
            }
            if let Some(base) = config.get("agent.codex.api_base_url") {
                env.insert("OPENAI_BASE_URL".into(), base.clone());
            }
        }
    }

    // GitHub token（所有 agent 通用）
    if let Some(token) = config.get("git.github_token") {
        env.insert("GITHUB_TOKEN".into(), token.clone());
    }

    // Tavily（搜索工具）
    if let Some(key) = config.get("tool.tavily.api_key") {
        env.insert("TAVILY_API_KEY".into(), key.clone());
    }

    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::tests::MockAdapter;
    use crate::adapter::AgentAdapter;
    use crate::contracts::TaskStatus;

    #[tokio::test]
    async fn assemble_prompt_basic() {
        let dir = tempfile::tempdir().unwrap();
        let prompt = assemble_prompt(dir.path(), "实现用户注册功能").await;

        assert!(prompt.contains("自主完成任务"));
        assert!(prompt.contains("实现用户注册功能"));
    }

    #[tokio::test]
    async fn assemble_prompt_with_agents_md() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("AGENTS.md"), "# 项目规则\n使用 Rust")
            .await
            .unwrap();

        let prompt = assemble_prompt(dir.path(), "test").await;
        assert!(prompt.contains("项目规则"));
        assert!(prompt.contains("使用 Rust"));
    }

    #[test]
    fn build_env_claude_code() {
        let mut config = HashMap::new();
        config.insert("agent.claude-code.api_key".into(), "sk-123".into());
        config.insert("git.github_token".into(), "ghp-abc".into());
        config.insert("tool.tavily.api_key".into(), "tvly-xyz".into());

        let env = build_agent_env(&AgentType::ClaudeCode, &config);
        assert_eq!(env.get("ANTHROPIC_API_KEY").unwrap(), "sk-123");
        assert_eq!(env.get("GITHUB_TOKEN").unwrap(), "ghp-abc");
        assert_eq!(env.get("TAVILY_API_KEY").unwrap(), "tvly-xyz");
        assert!(env.get("OPENAI_API_KEY").is_none());
    }

    #[test]
    fn build_env_codex() {
        let mut config = HashMap::new();
        config.insert("agent.codex.api_key".into(), "sk-oai".into());

        let env = build_agent_env(&AgentType::Codex, &config);
        assert_eq!(env.get("OPENAI_API_KEY").unwrap(), "sk-oai");
        assert!(env.get("ANTHROPIC_API_KEY").is_none());
    }

    #[tokio::test]
    async fn full_stage_execution_with_mock() {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();

        // 创建项目和任务
        let project = db
            .create_project(&crate::contracts::CreateProjectRequest {
                name: "test-proj".into(),
                repo_url: None,
                local_path: Some("/tmp".into()),
                default_agent: None,
            })
            .await
            .unwrap();

        let task = db
            .create_task(&crate::contracts::CreateTaskRequest {
                title: "Test".into(),
                prompt: "Do something".into(),
                project_id: Some(project.id.clone()),
                task_type: None,
                inputs: None,
            })
            .await
            .unwrap();

        db.update_task_status(&task.id, TaskStatus::Running, None)
            .await
            .unwrap();

        // 创建 mock adapter registry
        let mut adapters: HashMap<AgentType, Box<dyn AgentAdapter>> = HashMap::new();
        adapters.insert(
            AgentType::ClaudeCode,
            Box::new(MockAdapter::new("claude-code", true)),
        );
        let registry = AdapterRegistry::from_map(adapters);

        // 创建临时 repo
        let repo_dir = tempfile::tempdir().unwrap();
        // git init + initial commit
        tokio::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_dir.path())
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(repo_dir.path())
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(repo_dir.path())
            .output()
            .await
            .unwrap();
        tokio::fs::write(repo_dir.path().join("README.md"), "# test")
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_dir.path())
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo_dir.path())
            .output()
            .await
            .unwrap();

        let workspace_base = tempfile::tempdir().unwrap();
        let executor = StageExecutor {
            db: db.clone(),
            adapter_registry: registry,
            workspace_manager: WorkspaceManager::new(workspace_base.path().to_path_buf()),
        };

        let stage_run_id = executor
            .execute(StageExecParams {
                task_id: task.id.clone(),
                project_name: "test-proj".into(),
                repo_path: repo_dir.path().to_str().unwrap().into(),
                stage_name: "coding".into(),
                agent_type: AgentType::ClaudeCode,
                prompt: "写一个 hello world".into(),
                model: None,
                env_config: HashMap::new(),
                needs_branch: true,
            })
            .await
            .unwrap();

        // 验证 stage run 记录
        let sr = db.get_stage_run(&stage_run_id).await.unwrap().unwrap();
        assert_eq!(sr.status, StageRunStatus::Success);
        assert_eq!(sr.stage_name, "coding");
        assert_eq!(sr.run_number, 1);
        assert!(sr.workspace_path.is_some());
        assert!(sr.branch.is_some());
        assert_eq!(sr.agent_exit_code, Some(0));
        assert!(sr.duration_seconds.is_some());
        assert!(sr.prompt_used.is_some());

        // 验证 task.current_stage 更新
        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.current_stage.as_deref(), Some("coding"));
    }
}
