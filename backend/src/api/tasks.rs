use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::consts;
use crate::container::manager::ContainerRuntime;
use crate::contracts::{
    ConfigItem, CreateTaskRequest, CreateTaskResponse, Task, TaskListQuery, TaskListResponse,
    TaskLogsResponse, TaskStatus, TestAgentRequest, TestResult, TestToolRequest,
    UpdateSettingsRequest,
};
use crate::db::{Database, TaskReportUpdate};
use crate::entity::task;
use crate::AppState;

type AppResult<T> = Result<T, AppError>;

pub async fn create_task<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<CreateTaskRequest>,
) -> AppResult<impl IntoResponse> {
    let model = state
        .db
        .create_task(&req, &state.config.default_model)
        .await
        .map_err(AppError::Internal)?;

    let response = CreateTaskResponse {
        id: model.id.clone(),
        status: model.status,
        created_at: model.created_at,
    };

    // Spawn async container execution
    let task_id = model.id.clone();
    let db = state.db.clone();
    let runtime = state.runtime.clone();
    let config = state.config.clone();

    tokio::spawn(async move {
        execute_task(task_id, model, db, runtime, config).await;
    });

    Ok((StatusCode::CREATED, Json(response)))
}

async fn execute_task<R: ContainerRuntime>(
    task_id: String,
    task_model: task::Model,
    db: Database,
    runtime: Arc<tokio::sync::Mutex<R>>,
    config: crate::config::PlatformConfig,
) {
    // Update to running
    if let Err(e) = db
        .update_task_status(&task_id, TaskStatus::Running, None, None)
        .await
    {
        tracing::error!("Failed to update task {task_id} to running: {e}");
        return;
    }

    // Read dynamic config from DB
    let env_config = db.get_all_config().await.unwrap_or_default();

    // Run container
    let result = {
        let rt = runtime.lock().await;
        rt.run_task(&task_model, &config, &env_config).await
    };

    match result {
        Ok(run_result) => {
            // Update container_id
            let _ = db
                .update_task_status(
                    &task_id,
                    TaskStatus::Running,
                    Some(&run_result.container_id),
                    None,
                )
                .await;

            // Update report data
            if let Some(ref report) = run_result.report {
                let files_csv = if report.files_changed.is_empty() {
                    None
                } else {
                    Some(report.files_changed.join(","))
                };

                let db_report = TaskReportUpdate {
                    agent_exit_code: Some(report.agent_exit_code),
                    duration_seconds: Some(report.duration_seconds),
                    pushed: report.pushed,
                    lines_added: report.lines_added,
                    lines_removed: report.lines_removed,
                    files_changed: files_csv,
                    summary: run_result.summary.clone(),
                    diff_patch: run_result.diff_patch.clone(),
                };
                let _ = db.update_task_report(&task_id, &db_report).await;
            }

            // Store agent log
            if let Some(ref log) = run_result.agent_log {
                let _ = db.update_task_logs(&task_id, log).await;
            }

            let status = if run_result.exit_code == 0 {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            };

            let error = if run_result.exit_code != 0 {
                Some(format!(
                    "Container exited with code {}",
                    run_result.exit_code
                ))
            } else {
                None
            };

            let _ = db
                .update_task_status(&task_id, status, None, error.as_deref())
                .await;
        }
        Err(e) => {
            tracing::error!("Container execution failed for task {task_id}: {e}");
            let _ = db
                .update_task_status(
                    &task_id,
                    TaskStatus::Failed,
                    None,
                    Some(&format!("Container error: {e}")),
                )
                .await;
        }
    }
}

pub async fn list_tasks<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Query(query): Query<TaskListQuery>,
) -> AppResult<Json<TaskListResponse>> {
    let limit = query.limit.unwrap_or(consts::DEFAULT_PAGE_SIZE).min(consts::MAX_PAGE_SIZE);
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
        .list_tasks(status, limit, offset)
        .await
        .map_err(AppError::Internal)?;

    let tasks: Vec<Task> = models.into_iter().map(Task::from).collect();

    Ok(Json(TaskListResponse {
        tasks,
        total: total as i32,
    }))
}

