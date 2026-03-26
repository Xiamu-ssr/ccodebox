pub mod tasks;

use std::sync::Arc;

use axum::routing::{get, post, put};
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
        .route("/api/settings", put(tasks::update_settings::<R>))
        .route(
            "/api/settings/test-agent",
            post(tasks::test_agent::<R>),
        )
        .route(
            "/api/settings/test-tool",
            post(tasks::test_tool::<R>),
        )
        .route(
            "/api/settings/images",
            get(tasks::get_image_status::<R>),
        )
        .route(
            "/api/settings/images/build",
            post(tasks::build_images::<R>),
        )
        .with_state(state)
        .fallback(crate::frontend::serve_frontend)
}
