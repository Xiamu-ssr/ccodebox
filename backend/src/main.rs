mod api;
mod config;
mod consts;
mod container;
mod contracts;
mod db;
mod entity;
mod frontend;

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use config::PlatformConfig;
use container::manager::{BollardRuntime, ContainerRuntime};
use db::Database;

pub struct AppState<R: ContainerRuntime> {
    pub db: Database,
    pub runtime: Arc<Mutex<R>>,
    pub config: PlatformConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str());

    match command {
        Some("version") => {
            println!("ccodebox {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("setup") => {
            let runtime = BollardRuntime::new()?;
            let config = PlatformConfig::from_env();
            tracing::info!("Building all Docker images...");
            runtime.build_all_images(&config).await?;
            tracing::info!("All images built successfully");
            return Ok(());
        }
        Some("serve") | None => {} // fall through to server
        Some(other) => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: ccodebox [serve|setup|version]");
            std::process::exit(1);
        }
    }

    let host = std::env::var("CCODEBOX_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("CCODEBOX_PORT").unwrap_or_else(|_| "3000".into());
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            let data_dir = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".ccodebox")
                .join("data");
            format!("sqlite:{}/ccodebox.db", data_dir.display())
        });

    let db = Database::new(&database_url).await?;
    db.migrate().await?;

    let runtime = BollardRuntime::new()?;
    let config = PlatformConfig::from_env();

    // Check and build missing Docker images on startup
    if let Err(e) = runtime.ensure_images(&config).await {
        tracing::warn!("Could not ensure Docker images: {e}");
        tracing::warn!("Run `ccodebox setup` to build images manually");
    }

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
    tracing::info!("Starting CCodeBoX on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
