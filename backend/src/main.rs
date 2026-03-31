mod adapter;
mod api;
mod config;
mod consts;
mod contracts;
mod db;
mod engine;
mod entity;
mod frontend;
mod workspace;

use std::sync::Arc;

use anyhow::Result;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use adapter::AdapterRegistry;
use config::PlatformConfig;
use contracts::AgentType;
use db::Database;
use engine::stage::{StageExecParams, StageExecutor};
use workspace::WorkspaceManager;

pub struct AppState {
    pub db: Database,
    pub adapter_registry: AdapterRegistry,
    pub config: PlatformConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str());

    match command {
        Some("version") => {
            println!("ccodebox {}", env!("CARGO_PKG_VERSION"));
        }
        Some("serve") => {
            init_tracing();
            cmd_serve().await?;
        }
        Some("project") => {
            init_tracing();
            cmd_project(&args[2..]).await?;
        }
        Some("run") => {
            init_tracing();
            cmd_run(&args[2..]).await?;
        }
        Some("agent") => {
            cmd_agent(&args[2..]).await?;
        }
        None => {
            init_tracing();
            cmd_serve().await?;
        }
        Some(other) => {
            eprintln!("Unknown command: {other}");
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
}

fn print_usage() {
    eprintln!(
        "Usage: ccodebox <command>

Commands:
  serve                          Start web server (default)
  project add --name <n> --path <p>  Register a project
  project list                   List projects
  run --project <n> --agent <a> \"prompt\"  Run a single stage
  agent list                     List available agents
  agent check                    Check agent installation
  version                        Show version"
    );
}

// ── serve ──

async fn cmd_serve() -> Result<()> {
    let host = std::env::var("CCODEBOX_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("CCODEBOX_PORT").unwrap_or_else(|_| "3000".into());

    let db = open_db().await?;
    let adapter_registry = AdapterRegistry::new();
    let config = PlatformConfig::from_env();

    let state = Arc::new(AppState {
        db,
        adapter_registry,
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

// ── project ──

async fn cmd_project(args: &[String]) -> Result<()> {
    let sub = args.first().map(|s| s.as_str());

    match sub {
        Some("add") => {
            let name = get_flag(args, "--name")
                .ok_or_else(|| anyhow::anyhow!("--name is required"))?;
            let path = get_flag(args, "--path");
            let repo = get_flag(args, "--repo");

            if path.is_none() && repo.is_none() {
                anyhow::bail!("--path or --repo is required");
            }

            let db = open_db().await?;
            let project = db
                .create_project(&contracts::CreateProjectRequest {
                    name: name.clone(),
                    local_path: path,
                    repo_url: repo,
                    default_agent: get_flag(args, "--agent"),
                })
                .await?;

            println!("Project created: {} ({})", project.name, project.id);
        }
        Some("list") => {
            let db = open_db().await?;
            let projects = db.list_projects().await?;

            if projects.is_empty() {
                println!("No projects registered.");
            } else {
                println!("{:<20} {:<40} {}", "NAME", "PATH", "ID");
                for p in projects {
                    println!(
                        "{:<20} {:<40} {}",
                        p.name,
                        p.local_path.as_deref().unwrap_or("-"),
                        p.id
                    );
                }
            }
        }
        _ => {
            eprintln!("Usage: ccodebox project <add|list>");
            std::process::exit(1);
        }
    }

    Ok(())
}

// ── run ──

async fn cmd_run(args: &[String]) -> Result<()> {
    let project_name = get_flag(args, "--project")
        .ok_or_else(|| anyhow::anyhow!("--project is required"))?;
    let agent_str = get_flag(args, "--agent").unwrap_or_else(|| "claude-code".into());
    let model = get_flag(args, "--model");

    // 最后一个非 flag 参数是 prompt
    let prompt = args
        .iter()
        .filter(|a| !a.starts_with("--") && !is_flag_value(args, a))
        .last()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Prompt is required (last argument)"))?;

    let agent_type: AgentType =
        serde_json::from_value(serde_json::Value::String(agent_str.clone()))
            .map_err(|_| anyhow::anyhow!("Unknown agent: {agent_str}"))?;

    let db = open_db().await?;

    let project = db
        .get_project_by_name(&project_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Project not found: {project_name}"))?;

    let repo_path = project
        .local_path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Project has no local_path"))?;

    let env_config = db.get_all_config().await.unwrap_or_default();

    // 创建 task 记录
    let task = db
        .create_task(&contracts::CreateTaskRequest {
            title: format!("CLI: {}", truncate_str(&prompt, 50)),
            prompt: prompt.clone(),
            project_id: Some(project.id.clone()),
            task_type: Some("single-stage".into()),
            inputs: None,
        })
        .await?;

    db.update_task_status(&task.id, contracts::TaskStatus::Running, None)
        .await?;

    println!("Task {} created, executing...", task.id);

    let executor = StageExecutor {
        db: db.clone(),
        adapter_registry: AdapterRegistry::new(),
        workspace_manager: WorkspaceManager::new(WorkspaceManager::default_base_dir()),
    };

    let result = executor
        .execute(StageExecParams {
            task_id: task.id.clone(),
            project_name: project.name.clone(),
            repo_path: repo_path.clone(),
            stage_name: "coding".into(),
            agent_type,
            prompt,
            model,
            env_config,
            needs_branch: true,
        })
        .await;

    match result {
        Ok(stage_run_id) => {
            db.update_task_status(&task.id, contracts::TaskStatus::Success, None)
                .await?;

            let sr = db.get_stage_run(&stage_run_id).await?.unwrap();
            println!("Done! Stage run: {}", sr.id);
            if let Some(ws) = &sr.workspace_path {
                println!("Workspace: {ws}");
            }
            if let Some(branch) = &sr.branch {
                println!("Branch: {branch}");
            }
            if let Some(duration) = sr.duration_seconds {
                println!("Duration: {duration}s");
            }
            if let Some(summary) = &sr.summary {
                println!("\n--- Summary ---\n{summary}");
            }
        }
        Err(e) => {
            db.update_task_status(&task.id, contracts::TaskStatus::Failed, Some(&format!("{e}")))
                .await?;
            eprintln!("Execution failed: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}

// ── agent ──

async fn cmd_agent(args: &[String]) -> Result<()> {
    let sub = args.first().map(|s| s.as_str());
    let registry = AdapterRegistry::new();

    match sub {
        Some("list") | Some("check") => {
            println!("{:<15} {}", "AGENT", "INSTALLED");
            for (agent_type, adapter) in registry.all() {
                let installed = adapter.check_installed().await.unwrap_or(false);
                let mark = if installed { "yes" } else { "no" };
                println!("{:<15} {}", agent_type.as_str(), mark);
            }
        }
        _ => {
            eprintln!("Usage: ccodebox agent <list|check>");
            std::process::exit(1);
        }
    }

    Ok(())
}

// ── helpers ──

async fn open_db() -> Result<Database> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ccodebox")
            .join("data");
        format!("sqlite:{}/ccodebox.db", data_dir.display())
    });
    let db = Database::new(&database_url).await?;
    db.migrate().await?;
    Ok(db)
}

fn get_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn is_flag_value(args: &[String], val: &str) -> bool {
    args.iter().any(|a| {
        a.starts_with("--")
            && args
                .iter()
                .position(|x| x == a)
                .and_then(|i| args.get(i + 1))
                .map(|v| v == val)
                .unwrap_or(false)
    })
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..s.floor_char_boundary(max)]
    }
}
