use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database as SeaDatabase,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Schema,
    Set,
};
use sea_orm::sea_query::OnConflict;

use crate::consts;
use crate::contracts::{ConfigItem, CreateProjectRequest, CreateTaskRequest, CreateTemplateRequest, UpdateTemplateRequest, StageRunStatus, TaskStatus};
use crate::entity::{platform_config, project, stage_run, task, template};

/// Stage 执行报告，批量更新 stage_run 记录
pub struct StageRunReport {
    pub exit_code: Option<i32>,
    pub duration: Option<i32>,
    pub agent_log: Option<String>,
    pub diff_patch: Option<String>,
    pub summary: Option<String>,
    pub error_report: Option<String>,
    pub prompt_used: Option<String>,
}

#[derive(Clone)]
pub struct Database {
    conn: DatabaseConnection,
}

impl Database {
    pub async fn new(url: &str) -> Result<Self> {
        let connect_url = if url == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            let path = url.strip_prefix("sqlite:").unwrap_or(url);
            if let Some(parent) = std::path::Path::new(path).parent()
                && !parent.as_os_str().is_empty()
            {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            if url.starts_with("sqlite:") {
                format!("{url}?mode=rwc")
            } else {
                format!("sqlite:{url}?mode=rwc")
            }
        };

        let mut opts = ConnectOptions::new(connect_url);
        if url == ":memory:" {
            opts.max_connections(1);
        } else {
            opts.max_connections(5);
        }
        opts.sqlx_logging(false);

        let conn = SeaDatabase::connect(opts).await?;
        Ok(Self { conn })
    }

    pub async fn migrate(&self) -> Result<()> {
        let builder = self.conn.get_database_backend();
        let schema = Schema::new(builder);

        let stmt = builder.build(
            &schema
                .create_table_from_entity(task::Entity)
                .if_not_exists()
                .to_owned(),
        );
        self.conn.execute(stmt).await?;

        let stmt = builder.build(
            &schema
                .create_table_from_entity(project::Entity)
                .if_not_exists()
                .to_owned(),
        );
        self.conn.execute(stmt).await?;

        let stmt = builder.build(
            &schema
                .create_table_from_entity(stage_run::Entity)
                .if_not_exists()
                .to_owned(),
        );
        self.conn.execute(stmt).await?;

        let stmt = builder.build(
            &schema
                .create_table_from_entity(platform_config::Entity)
                .if_not_exists()
                .to_owned(),
        );
        self.conn.execute(stmt).await?;

        let stmt = builder.build(
            &schema
                .create_table_from_entity(template::Entity)
                .if_not_exists()
                .to_owned(),
        );
        self.conn.execute(stmt).await?;

        // Incremental migrations for existing DBs
        self.migrate_add_column("stage_runs", "agent_pid", "INTEGER").await;

        Ok(())
    }

