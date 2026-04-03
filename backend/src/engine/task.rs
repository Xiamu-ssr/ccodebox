use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::consts;
use crate::contracts::{AgentType, StageRunStatus, TaskStatus};
use crate::db::Database;
use crate::engine::stage::{StageExecParams, StageExecutor};
use crate::engine::task_type::{get_task_type_from_db, substitute, StageDef};

/// 任务编排器 — 按模板 stages 顺序执行，支持 context_from 和 on_fail 重试
pub struct TaskOrchestrator {
    pub db: Database,
    pub stage_executor: StageExecutor,
}

impl TaskOrchestrator {
    /// 执行整个 task：按模板编排 stages
    pub async fn execute(&self, task_id: &str) -> Result<()> {
        // 1. 读 task + project
        let task = self
            .db
            .get_task(task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Task not found: {task_id}"))?;

        let project = match &task.project_id {
            Some(pid) => self
                .db
                .get_project(pid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Project not found: {pid}"))?,
            None => anyhow::bail!("Task has no project_id"),
        };

        let repo_path = project
            .local_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Project has no local_path"))?
            .clone();

        // 2. 解析 task_type 模板（从 DB 读取）
        let task_type_def = get_task_type_from_db(&self.db, &task.task_type)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Unknown task type: {}", task.task_type))?;

        // 3. 构建变量表（inputs JSON + task.prompt 注入）
        let mut vars: HashMap<String, String> = if let Some(inputs_json) = &task.inputs {
            serde_json::from_str(inputs_json).unwrap_or_default()
        } else {
            HashMap::new()
        };
        // 注入 task.prompt 作为 "prompt" 变量（单 stage 兼容）
        vars.entry("prompt".into())
            .or_insert_with(|| task.prompt.clone());

        // 4. 设 task Running
        self.db
            .update_task_status(task_id, TaskStatus::Running, None)
            .await?;

        let env_config = self.db.get_all_config().await.unwrap_or_default();

        // 5. 按 stages 顺序执行
        let stages = task_type_def.stages.clone();
        let mut stage_idx = 0;
        let mut retry_counts: HashMap<String, i32> = HashMap::new();
        let mut carry_context: Option<String> = None;

        while stage_idx < stages.len() {
            let stage_def = &stages[stage_idx];

            let result = self
                .execute_one_stage(
                    task_id,
                    &project.name,
                    &repo_path,
                    stage_def,
                    &vars,
                    &env_config,
                    carry_context.take(),
                )
                .await;

            match result {
                Ok(stage_run_id) => {
                    let sr = self.db.get_stage_run(&stage_run_id).await?.unwrap();
                    let final_status =
                        judge_stage_result(&sr.stage_name, sr.agent_exit_code, &sr.agent_log);

                    // 如果判定结果与 stage executor 设的不同，覆盖
                    if final_status != sr.status {
                        self.db
                            .update_stage_run_status(&stage_run_id, final_status, None, None)
                            .await?;
                    }

                    if final_status == StageRunStatus::Success {
                        // 成功 → 下一个 stage
                        carry_context = None;
                        stage_idx += 1;
                    } else {
                        // 失败 → 检查 on_fail
                        if let Some(on_fail) = &stage_def.on_fail {
                            let count = retry_counts
                                .entry(on_fail.goto.clone())
                                .or_insert(0);
                            *count += 1;

                            if *count > on_fail.max_retries
                                || *count > consts::MAX_STAGE_RETRIES_HARD_LIMIT
                            {
                                self.db
                                    .update_task_status(
                                        task_id,
                                        TaskStatus::Failed,
                                        Some(&format!(
                                            "Stage '{}' failed after {} retries",
                                            stage_def.name, on_fail.max_retries
                                        )),
                                    )
                                    .await?;
                                return Ok(());
                            }

                            // carry error_report
                            carry_context = match on_fail.carry.as_str() {
                                "error_report" => sr.error_report.clone().or(sr.agent_log.clone()),
                                _ => None,
                            };

                            // goto 目标 stage
                            stage_idx = stages
                                .iter()
                                .position(|s| s.name == on_fail.goto)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("on_fail goto target not found: {}", on_fail.goto)
                                })?;
                        } else {
                            // 无 on_fail → task 失败
                            self.db
                                .update_task_status(
                                    task_id,
                                    TaskStatus::Failed,
                                    Some(&format!("Stage '{}' failed", stage_def.name)),
                                )
                                .await?;
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    self.db
                        .update_task_status(
                            task_id,
                            TaskStatus::Failed,
                            Some(&format!("Stage '{}' error: {e}", stage_def.name)),
                        )
                        .await?;
                    return Err(e);
                }
            }
        }

        // 6. 全部 stage 通过
        self.db
            .update_task_status(task_id, TaskStatus::Success, None)
            .await?;

        Ok(())
    }

    /// 执行单个 stage
    async fn execute_one_stage(
        &self,
        task_id: &str,
        project_name: &str,
        repo_path: &str,
        stage_def: &StageDef,
        vars: &HashMap<String, String>,
        env_config: &HashMap<String, String>,
        carry_context: Option<String>,
    ) -> Result<String> {
        // 变量替换
        let agent_str = substitute(&stage_def.agent, vars);
        let mut prompt = substitute(&stage_def.prompt, vars);

        // context_from：拼入上游 stage 产出
        if let Some(from_stage) = &stage_def.context_from {
            let context = self
                .build_context_from(task_id, from_stage)
                .await
                .unwrap_or_default();
            if !context.is_empty() {
                prompt = format!("{context}\n\n---\n\n{prompt}");
            }
        }

        // carry context（on_fail 携带的 error_report）
        if let Some(carried) = carry_context {
            prompt = format!(
                "## Previous attempt failed\n\nError from previous run:\n{carried}\n\n---\n\n{prompt}"
            );
        }

        // 解析 model
        let model = vars.get("model").and_then(|m| {
            if m.is_empty() {
                None
            } else {
                Some(m.clone())
            }
        });

        let agent_type: AgentType =
            serde_json::from_value(serde_json::Value::String(agent_str.clone()))
                .with_context(|| format!("Unknown agent: {agent_str}"))?;

        self.stage_executor
            .execute(StageExecParams {
                task_id: task_id.to_string(),
                project_name: project_name.to_string(),
                repo_path: repo_path.to_string(),
                stage_name: stage_def.name.clone(),
                agent_type,
                prompt,
                model,
                env_config: env_config.clone(),
                needs_branch: stage_def.needs_branch,
            })
            .await
    }

    /// 从上游 stage 的最近一次成功 run 构建 context
    async fn build_context_from(&self, task_id: &str, from_stage: &str) -> Result<String> {
        let runs = self.db.list_stage_runs_by_task(task_id).await?;

        // 找到该 stage 最近一次成功的 run
        let upstream = runs
            .iter()
            .rev()
            .find(|r| r.stage_name == from_stage && r.status == StageRunStatus::Success);

        let Some(sr) = upstream else {
            return Ok(String::new());
        };

        let mut parts = Vec::new();
        parts.push(format!("## Context from stage '{from_stage}'"));

        // Don't embed the diff — the agent is in the worktree and can inspect it directly
        parts.push(
            "### Changes\nThe previous stage has made code changes in this workspace. \
             Run `git diff HEAD~1` to see what was changed, or `git log --oneline -5` for recent commits."
                .to_string(),
        );

        if let Some(summary) = &sr.summary {
            if !summary.is_empty() {
                parts.push(format!("### Summary\n{summary}"));
            }
        }

        Ok(parts.join("\n\n"))
    }
}

/// 判定 stage 结果：testing stage 检查 agent_log 关键字
pub fn judge_stage_result(
    stage_name: &str,
    exit_code: Option<i32>,
    agent_log: &Option<String>,
) -> StageRunStatus {
    let code = exit_code.unwrap_or(-1);

    if stage_name == "testing" {
        if let Some(log) = agent_log {
            let log_upper = log.to_uppercase();
            if log_upper.contains("ALL TESTS PASSED") {
                return StageRunStatus::Success;
            }
            if log_upper.contains("FAIL") || log_upper.contains("ERROR") {
                return StageRunStatus::Failed;
            }
        }
    }

    if code == 0 {
        StageRunStatus::Success
    } else {
        StageRunStatus::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::tests::MockAdapter;
    use crate::adapter::{AdapterRegistry, AgentAdapter};
    use crate::contracts::CreateProjectRequest;
    use crate::workspace::WorkspaceManager;

    async fn setup_test() -> (Database, String, String) {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db.seed_builtin_templates().await.unwrap();

        // 创建临时 git repo
        let repo_dir = tempfile::tempdir().unwrap();
        let repo_path = repo_dir.path().to_str().unwrap().to_string();

        tokio::process::Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        tokio::fs::write(format!("{repo_path}/README.md"), "# test")
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();

        // 创建项目
        let project = db
            .create_project(&CreateProjectRequest {
                name: "test-proj".into(),
                repo_url: None,
                local_path: Some(repo_path.clone()),
                default_agent: None,
            })
            .await
            .unwrap();

        // 防止 tempdir 被 drop
        std::mem::forget(repo_dir);

        (db, project.id, repo_path)
    }

    fn make_orchestrator(db: Database, success: bool) -> TaskOrchestrator {
        let mut adapters: HashMap<AgentType, Box<dyn AgentAdapter>> = HashMap::new();
        adapters.insert(
            AgentType::ClaudeCode,
            Box::new(MockAdapter::new("claude-code", true).with_success(success)),
        );
        let registry = AdapterRegistry::from_map(adapters);
        let workspace_base = tempfile::tempdir().unwrap();
        let ws_path = workspace_base.path().to_path_buf();
        std::mem::forget(workspace_base);

        TaskOrchestrator {
            db: db.clone(),
            stage_executor: StageExecutor {
                db,
                adapter_registry: registry,
                workspace_manager: WorkspaceManager::new(ws_path),
            },
        }
    }

    #[tokio::test]
    async fn single_stage_orchestration() {
        let (db, project_id, _) = setup_test().await;

        let task = db
            .create_task(&crate::contracts::CreateTaskRequest {
                title: "Single stage".into(),
                prompt: "Hello world".into(),
                project_id: Some(project_id),
                task_type: Some("single-stage".into()),
                inputs: Some(
                    serde_json::json!({"prompt": "Hello world", "agent_type": "claude-code"})
                        .to_string(),
                ),
            })
            .await
            .unwrap();

        let orchestrator = make_orchestrator(db.clone(), true);
        orchestrator.execute(&task.id).await.unwrap();

        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Success);

        let runs = db.list_stage_runs_by_task(&task.id).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].stage_name, "coding");
    }

