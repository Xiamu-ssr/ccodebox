use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::contracts::{CreateProjectRequest, Project, ProjectListResponse};
use crate::AppState;

use super::tasks::AppError;

type AppResult<T> = Result<T, AppError>;

pub async fn create_project(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProjectRequest>,
) -> AppResult<impl IntoResponse> {
    if req.repo_url.is_none() && req.local_path.is_none() {
        return Err(AppError::BadRequest(
            "repo_url or local_path must be provided".into(),
        ));
    }

    let model = state
        .db
        .create_project(&req)
        .await
        .map_err(AppError::Internal)?;

    Ok((StatusCode::CREATED, Json(Project::from(model))))
}

pub async fn list_projects(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ProjectListResponse>> {
    let models = state
        .db
        .list_projects()
        .await
        .map_err(AppError::Internal)?;

    let projects: Vec<Project> = models.into_iter().map(Project::from).collect();
    Ok(Json(ProjectListResponse { projects }))
}

pub async fn get_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Project>> {
    let model = state
        .db
        .get_project(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(Project::from(model)))
}

pub async fn delete_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    state
        .db
        .get_project(&id.to_string())
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    state
        .db
        .delete_project(&id.to_string())
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::OK)
}
