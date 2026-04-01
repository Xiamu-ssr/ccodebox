use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::adapter::AdapterRegistry;
use crate::consts;
use crate::contracts::{
    AgentInfo, ConfigItem, CreateTaskRequest, CreateTaskResponse, RunStageRequest, StageRun,
    StageRunStatus, Task, TaskListQuery, TaskListResponse, TaskStatus, TaskTypeListResponse,
    TestAgentRequest, TestResult, TestToolRequest, UpdateSettingsRequest,
};
use crate::engine::stage::{StageExecParams, StageExecutor};
use crate::engine::task::TaskOrchestrator;
use crate::workspace::WorkspaceManager;
use crate::AppState;

type AppResult<T> = Result<T, AppError>;

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTaskRequest>,
) -> AppResult<impl IntoResponse> {
    let model = state
        .db
        .create_task(&req)
        .await
        .map_err(AppError::Internal)?;

    let response = CreateTaskResponse {
        id: model.id.clone(),
        status: model.status,
        created_at: model.created_at,
    };

    // For single-stage tasks with a project, spawn execution immediately
    if req.project_id.is_some() {
        let task_id = model.id.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            if let Err(e) = execute_task_stages(task_id, state_clone).await {
                tracing::error!("Task execution failed: {e}");
            }
        });
    }

    Ok((StatusCode::CREATED, Json(response)))
}

/// 后台执行 task — 委托给 TaskOrchestrator
async fn execute_task_stages(task_id: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let orchestrator = TaskOrchestrator {
        db: state.db.clone(),
        stage_executor: StageExecutor {
            db: state.db.clone(),
            adapter_registry: AdapterRegistry::new(),
            workspace_manager: WorkspaceManager::new(WorkspaceManager::default_base_dir()),
        },
    };
    orchestrator.execute(&task_id).await
}

/// POST /api/run — 单 stage 直接运行（最简模式）
pub async fn run_stage(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunStageRequest>,
) -> AppResult<impl IntoResponse> {
    let project = state
        .db
        .get_project(&req.project_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::BadRequest(format!(
            "Project not found: {}",
            req.project_id
        )))?;

    let repo_path = project
        .local_path
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Project has no local_path".into()))?
        .clone();

    // 创建一个 task 记录
    let task = state
        .db
        .create_task(&CreateTaskRequest {
            title: format!("Run: {}", truncate(&req.prompt, 50)),
            prompt: req.prompt.clone(),
            project_id: Some(req.project_id.clone()),
            task_type: Some("single-stage".into()),
            inputs: None,
        })
        .await
        .map_err(AppError::Internal)?;

    let response = CreateTaskResponse {
        id: task.id.clone(),
        status: task.status,
        created_at: task.created_at,
    };

    let task_id = task.id.clone();
    let prompt = req.prompt.clone();
    let agent_type = req.agent_type;
    let model = req.model.clone();

    tokio::spawn(async move {
        // 更新 task 为 Running
        let _ = state
            .db
            .update_task_status(&task_id, TaskStatus::Running, None)
            .await;

        let env_config = state.db.get_all_config().await.unwrap_or_default();

        // Default model fallback: if not specified, use per-agent default from settings
        let model = model.or_else(|| {
            let key = format!("agent.{}.default_model", agent_type.as_str());
            env_config.get(&key).cloned().filter(|v| !v.is_empty())
        });

        let executor = StageExecutor {
            db: state.db.clone(),
            adapter_registry: AdapterRegistry::new(),
            workspace_manager: WorkspaceManager::new(WorkspaceManager::default_base_dir()),
        };

        let result = executor
            .execute(StageExecParams {
                task_id: task_id.clone(),
                project_name: project.name.clone(),
                repo_path,
                stage_name: "coding".into(),
                agent_type,
                prompt,
                model,
                env_config,
                needs_branch: true,
            })
            .await;

        let (status, error) = match result {
            Ok(_) => (TaskStatus::Success, None),
            Err(e) => (TaskStatus::Failed, Some(format!("{e}"))),
        };

        let _ = state
            .db
            .update_task_status(&task_id, status, error.as_deref())
            .await;
    });

    Ok((StatusCode::CREATED, Json(response)))
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..s.floor_char_boundary(max)]
    }
}