    #[tokio::test]
    async fn multi_stage_all_pass() {
        let (db, project_id, _) = setup_test().await;

        let task = db
            .create_task(&crate::contracts::CreateTaskRequest {
                title: "Feature dev".into(),
                prompt: "Build feature".into(),
                project_id: Some(project_id),
                task_type: Some("feature-dev".into()),
                inputs: Some(
                    serde_json::json!({"requirement": "Add auth", "agent_type": "claude-code"})
                        .to_string(),
                ),
            })
            .await
            .unwrap();

        let orchestrator = make_orchestrator(db.clone(), true);
        orchestrator.execute(&task.id).await.unwrap();

        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Success);

        let runs = db.list_stage_runs_by_task(&task.id).await.unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].stage_name, "coding");
        assert_eq!(runs[1].stage_name, "testing");
    }

    #[tokio::test]
    async fn stage_failure_no_on_fail_marks_task_failed() {
        let (db, project_id, _) = setup_test().await;

        // single-stage 无 on_fail，失败即终
        let task = db
            .create_task(&crate::contracts::CreateTaskRequest {
                title: "Will fail".into(),
                prompt: "fail".into(),
                project_id: Some(project_id),
                task_type: Some("single-stage".into()),
                inputs: Some(
                    serde_json::json!({"prompt": "fail", "agent_type": "claude-code"}).to_string(),
                ),
            })
            .await
            .unwrap();

        // MockAdapter(success=false) 会让 agent exit code 非 0
        let orchestrator = make_orchestrator(db.clone(), false);
        orchestrator.execute(&task.id).await.unwrap();

        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Failed);
    }

    #[test]
    fn judge_stage_result_normal() {
        assert_eq!(
            judge_stage_result("coding", Some(0), &None),
            StageRunStatus::Success
        );
        assert_eq!(
            judge_stage_result("coding", Some(1), &None),
            StageRunStatus::Failed
        );
    }

    #[test]
    fn judge_stage_result_testing_keywords() {
        assert_eq!(
            judge_stage_result(
                "testing",
                Some(0),
                &Some("ALL TESTS PASSED".into())
            ),
            StageRunStatus::Success
        );
        assert_eq!(
            judge_stage_result(
                "testing",
                Some(0),
                &Some("Some test FAIL detected".into())
            ),
            StageRunStatus::Failed
        );
        // exit_code 非 0 但 log 说 all tests passed → 信 log
        assert_eq!(
            judge_stage_result(
                "testing",
                Some(1),
                &Some("ALL TESTS PASSED".into())
            ),
            StageRunStatus::Success
        );
    }
}