pub async fn get_task<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
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

pub async fn get_task_logs<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TaskLogsResponse>> {
    let logs = state
        .db
        .get_task_logs(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(TaskLogsResponse { logs }))
}

pub async fn cancel_task<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let model = state
        .db
        .get_task(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    if model.status != TaskStatus::Running {
        return Err(AppError::BadRequest("Task is not running".into()));
    }

    if let Some(ref container_id) = model.container_id {
        let rt = state.runtime.lock().await;
        rt.cancel_container(container_id).await.ok();
    }

    state
        .db
        .update_task_status(&id.to_string(), TaskStatus::Cancelled, None, None)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::OK)
}

/// GET /api/settings — returns agent info + masked config from DB
pub async fn get_settings<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
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

    Ok(Json(state.config.settings_response(masked)))
}

/// PUT /api/settings — batch update config
pub async fn update_settings<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
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
pub async fn test_agent<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
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

    // Simple HTTP test
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
pub async fn test_tool<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<TestToolRequest>,
) -> AppResult<Json<TestResult>> {
    let config = state.db.get_all_config().await.map_err(AppError::Internal)?;

    match req.tool.as_str() {
        "tavily" => {
            let key = config.get("tool.tavily.api_key").cloned().unwrap_or_default();
            if key.is_empty() {
                return Ok(Json(TestResult {
                    success: false,
                    message: "No Tavily API key configured".into(),
                }));
            }
            // Test Tavily with a simple search
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

    if resp.status().is_success() || resp.status().as_u16() == 401 {
        // 401 means we reached the API, key format may be wrong but endpoint works
        if resp.status().is_success() {
            Ok(())
        } else {
            Err("API key rejected (401 Unauthorized)".into())
        }
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

/// GET /api/settings/images — check image build status
pub async fn get_image_status<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
) -> AppResult<Json<Vec<crate::container::images::ImageStatus>>> {
    let rt = state.runtime.lock().await;
    let statuses = rt
        .check_image_status(&state.config)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(statuses))
}

/// POST /api/settings/images/build — trigger image builds (async, returns 202)
pub async fn build_images<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
) -> AppResult<StatusCode> {
    let runtime = state.runtime.clone();
    let config = state.config.clone();

    tokio::spawn(async move {
        let rt = runtime.lock().await;
        if let Err(e) = rt.build_all_images(&config).await {
            tracing::error!("Image build failed: {e}");
        }
    });

    Ok(StatusCode::ACCEPTED)
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

    use crate::api;
    use crate::config::PlatformConfig;
    use crate::container::manager::tests::MockRuntime;
    use crate::contracts::{CreateTaskResponse, TaskListResponse, UpdateSettingsRequest, ConfigItem};
    use crate::db::Database;
    use crate::AppState;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    async fn test_app() -> (axum::Router, Arc<AppState<MockRuntime>>) {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        let runtime = MockRuntime::new();
        let config = PlatformConfig {
            cc_image: "test:latest".into(),
            codex_image: "test-codex:latest".into(),
            container_memory_limit: 1024,
            container_cpu_quota: 100000,
            default_model: "test-model".into(),
        };
        let state = Arc::new(AppState {
            db,
            runtime: Arc::new(Mutex::new(runtime)),
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
    async fn settings_returns_agents_and_empty_config() {
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
        assert_eq!(resp.agents[0].name, "Claude Code");
        assert_eq!(resp.agents[1].name, "Codex");
        assert!(resp.config.is_empty());
    }

    #[tokio::test]
    async fn put_settings_then_get_masked() {
        let (_, state) = test_app().await;

        // Set config via DB directly
        state.db.set_config("agent.claude-code.api_key", "sk-ant-very-long-key-1234", true).await.unwrap();
        state.db.set_config("agent.claude-code.default_model", "sonnet", false).await.unwrap();

        // GET settings — encrypted values should be masked
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

        // Verify in DB
        let val = state.db.get_config("agent.claude-code.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("sk-new"));
        let val = state.db.get_config("tool.tavily.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("tvly-abc"));
    }
}