/// GET /api/task-types — 返回可用的任务类型列表（从 DB 读取）
pub async fn list_task_types(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<TaskTypeListResponse>> {
    let infos = crate::engine::task_type::list_task_types_from_db(&state.db)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(TaskTypeListResponse { task_types: infos }))
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskListQuery>,
) -> AppResult<Json<TaskListResponse>> {
    let limit = query
        .limit
        .unwrap_or(consts::DEFAULT_PAGE_SIZE)
        .min(consts::MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0);

    let status = query
        .status
        .as_deref()
        .map(|s| {
            serde_json::from_value::<TaskStatus>(serde_json::Value::String(s.to_string()))
                .map_err(|_| AppError::BadRequest(format!("invalid status filter: {s}")))
        })
        .transpose()?;

    let (models, total) = state
        .db
        .list_tasks(status, query.project_id.as_deref(), limit, offset)
        .await
        .map_err(AppError::Internal)?;

    let tasks: Vec<Task> = models.into_iter().map(Task::from).collect();

    Ok(Json(TaskListResponse {
        tasks,
        total: total as i32,
    }))
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Task>> {
    let model = state
        .db
        .get_task(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(Task::from(model)))
}

pub async fn get_task_stages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<StageRun>>> {
    // Verify task exists
    state
        .db
        .get_task(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    let models = state
        .db
        .list_stage_runs_by_task(&id.to_string())
        .await
        .map_err(AppError::Internal)?;

    let runs: Vec<StageRun> = models.into_iter().map(StageRun::from).collect();
    Ok(Json(runs))
}

pub async fn get_stage_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StageRun>> {
    let model = state
        .db
        .get_stage_run(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(StageRun::from(model)))
}

pub async fn cancel_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let task_id = id.to_string();
    let model = state
        .db
        .get_task(&task_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    if model.status != TaskStatus::Running && model.status != TaskStatus::Pending {
        return Err(AppError::BadRequest("Task is not running or pending".into()));
    }

    // 1. 查找所有 active stage runs → kill agent 进程
    let active_runs = state
        .db
        .get_active_stage_runs(&task_id)
        .await
        .map_err(AppError::Internal)?;

    for run in &active_runs {
        // Kill agent process by PID
        if let Some(pid) = run.agent_pid {
            kill_process_tree(pid);
        }
        // Update stage run status to Cancelled
        let _ = state
            .db
            .update_stage_run_status(
                &run.id,
                StageRunStatus::Cancelled,
                None,
                None,
            )
            .await;
    }

    // 2. Update task status
    state
        .db
        .update_task_status(&task_id, TaskStatus::Cancelled, None)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::OK)
}

/// Kill a process and its children. Sends SIGTERM first, then SIGKILL after 2s.
fn kill_process_tree(pid: i32) {
    use std::process::Command;

    // First try to kill the process group (negative PID)
    let _ = Command::new("kill").args(["-TERM", &format!("-{pid}")]).output();

    // Also kill the specific PID
    let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).output();

    // Schedule a SIGKILL after 2 seconds in case SIGTERM doesn't work
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let _ = Command::new("kill").args(["-9", &format!("-{pid}")]).output();
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
    });
}

/// POST /api/stage-runs/{id}/stop — stop a single running stage run
pub async fn stop_stage_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let run_id = id.to_string();
    let run = state
        .db
        .get_stage_run(&run_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    if run.status != StageRunStatus::Running {
        return Err(AppError::BadRequest("Stage run is not running".into()));
    }

    // Kill agent process
    if let Some(pid) = run.agent_pid {
        kill_process_tree(pid);
    }

    // Update stage run status to Cancelled
    state
        .db
        .update_stage_run_status(&run_id, StageRunStatus::Cancelled, None, None)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::OK)
}

