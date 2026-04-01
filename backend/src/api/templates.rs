use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::contracts::{
    CreateTemplateRequest, Template, TemplateListResponse, UpdateTemplateRequest,
};
use crate::AppState;

use super::tasks::AppError;

type AppResult<T> = Result<T, AppError>;

/// GET /api/templates
pub async fn list_templates(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<TemplateListResponse>> {
    let models = state.db.list_templates().await.map_err(AppError::Internal)?;
    let templates: Vec<Template> = models.into_iter().map(Template::from).collect();
    Ok(Json(TemplateListResponse { templates }))
}

/// GET /api/templates/:name
pub async fn get_template(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> AppResult<Json<Template>> {
    let model = state
        .db
        .get_template_by_name(&name)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;
    Ok(Json(Template::from(model)))
}

/// POST /api/templates
pub async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTemplateRequest>,
) -> AppResult<impl IntoResponse> {
    // Validate YAML parses
    serde_yaml::from_str::<crate::engine::task_type::TaskTypeDefinition>(&req.definition)
        .map_err(|e| AppError::BadRequest(format!("Invalid YAML: {e}")))?;

    // Check name uniqueness
    if state.db.get_template_by_name(&req.name).await.map_err(AppError::Internal)?.is_some() {
        return Err(AppError::BadRequest(format!(
            "Template '{}' already exists",
            req.name
        )));
    }

    let model = state
        .db
        .create_template(&req, false)
        .await
        .map_err(AppError::Internal)?;

    Ok((StatusCode::CREATED, Json(Template::from(model))))
}

/// PUT /api/templates/:name
pub async fn update_template(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<UpdateTemplateRequest>,
) -> AppResult<Json<Template>> {
    // Validate YAML if provided
    if let Some(def) = &req.definition {
        serde_yaml::from_str::<crate::engine::task_type::TaskTypeDefinition>(def)
            .map_err(|e| AppError::BadRequest(format!("Invalid YAML: {e}")))?;
    }

    let model = state
        .db
        .update_template(&name, &req)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(Template::from(model)))
}

/// DELETE /api/templates/:name
pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> AppResult<StatusCode> {
    state
        .db
        .delete_template(&name)
        .await
        .map_err(|e| {
            if e.to_string().contains("Cannot delete builtin") {
                AppError::BadRequest(e.to_string())
            } else {
                AppError::Internal(e)
            }
        })?;
    Ok(StatusCode::OK)
}
