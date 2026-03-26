pub mod tasks;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::container::manager::ContainerRuntime;
use crate::AppState;

pub fn router<R: ContainerRuntime>(state: Arc<AppState<R>>) -> Router {
    Router::new()
        .route("/api/tasks", post(tasks::create_task::<R>))
        .route("/api/tasks", get(tasks::list_tasks::<R>))
        .route("/api/tasks/{id}", get(tasks::get_task::<R>))
        .route("/api/tasks/{id}/logs", get(tasks::get_task_logs::<R>))
        .route("/api/tasks/{id}/cancel", post(tasks::cancel_task::<R>))
        .route("/api/settings", get(tasks::get_settings::<R>))
        .with_state(state)
}