/// GET /api/agents — returns available agent types + installation status
pub async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<AgentInfo>>> {
    let mut agents = Vec::new();
    for (agent_type, adapter) in state.adapter_registry.all() {
        let installed = adapter.check_installed().await.unwrap_or(false);
        agents.push(AgentInfo {
            agent_type: *agent_type,
            name: agent_type.as_str().to_string(),
            installed,
        });
    }
    Ok(Json(agents))
}

/// GET /api/settings — returns agent info + masked config from DB
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<crate::contracts::SettingsResponse>> {
    let items = state
        .db
        .get_all_config_items()
        .await
        .map_err(AppError::Internal)?;

    // Mask sensitive values
    let masked: Vec<ConfigItem> = items
        .into_iter()
        .map(|item| {
            if item.encrypted && !item.value.is_empty() {
                let masked_value = mask_value(&item.value);
                ConfigItem {
                    value: masked_value,
                    ..item
                }
            } else {
                item
            }
        })
        .collect();

    // Check agent installation status
    let mut agent_status = Vec::new();
    for (agent_type, adapter) in state.adapter_registry.all() {
        let installed = adapter.check_installed().await.unwrap_or(false);
        agent_status.push((*agent_type, installed));
    }

    Ok(Json(state.config.settings_response(masked, agent_status)))
}

/// PUT /api/settings — batch update config
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSettingsRequest>,
) -> AppResult<StatusCode> {
    for item in &req.config {
        state
            .db
            .set_config(&item.key, &item.value, item.encrypted)
            .await
            .map_err(AppError::Internal)?;
    }
    Ok(StatusCode::OK)
}

/// POST /api/settings/test-agent — test agent API key
pub async fn test_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TestAgentRequest>,
) -> AppResult<Json<TestResult>> {
    let config = state.db.get_all_config().await.map_err(AppError::Internal)?;

    let (key_name, test_url) = match req.agent_type {
        crate::contracts::AgentType::ClaudeCode => {
            let base = config
                .get("agent.claude-code.api_base_url")
                .cloned()
                .unwrap_or_else(|| consts::DEFAULT_API_BASE_URL.into());
            ("agent.claude-code.api_key", format!("{base}/v1/models"))
        }
        crate::contracts::AgentType::Codex => {
            let base = config
                .get("agent.codex.api_base_url")
                .cloned()
                .unwrap_or_else(|| "https://api.openai.com".into());
            ("agent.codex.api_key", format!("{base}/v1/models"))
        }
    };

    let api_key = config.get(key_name).cloned().unwrap_or_default();
    if api_key.is_empty() {
        return Ok(Json(TestResult {
            success: false,
            message: format!("No API key configured for {key_name}"),
        }));
    }

    match test_api_key(&test_url, &api_key).await {
        Ok(()) => Ok(Json(TestResult {
            success: true,
            message: "Connection successful".into(),
        })),
        Err(e) => Ok(Json(TestResult {
            success: false,
            message: format!("Connection failed: {e}"),
        })),
    }
}

/// POST /api/settings/test-tool — test tool API key
pub async fn test_tool(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TestToolRequest>,
) -> AppResult<Json<TestResult>> {
    let config = state.db.get_all_config().await.map_err(AppError::Internal)?;

    match req.tool.as_str() {
        "tavily" => {
            let key = config
                .get("tool.tavily.api_key")
                .cloned()
                .unwrap_or_default();
            if key.is_empty() {
                return Ok(Json(TestResult {
                    success: false,
                    message: "No Tavily API key configured".into(),
                }));
            }
            let client = reqwest::Client::new();
            let resp = client
                .post("https://api.tavily.com/search")
                .json(&serde_json::json!({
                    "api_key": key,
                    "query": "test",
                    "max_results": 1,
                }))
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => Ok(Json(TestResult {
                    success: true,
                    message: "Tavily API key is valid".into(),
                })),
                Ok(r) => Ok(Json(TestResult {
                    success: false,
                    message: format!("Tavily returned status {}", r.status()),
                })),
                Err(e) => Ok(Json(TestResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                })),
            }
        }
        _ => Err(AppError::BadRequest(format!("Unknown tool: {}", req.tool))),
    }
}

