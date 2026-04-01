pub mod projects;
pub mod tasks;
pub mod templates;

use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Tasks
        .route("/api/tasks", post(tasks::create_task))
        .route("/api/tasks", get(tasks::list_tasks))
        .route("/api/tasks/{id}", get(tasks::get_task))
        .route("/api/tasks/{id}/stages", get(tasks::get_task_stages))
        .route("/api/tasks/{id}/cancel", post(tasks::cancel_task))
        // Run (single stage shortcut)
        .route("/api/run", post(tasks::run_stage))
        // Task types (legacy compat — reads from DB templates)
        .route("/api/task-types", get(tasks::list_task_types))
        // Templates (CRUD)
        .route("/api/templates", get(templates::list_templates))
        .route("/api/templates", post(templates::create_template))
        .route("/api/templates/{name}", get(templates::get_template))
        .route("/api/templates/{name}", put(templates::update_template))
        .route("/api/templates/{name}", delete(templates::delete_template))
        // Projects
        .route("/api/projects", post(projects::create_project))
        .route("/api/projects", get(projects::list_projects))
        .route("/api/projects/{id}", get(projects::get_project))
        .route("/api/projects/{id}", delete(projects::delete_project))
        // Stage runs
        .route("/api/stage-runs/{id}", get(tasks::get_stage_run))
        .route("/api/stage-runs/{id}/stop", post(tasks::stop_stage_run))
        // Agents
        .route("/api/agents", get(tasks::list_agents))
        // Settings
        .route("/api/settings", get(tasks::get_settings))
        .route("/api/settings", put(tasks::update_settings))
        .route("/api/settings/test-agent", post(tasks::test_agent))
        .route("/api/settings/test-tool", post(tasks::test_tool))
        .with_state(state)
        .fallback(crate::frontend::serve_frontend)
}
