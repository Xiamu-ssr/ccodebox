use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::container::manager::ContainerRuntime;
use crate::models::task::{
    CreateTaskRequest, CreateTaskResponse, TaskListQuery, TaskListResponse, TaskLogsResponse,
    TaskStatus,
};
use crate::AppState;

type AppResult<T> = Result<T, AppError>;

pub async fn create_task<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<CreateTaskRequest>,
) -> AppResult<impl IntoResponse> {
    let task = state
        .db
        .create_task(&req, &state.config.default_model)
        .await
        .map_err(AppError::Internal)?;

    let response = CreateTaskResponse {
        id: task.id,
        status: task.status,
        created_at: task.created_at,
    };

    // Spawn async container execution
    let task_id = task.id;
    let db = state.db.clone();
    let runtime = state.runtime.clone();
    let config = state.config.clone();

    tokio::spawn(async move {
        execute_task(task_id, task, db, runtime, config).await;
    });

    Ok((StatusCode::CREATED, Json(response)))
}

async fn execute_task<R: ContainerRuntime>(
    task_id: Uuid,
    task: crate::models::task::Task,
    db: crate::db::Database,
    runtime: Arc<tokio::sync::Mutex<R>>,
    config: crate::models::settings::PlatformConfig,
) {
    // Update to running
    if let Err(e) = db
        .update_task_status(task_id, TaskStatus::Running, None, None)
        .await
    {
        tracing::error!("Failed to update task {task_id} to running: {e}");
        return;
    }

    // Run container
    let result = {
        let rt = runtime.lock().await;
        rt.run_task(&task, &config).await
    };

    match result {
        Ok(run_result) => {
            // Update container_id
            let _ = db
                .update_task_status(
                    task_id,
                    TaskStatus::Running,
                    Some(&run_result.container_id),
                    None,
                )
                .await;

            // Update report data
            if let Some(ref report) = run_result.report {
                let db_report = crate::db::TaskReportUpdate {
                    rounds_used: report.rounds,
                    lint_status: Some(&report.lint_status),
                    test_status: Some(&report.test_status),
                    lines_added: report.lines_added,
                    lines_removed: report.lines_removed,
                    files_changed: Some(&report.files_changed),
                    summary: run_result.summary.as_deref(),
                    diff_patch: run_result.diff_patch.as_deref(),
                };
                let _ = db.update_task_report(task_id, &db_report).await;
            }

            // Store logs
            if !run_result.logs.is_empty() {
                let combined_logs = run_result
                    .logs
                    .iter()
                    .enumerate()
                    .map(|(i, log)| format!("=== Round {} ===\n{}", i + 1, log))
                    .collect::<Vec<_>>()
                    .join("\n---\n");

                let _ = db
                    .update_task_logs(task_id, &combined_logs, run_result.logs.len() as i32)
                    .await;
            }

            let status = if run_result.exit_code == 0 {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            };

            let error = if run_result.exit_code != 0 {
                Some(format!("Container exited with code {}", run_result.exit_code))
            } else {
                None
            };

            let _ = db
                .update_task_status(task_id, status, None, error.as_deref())
                .await;
        }
        Err(e) => {
            tracing::error!("Container execution failed for task {task_id}: {e}");
            let _ = db
                .update_task_status(
                    task_id,
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
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let (tasks, total) = state
        .db
        .list_tasks(query.status.as_deref(), limit, offset)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(TaskListResponse { tasks, total }))
}

pub async fn get_task<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<crate::models::task::Task>> {
    let task = state
        .db
        .get_task(id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(task))
}

pub async fn get_task_logs<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TaskLogsResponse>> {
    let (logs, rounds) = state
        .db
        .get_task_logs(id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(TaskLogsResponse { logs, rounds }))
}

pub async fn cancel_task<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let task = state
        .db
        .get_task(id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    if task.status != TaskStatus::Running {
        return Err(AppError::BadRequest("Task is not running".into()));
    }

    if let Some(ref container_id) = task.container_id {
        let rt = state.runtime.lock().await;
        rt.cancel_container(container_id).await.ok();
    }

    state
        .db
        .update_task_status(id, TaskStatus::Cancelled, None, None)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::OK)
}

pub async fn get_settings<R: ContainerRuntime>(
    State(state): State<Arc<AppState<R>>>,
) -> Json<crate::models::settings::SettingsResponse> {
    Json(state.config.settings_response())
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
    use crate::container::manager::tests::MockRuntime;
    use crate::db::Database;
    use crate::models::settings::PlatformConfig;
    use crate::models::task::{CreateTaskResponse, TaskListResponse};
    use crate::AppState;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    async fn test_app() -> axum::Router {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        let runtime = MockRuntime::new();
        let config = PlatformConfig {
            cc_image: "test:latest".into(),
            cc_api_base_url: "http://localhost".into(),
            cc_api_key: "test-key".into(),
            container_memory_limit: 1024,
            container_cpu_quota: 100000,
            default_model: "test-model".into(),
            max_rounds_limit: 5,
        };
        let state = Arc::new(AppState {
            db,
            runtime: Arc::new(Mutex::new(runtime)),
            config,
        });
        api::router(state)
    }

    #[tokio::test]
    async fn create_task_returns_201() {
        let app = test_app().await;

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
        assert_eq!(resp.status, crate::models::task::TaskStatus::Pending);
    }

    #[tokio::test]
    async fn list_tasks_empty() {
        let app = test_app().await;

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
        let app = test_app().await;

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
    async fn settings_returns_agent_config() {
        let app = test_app().await;

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
        let resp: crate::models::settings::SettingsResponse =
            serde_json::from_slice(&body).unwrap();
        assert!(!resp.agents.is_empty());
        assert_eq!(resp.max_rounds_limit, 5);
    }
}
