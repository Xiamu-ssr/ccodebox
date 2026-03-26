mod api;
mod container;
mod db;
mod models;

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use container::manager::BollardRuntime;
use db::Database;

pub struct AppState<R: container::manager::ContainerRuntime> {
    pub db: Database,
    pub runtime: Arc<Mutex<R>>,
    pub config: models::settings::PlatformConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let host = std::env::var("CCODEBOX_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("CCODEBOX_PORT").unwrap_or_else(|_| "3000".into());
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/ccodebox.db".into());

    let db = Database::new(&database_url).await?;
    db.migrate().await?;

    let runtime = BollardRuntime::new()?;
    let config = models::settings::PlatformConfig::from_env();

    let state = Arc::new(AppState {
        db,
        runtime: Arc::new(Mutex::new(runtime)),
        config,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = api::router(state).layer(cors);

    let addr = format!("{host}:{port}");
    tracing::info!("Starting CCodeBoX backend on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
