use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::contracts::{CreateProjectRequest, Project, ProjectListResponse};
use crate::AppState;

use super::tasks::AppError;

type AppResult<T> = Result<T, AppError>;

/// POST /api/projects
/// 如果只有 repo_url 没有 local_path，自动 clone 到 ~/.ccodebox/repos/{name}
pub async fn create_project(
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<CreateProjectRequest>,
) -> AppResult<impl IntoResponse> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    // 如果有 repo_url 但没有 local_path → 自动 clone
    if req.local_path.is_none() {
        if let Some(repo_url) = &req.repo_url {
            let repos_dir = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".ccodebox")
                .join("repos")
                .join(&req.name);

            let repos_dir_str = repos_dir.to_str().unwrap_or("").to_string();

            // 如果目录已存在且是 git repo，跳过 clone
            if !repos_dir.join(".git").exists() {
                tokio::fs::create_dir_all(&repos_dir)
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?;

                let output = tokio::process::Command::new("git")
                    .args(["clone", repo_url, &repos_dir_str])
                    .output()
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // clone 失败，清理目录
                    tokio::fs::remove_dir_all(&repos_dir).await.ok();
                    return Err(AppError::BadRequest(format!(
                        "git clone failed: {stderr}"
                    )));
                }
            }

            req.local_path = Some(repos_dir_str);
        } else {
            return Err(AppError::BadRequest(
                "repo_url is required (or use CLI with --from for local repos)".into(),
            ));
        }
    } else {
        // 有 local_path，验证是 git repo
        let path = req.local_path.as_ref().unwrap();
        let git_dir = std::path::Path::new(path).join(".git");
        if !git_dir.exists() {
            return Err(AppError::BadRequest(format!(
                "local_path is not a git repo: {path}"
            )));
        }
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
    Path(id): Path<String>,
) -> AppResult<Json<Project>> {
    let model = state
        .db
        .get_project(&id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(Project::from(model)))
}

pub async fn delete_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    state
        .db
        .get_project(&id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    state
        .db
        .delete_project(&id)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::OK)
}