async fn test_api_key(url: &str, key: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {key}"))
        .header("x-api-key", key)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("API returned status {}", resp.status()))
    }
}

fn mask_value(value: &str) -> String {
    if value.len() <= 8 {
        return "***".into();
    }
    let prefix = &value[..3];
    let suffix = &value[value.len() - 4..];
    format!("{prefix}***{suffix}")
}

// ── Error handling ──

pub(crate) enum AppError {
    NotFound,
    BadRequest(String),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(e) => {
                tracing::error!("Internal error: {e:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::adapter::AdapterRegistry;
    use crate::api;
    use crate::config::PlatformConfig;
    use crate::contracts::{CreateTaskResponse, TaskListResponse, UpdateSettingsRequest, ConfigItem};
    use crate::db::Database;
    use crate::AppState;
    use std::sync::Arc;

    async fn test_app() -> (axum::Router, Arc<AppState>) {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        let config = PlatformConfig {
            default_model: "test-model".into(),
        };
        let state = Arc::new(AppState {
            db,
            adapter_registry: AdapterRegistry::new(),
            config,
        });
        (api::router(state.clone()), state)
    }

    #[tokio::test]
    async fn create_task_returns_201() {
        let (app, _) = test_app().await;

        let body = serde_json::json!({
            "title": "Test task",
            "prompt": "Do something"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: CreateTaskResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.status, crate::contracts::TaskStatus::Pending);
    }

    #[tokio::test]
    async fn list_tasks_empty() {
        let (app, _) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: TaskListResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.total, 0);
        assert!(resp.tasks.is_empty());
    }

    #[tokio::test]
    async fn get_nonexistent_task_returns_404() {
        let (app, _) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tasks/00000000-0000-0000-0000-000000000000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn settings_returns_agents() {
        let (app, _) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: crate::contracts::SettingsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.agents.len(), 2);
        let names: Vec<&str> = resp.agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"Claude Code"));
        assert!(names.contains(&"Codex"));
        assert!(resp.config.is_empty());
    }

    #[tokio::test]
    async fn put_settings_then_get_masked() {
        let (_, state) = test_app().await;

        state.db.set_config("agent.claude-code.api_key", "sk-ant-very-long-key-1234", true).await.unwrap();
        state.db.set_config("agent.claude-code.default_model", "sonnet", false).await.unwrap();

        let app = api::router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: crate::contracts::SettingsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.config.len(), 2);

        let api_key_item = resp.config.iter().find(|i| i.key == "agent.claude-code.api_key").unwrap();
        assert!(api_key_item.encrypted);
        assert!(api_key_item.value.contains("***"));
        assert!(!api_key_item.value.contains("very-long"));

        let model_item = resp.config.iter().find(|i| i.key == "agent.claude-code.default_model").unwrap();
        assert!(!model_item.encrypted);
        assert_eq!(model_item.value, "sonnet");
    }

    #[tokio::test]
    async fn put_settings_updates_config() {
        let (_, state) = test_app().await;
        let app = api::router(state.clone());

        let req = UpdateSettingsRequest {
            config: vec![
                ConfigItem { key: "agent.claude-code.api_key".into(), value: "sk-new".into(), encrypted: true },
                ConfigItem { key: "tool.tavily.api_key".into(), value: "tvly-abc".into(), encrypted: true },
            ],
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let val = state.db.get_config("agent.claude-code.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("sk-new"));
        let val = state.db.get_config("tool.tavily.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("tvly-abc"));
    }
}