    /// Best-effort ALTER TABLE ADD COLUMN (ignores "duplicate column" errors)
    async fn migrate_add_column(&self, table: &str, column: &str, col_type: &str) {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}");
        // Ignore error — column may already exist
        let _ = self.conn.execute_unprepared(&sql).await;
    }

    // ── Task CRUD ──

    pub async fn create_task(&self, req: &CreateTaskRequest) -> Result<task::Model> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let model = task::ActiveModel {
            id: Set(id),
            title: Set(req.title.clone()),
            prompt: Set(req.prompt.clone()),
            project_id: Set(req.project_id.clone()),
            task_type: Set(req.task_type.clone().unwrap_or_else(|| consts::DEFAULT_TASK_TYPE.into())),
            inputs: Set(req.inputs.clone()),
            status: Set(TaskStatus::Pending),
            created_at: Set(now),
            ..Default::default()
        };

        let result = model.insert(&self.conn).await?;
        Ok(result)
    }

    pub async fn get_task(&self, id: &str) -> Result<Option<task::Model>> {
        let result = task::Entity::find_by_id(id).one(&self.conn).await?;
        Ok(result)
    }

    pub async fn list_tasks(
        &self,
        status: Option<TaskStatus>,
        project_id: Option<&str>,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<task::Model>, u64)> {
        let mut query = task::Entity::find().order_by_desc(task::Column::CreatedAt);

        if let Some(s) = status {
            query = query.filter(task::Column::Status.eq(s));
        }
        if let Some(pid) = project_id {
            query = query.filter(task::Column::ProjectId.eq(pid));
        }

        let total = query.clone().count(&self.conn).await?;
        let models = query
            .offset(Some(offset))
            .limit(Some(limit))
            .all(&self.conn)
            .await?;

        Ok((models, total))
    }

    pub async fn update_task_status(
        &self,
        id: &str,
        status: TaskStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now();

        let mut model = task::ActiveModel {
            id: Set(id.to_string()),
            status: Set(status),
            ..Default::default()
        };

        if let Some(e) = error {
            model.error = Set(Some(e.to_string()));
        }

        match status {
            TaskStatus::Running => {
                model.started_at = Set(Some(now));
            }
            TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled => {
                model.finished_at = Set(Some(now));
            }
            TaskStatus::Pending => {}
        }

        model.update(&self.conn).await?;
        Ok(())
    }

    pub async fn update_task_current_stage(&self, id: &str, stage: &str) -> Result<()> {
        let model = task::ActiveModel {
            id: Set(id.to_string()),
            current_stage: Set(Some(stage.to_string())),
            ..Default::default()
        };
        model.update(&self.conn).await?;
        Ok(())
    }

    // ── Project CRUD ──

    pub async fn create_project(&self, req: &CreateProjectRequest) -> Result<project::Model> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let model = project::ActiveModel {
            id: Set(id),
            name: Set(req.name.clone()),
            repo_url: Set(req.repo_url.clone()),
            local_path: Set(req.local_path.clone()),
            default_agent: Set(req.default_agent.clone()),
            created_at: Set(now),
        };

        let result = model.insert(&self.conn).await?;
        Ok(result)
    }

    pub async fn get_project(&self, id: &str) -> Result<Option<project::Model>> {
        let result = project::Entity::find_by_id(id).one(&self.conn).await?;
        Ok(result)
    }

    pub async fn get_project_by_name(&self, name: &str) -> Result<Option<project::Model>> {
        let result = project::Entity::find()
            .filter(project::Column::Name.eq(name))
            .one(&self.conn)
            .await?;
        Ok(result)
    }

    pub async fn list_projects(&self) -> Result<Vec<project::Model>> {
        let result = project::Entity::find()
            .order_by_desc(project::Column::CreatedAt)
            .all(&self.conn)
            .await?;
        Ok(result)
    }

    pub async fn delete_project(&self, id: &str) -> Result<()> {
        project::Entity::delete_by_id(id)
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    // ── StageRun CRUD ──

    pub async fn create_stage_run(
        &self,
        task_id: &str,
        stage_name: &str,
        run_number: i32,
        agent_type: &str,
    ) -> Result<stage_run::Model> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let model = stage_run::ActiveModel {
            id: Set(id),
            task_id: Set(task_id.to_string()),
            stage_name: Set(stage_name.to_string()),
            run_number: Set(run_number),
            agent_type: Set(agent_type.to_string()),
            status: Set(StageRunStatus::Pending),
            created_at: Set(now),
            ..Default::default()
        };

        let result = model.insert(&self.conn).await?;
        Ok(result)
    }

    pub async fn get_stage_run(&self, id: &str) -> Result<Option<stage_run::Model>> {
        let result = stage_run::Entity::find_by_id(id).one(&self.conn).await?;
        Ok(result)
    }

    pub async fn list_stage_runs_by_task(&self, task_id: &str) -> Result<Vec<stage_run::Model>> {
        let result = stage_run::Entity::find()
            .filter(stage_run::Column::TaskId.eq(task_id))
            .order_by_asc(stage_run::Column::CreatedAt)
            .all(&self.conn)
            .await?;
        Ok(result)
    }

    pub async fn update_stage_run_status(
        &self,
        id: &str,
        status: StageRunStatus,
        workspace_path: Option<&str>,
        branch: Option<&str>,
    ) -> Result<()> {
        let mut model = stage_run::ActiveModel {
            id: Set(id.to_string()),
            status: Set(status),
            ..Default::default()
        };

        if let Some(wp) = workspace_path {
            model.workspace_path = Set(Some(wp.to_string()));
        }
        if let Some(b) = branch {
            model.branch = Set(Some(b.to_string()));
        }

        match status {
            StageRunStatus::Success | StageRunStatus::Failed => {
                model.finished_at = Set(Some(Utc::now()));
            }
            _ => {}
        }

        model.update(&self.conn).await?;
        Ok(())
    }

    pub async fn update_stage_run_report(
        &self,
        id: &str,
        report: &StageRunReport,
    ) -> Result<()> {
        let model = stage_run::ActiveModel {
            id: Set(id.to_string()),
            agent_exit_code: Set(report.exit_code),
            duration_seconds: Set(report.duration),
            agent_log: Set(report.agent_log.clone()),
            diff_patch: Set(report.diff_patch.clone()),
            summary: Set(report.summary.clone()),
            error_report: Set(report.error_report.clone()),
            prompt_used: Set(report.prompt_used.clone()),
            ..Default::default()
        };

        model.update(&self.conn).await?;
        Ok(())
    }

    /// 保存 agent 子进程 PID
    pub async fn update_stage_run_pid(&self, id: &str, pid: i32) -> Result<()> {
        let model = stage_run::ActiveModel {
            id: Set(id.to_string()),
            agent_pid: Set(Some(pid)),
            ..Default::default()
        };
        model.update(&self.conn).await?;
        Ok(())
    }

    /// 查找某个 task 下所有 running/pending 的 stage runs（用于取消）
    pub async fn get_active_stage_runs(&self, task_id: &str) -> Result<Vec<stage_run::Model>> {
        let result = stage_run::Entity::find()
            .filter(stage_run::Column::TaskId.eq(task_id))
            .filter(
                stage_run::Column::Status.is_in([
                    StageRunStatus::Running,
                    StageRunStatus::Pending,
                ]),
            )
            .all(&self.conn)
            .await?;
        Ok(result)
    }

    /// 查找所有 running 状态的 stage runs（用于启动恢复）
    pub async fn get_all_running_stage_runs(&self) -> Result<Vec<stage_run::Model>> {
        let result = stage_run::Entity::find()
            .filter(stage_run::Column::Status.eq(StageRunStatus::Running))
            .all(&self.conn)
            .await?;
        Ok(result)
    }

    // ── platform_config CRUD ──

    pub async fn set_config(&self, key: &str, value: &str, encrypted: bool) -> Result<()> {
        let now = Utc::now();
        let model = platform_config::ActiveModel {
            key: Set(key.to_string()),
            value: Set(value.to_string()),
            encrypted: Set(encrypted),
            updated_at: Set(now),
        };

        platform_config::Entity::insert(model)
            .on_conflict(
                OnConflict::column(platform_config::Column::Key)
                    .update_columns([
                        platform_config::Column::Value,
                        platform_config::Column::Encrypted,
                        platform_config::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;

        Ok(())
    }

    pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
        let result = platform_config::Entity::find_by_id(key)
            .one(&self.conn)
            .await?;
        Ok(result.map(|m| m.value))
    }

    pub async fn get_all_config(&self) -> Result<std::collections::HashMap<String, String>> {
        let all = platform_config::Entity::find().all(&self.conn).await?;
        Ok(all.into_iter().map(|m| (m.key, m.value)).collect())
    }

    pub async fn get_all_config_items(&self) -> Result<Vec<ConfigItem>> {
        let all = platform_config::Entity::find().all(&self.conn).await?;
        Ok(all
            .into_iter()
            .map(|m| ConfigItem {
                key: m.key,
                value: m.value,
                encrypted: m.encrypted,
            })
            .collect())
    }

    pub async fn delete_config(&self, key: &str) -> Result<()> {
        platform_config::Entity::delete_by_id(key)
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    // ── Template CRUD ──

    pub async fn create_template(
        &self,
        req: &CreateTemplateRequest,
        builtin: bool,
    ) -> Result<template::Model> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let model = template::ActiveModel {
            id: Set(id),
            name: Set(req.name.clone()),
            description: Set(req.description.clone()),
            definition: Set(req.definition.clone()),
            builtin: Set(builtin),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = model.insert(&self.conn).await?;
        Ok(result)
    }

    pub async fn get_template_by_name(&self, name: &str) -> Result<Option<template::Model>> {
        let result = template::Entity::find()
            .filter(template::Column::Name.eq(name))
            .one(&self.conn)
            .await?;
        Ok(result)
    }

    pub async fn list_templates(&self) -> Result<Vec<template::Model>> {
        let result = template::Entity::find()
            .order_by_asc(template::Column::Name)
            .all(&self.conn)
            .await?;
        Ok(result)
    }

    pub async fn update_template(
        &self,
        name: &str,
        req: &UpdateTemplateRequest,
    ) -> Result<Option<template::Model>> {
        let existing = self.get_template_by_name(name).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut model = template::ActiveModel {
            id: Set(existing.id),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        if let Some(desc) = &req.description {
            model.description = Set(desc.clone());
        }
        if let Some(def) = &req.definition {
            model.definition = Set(def.clone());
        }

        let result = model.update(&self.conn).await?;
        Ok(Some(result))
    }

    pub async fn delete_template(&self, name: &str) -> Result<bool> {
        let existing = self.get_template_by_name(name).await?;
        let Some(existing) = existing else {
            return Ok(false);
        };
        if existing.builtin {
            anyhow::bail!("Cannot delete builtin template: {name}");
        }
        template::Entity::delete_by_id(existing.id)
            .exec(&self.conn)
            .await?;
        Ok(true)
    }

    /// Seed 内置模板（仅在不存在时插入）
    pub async fn seed_builtin_templates(&self) -> Result<()> {
        let single_stage_yaml = include_str!("../../task-types/single-stage.yaml");
        let feature_dev_yaml = include_str!("../../task-types/feature-dev.yaml");

        for (name, desc, yaml) in [
            ("single-stage", "Single agent execution", single_stage_yaml),
            ("feature-dev", "Feature development: code → test → done", feature_dev_yaml),
        ] {
            if self.get_template_by_name(name).await?.is_none() {
                self.create_template(
                    &CreateTemplateRequest {
                        name: name.into(),
                        description: desc.into(),
                        definition: yaml.into(),
                    },
                    true,
                )
                .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db
    }

    fn make_task_req(title: &str) -> CreateTaskRequest {
        CreateTaskRequest {
            title: title.into(),
            prompt: "prompt".into(),
            project_id: None,
            task_type: None,
            inputs: None,
        }
    }

    fn make_project_req(name: &str) -> CreateProjectRequest {
        CreateProjectRequest {
            name: name.into(),
            repo_url: None,
            local_path: Some("/tmp/test".into()),
            default_agent: None,
        }
    }

    // ── Task tests ──

    #[tokio::test]
    async fn create_and_get_task() {
        let db = test_db().await;
        let req = CreateTaskRequest {
            title: "Test task".into(),
            prompt: "Do something".into(),
            project_id: None,
            task_type: Some("feature-dev".into()),
            inputs: Some(r#"{"requirement":"test"}"#.into()),
        };

        let task = db.create_task(&req).await.unwrap();
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.task_type, "feature-dev");
        assert!(task.inputs.is_some());

        let fetched = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[tokio::test]
    async fn create_task_uses_defaults() {
        let db = test_db().await;
        let task = db.create_task(&make_task_req("Defaults")).await.unwrap();
        assert_eq!(task.task_type, "single-stage");
    }

    #[tokio::test]
    async fn list_tasks_with_status_filter() {
        let db = test_db().await;

        for i in 0..5 {
            let task = db
                .create_task(&make_task_req(&format!("Task {i}")))
                .await
                .unwrap();
            if i < 2 {
                db.update_task_status(&task.id, TaskStatus::Running, None)
                    .await
                    .unwrap();
            }
        }

        let (all, total) = db.list_tasks(None, None, 20, 0).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(all.len(), 5);

        let (running, running_total) = db
            .list_tasks(Some(TaskStatus::Running), None, 20, 0)
            .await
            .unwrap();
        assert_eq!(running_total, 2);
        assert_eq!(running.len(), 2);
        assert!(running.iter().all(|t| t.status == TaskStatus::Running));
    }

    #[tokio::test]
    async fn get_nonexistent_task() {
        let db = test_db().await;
        let result = db
            .get_task(&uuid::Uuid::new_v4().to_string())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn pagination() {
        let db = test_db().await;
        for i in 0..10 {
            db.create_task(&make_task_req(&format!("Task {i}")))
                .await
                .unwrap();
        }

        let (page1, total) = db.list_tasks(None, None, 3, 0).await.unwrap();
        assert_eq!(total, 10);
        assert_eq!(page1.len(), 3);

        let (page2, _) = db.list_tasks(None, None, 3, 3).await.unwrap();
        assert_eq!(page2.len(), 3);
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[tokio::test]
    async fn status_transitions() {
        let db = test_db().await;
        let task = db
            .create_task(&make_task_req("Status test"))
            .await
            .unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());

        db.update_task_status(&task.id, TaskStatus::Running, None)
            .await
            .unwrap();
        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Running);
        assert!(t.started_at.is_some());

        db.update_task_status(&task.id, TaskStatus::Failed, Some("boom"))
            .await
            .unwrap();
        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Failed);
        assert!(t.finished_at.is_some());
        assert_eq!(t.error.as_deref(), Some("boom"));
    }

    // ── Project tests ──

    #[tokio::test]
    async fn create_and_get_project() {
        let db = test_db().await;
        let req = CreateProjectRequest {
            name: "my-app".into(),
            repo_url: Some("https://github.com/user/repo".into()),
            local_path: Some("/home/user/code/my-app".into()),
            default_agent: Some("claude-code".into()),
        };

        let proj = db.create_project(&req).await.unwrap();
        assert_eq!(proj.name, "my-app");
        assert_eq!(proj.repo_url.as_deref(), Some("https://github.com/user/repo"));

        let fetched = db.get_project(&proj.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, proj.id);
    }

    #[tokio::test]
    async fn get_project_by_name() {
        let db = test_db().await;
        db.create_project(&make_project_req("alpha")).await.unwrap();
        db.create_project(&make_project_req("beta")).await.unwrap();

        let found = db.get_project_by_name("alpha").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "alpha");

        let not_found = db.get_project_by_name("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn list_and_delete_projects() {
        let db = test_db().await;
        let p1 = db.create_project(&make_project_req("proj-1")).await.unwrap();
        db.create_project(&make_project_req("proj-2")).await.unwrap();

        let all = db.list_projects().await.unwrap();
        assert_eq!(all.len(), 2);

        db.delete_project(&p1.id).await.unwrap();
        let all = db.list_projects().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "proj-2");
    }

    // ── StageRun tests ──

    #[tokio::test]
    async fn create_and_get_stage_run() {
        let db = test_db().await;
        let task = db.create_task(&make_task_req("SR test")).await.unwrap();

        let sr = db
            .create_stage_run(&task.id, "coding", 1, "claude-code")
            .await
            .unwrap();
        assert_eq!(sr.task_id, task.id);
        assert_eq!(sr.stage_name, "coding");
        assert_eq!(sr.run_number, 1);
        assert_eq!(sr.status, StageRunStatus::Pending);

        let fetched = db.get_stage_run(&sr.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, sr.id);
    }

    #[tokio::test]
    async fn list_stage_runs_by_task() {
        let db = test_db().await;
        let t1 = db.create_task(&make_task_req("T1")).await.unwrap();
        let t2 = db.create_task(&make_task_req("T2")).await.unwrap();

        db.create_stage_run(&t1.id, "coding", 1, "claude-code").await.unwrap();
        db.create_stage_run(&t1.id, "testing", 1, "claude-code").await.unwrap();
        db.create_stage_run(&t2.id, "coding", 1, "codex").await.unwrap();

        let t1_runs = db.list_stage_runs_by_task(&t1.id).await.unwrap();
        assert_eq!(t1_runs.len(), 2);

        let t2_runs = db.list_stage_runs_by_task(&t2.id).await.unwrap();
        assert_eq!(t2_runs.len(), 1);
    }

    #[tokio::test]
    async fn update_stage_run_status_and_report() {
        let db = test_db().await;
        let task = db.create_task(&make_task_req("SR update")).await.unwrap();
        let sr = db
            .create_stage_run(&task.id, "coding", 1, "claude-code")
            .await
            .unwrap();

        // Update to running with workspace info
        db.update_stage_run_status(
            &sr.id,
            StageRunStatus::Running,
            Some("/tmp/workspace"),
            Some("ccodebox/t001"),
        )
        .await
        .unwrap();

        let updated = db.get_stage_run(&sr.id).await.unwrap().unwrap();
        assert_eq!(updated.status, StageRunStatus::Running);
        assert_eq!(updated.workspace_path.as_deref(), Some("/tmp/workspace"));
        assert_eq!(updated.branch.as_deref(), Some("ccodebox/t001"));

        // Update report
        db.update_stage_run_report(
            &sr.id,
            &StageRunReport {
                exit_code: Some(0),
                duration: Some(120),
                agent_log: Some("agent output log".into()),
                diff_patch: Some("diff content".into()),
                summary: Some("summary text".into()),
                error_report: None,
                prompt_used: Some("final prompt".into()),
            },
        )
        .await
        .unwrap();

        let reported = db.get_stage_run(&sr.id).await.unwrap().unwrap();
        assert_eq!(reported.agent_exit_code, Some(0));
        assert_eq!(reported.duration_seconds, Some(120));
        assert_eq!(reported.agent_log.as_deref(), Some("agent output log"));
        assert_eq!(reported.diff_patch.as_deref(), Some("diff content"));
        assert_eq!(reported.prompt_used.as_deref(), Some("final prompt"));

        // Update to success
        db.update_stage_run_status(&sr.id, StageRunStatus::Success, None, None)
            .await
            .unwrap();
        let finished = db.get_stage_run(&sr.id).await.unwrap().unwrap();
        assert_eq!(finished.status, StageRunStatus::Success);
        assert!(finished.finished_at.is_some());
    }

    // ── platform_config tests ──

    #[tokio::test]
    async fn set_and_get_config() {
        let db = test_db().await;

        let val = db.get_config("agent.claude-code.api_key").await.unwrap();
        assert!(val.is_none());

        db.set_config("agent.claude-code.api_key", "sk-test-123", true)
            .await
            .unwrap();
        let val = db.get_config("agent.claude-code.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("sk-test-123"));

        db.set_config("agent.claude-code.api_key", "sk-new-456", true)
            .await
            .unwrap();
        let val = db.get_config("agent.claude-code.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("sk-new-456"));
    }

    #[tokio::test]
    async fn get_all_config() {
        let db = test_db().await;

        db.set_config("agent.claude-code.api_key", "sk-cc", true).await.unwrap();
        db.set_config("agent.codex.api_key", "sk-codex", true).await.unwrap();
        db.set_config("tool.tavily.api_key", "tvly-123", true).await.unwrap();
        db.set_config("agent.claude-code.default_model", "sonnet", false).await.unwrap();

        let all = db.get_all_config().await.unwrap();
        assert_eq!(all.len(), 4);
        assert_eq!(all.get("agent.claude-code.api_key").unwrap(), "sk-cc");
    }

    #[tokio::test]
    async fn get_all_config_items() {
        let db = test_db().await;

        db.set_config("agent.claude-code.api_key", "sk-cc", true).await.unwrap();
        db.set_config("agent.claude-code.default_model", "sonnet", false).await.unwrap();

        let items = db.get_all_config_items().await.unwrap();
        assert_eq!(items.len(), 2);

        let api_key = items.iter().find(|i| i.key == "agent.claude-code.api_key").unwrap();
        assert!(api_key.encrypted);
    }

    #[tokio::test]
    async fn delete_config() {
        let db = test_db().await;

        db.set_config("temp.key", "value", false).await.unwrap();
        assert!(db.get_config("temp.key").await.unwrap().is_some());

        db.delete_config("temp.key").await.unwrap();
        assert!(db.get_config("temp.key").await.unwrap().is_none());

        db.delete_config("nonexistent").await.unwrap();
    }
}
