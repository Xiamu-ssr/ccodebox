use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database as SeaDatabase,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Schema,
    Set,
};
use sea_orm::sea_query::OnConflict;

use crate::contracts::{AgentType, ConfigItem, CreateTaskRequest, TaskStatus};
use crate::entity::{platform_config, task, task_log};

pub struct TaskReportUpdate {
    pub agent_exit_code: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub pushed: bool,
    pub lines_added: i32,
    pub lines_removed: i32,
    pub files_changed: Option<String>,
    pub summary: Option<String>,
    pub diff_patch: Option<String>,
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
                .create_table_from_entity(task_log::Entity)
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

        Ok(())
    }

    pub async fn create_task(
        &self,
        req: &CreateTaskRequest,
        default_model: &str,
    ) -> Result<task::Model> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let model = task::ActiveModel {
            id: Set(id),
            title: Set(req.title.clone()),
            prompt: Set(req.prompt.clone()),
            repo_url: Set(req.repo_url.clone()),
            branch: Set(req.branch.clone()),
            agent_type: Set(req.agent_type.unwrap_or(AgentType::ClaudeCode)),
            model: Set(req.model.clone().unwrap_or_else(|| default_model.to_string())),
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
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<task::Model>, u64)> {
        let mut query = task::Entity::find().order_by_desc(task::Column::CreatedAt);

        if let Some(s) = status {
            query = query.filter(task::Column::Status.eq(s));
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
        container_id: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now();

        let mut model = task::ActiveModel {
            id: Set(id.to_string()),
            status: Set(status),
            ..Default::default()
        };

        if let Some(cid) = container_id {
            model.container_id = Set(Some(cid.to_string()));
        }

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

    pub async fn update_task_report(&self, id: &str, report: &TaskReportUpdate) -> Result<()> {
        let model = task::ActiveModel {
            id: Set(id.to_string()),
            agent_exit_code: Set(report.agent_exit_code),
            duration_seconds: Set(report.duration_seconds),
            pushed: Set(report.pushed),
            lines_added: Set(report.lines_added),
            lines_removed: Set(report.lines_removed),
            files_changed: Set(report.files_changed.clone()),
            summary: Set(report.summary.clone()),
            diff_patch: Set(report.diff_patch.clone()),
            ..Default::default()
        };

        model.update(&self.conn).await?;
        Ok(())
    }

    pub async fn update_task_logs(&self, task_id: &str, logs: &str) -> Result<()> {
        let model = task_log::ActiveModel {
            task_id: Set(task_id.to_string()),
            logs: Set(logs.to_string()),
            rounds: Set(1),
        };

        task_log::Entity::insert(model)
            .on_conflict(
                OnConflict::column(task_log::Column::TaskId)
                    .update_columns([task_log::Column::Logs, task_log::Column::Rounds])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;

        Ok(())
    }

    pub async fn get_task_logs(&self, task_id: &str) -> Result<Option<String>> {
        let result = task_log::Entity::find_by_id(task_id)
            .one(&self.conn)
            .await?;
        Ok(result.map(|m| m.logs))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db
    }

    fn make_req(title: &str) -> CreateTaskRequest {
        CreateTaskRequest {
            title: title.into(),
            prompt: "prompt".into(),
            repo_url: None,
            branch: None,
            agent_type: None,
            model: None,
        }
    }

    #[tokio::test]
    async fn create_and_get_task() {
        let db = test_db().await;
        let req = CreateTaskRequest {
            title: "Test task".into(),
            prompt: "Do something".into(),
            repo_url: Some("https://github.com/user/repo".into()),
            branch: Some("main".into()),
            agent_type: Some(AgentType::ClaudeCode),
            model: Some("claude-opus-4-6".into()),
        };

        let task = db.create_task(&req, "default").await.unwrap();
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.agent_type, AgentType::ClaudeCode);
        assert_eq!(task.model, "claude-opus-4-6");
        assert_eq!(
            task.repo_url.as_deref(),
            Some("https://github.com/user/repo")
        );

        let fetched = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, task.id);
        assert_eq!(fetched.title, "Test task");
        assert_eq!(fetched.model, "claude-opus-4-6");
    }

    #[tokio::test]
    async fn create_task_uses_defaults() {
        let db = test_db().await;
        let req = make_req("Defaults");
        let task = db.create_task(&req, "my-default-model").await.unwrap();
        assert_eq!(task.agent_type, AgentType::ClaudeCode);
        assert_eq!(task.model, "my-default-model");
    }

    #[tokio::test]
    async fn list_tasks_with_status_filter() {
        let db = test_db().await;

        for i in 0..5 {
            let task = db
                .create_task(&make_req(&format!("Task {i}")), "m")
                .await
                .unwrap();
            if i < 2 {
                db.update_task_status(&task.id, TaskStatus::Running, Some("cid"), None)
                    .await
                    .unwrap();
            }
        }

        let (all, total) = db.list_tasks(None, 20, 0).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(all.len(), 5);

        let (running, running_total) = db
            .list_tasks(Some(TaskStatus::Running), 20, 0)
            .await
            .unwrap();
        assert_eq!(running_total, 2);
        assert_eq!(running.len(), 2);
        assert!(running.iter().all(|t| t.status == TaskStatus::Running));
    }

    #[tokio::test]
    async fn update_task_report() {
        let db = test_db().await;
        let task = db.create_task(&make_req("Report test"), "m").await.unwrap();
        db.update_task_report(
            &task.id,
            &TaskReportUpdate {
                agent_exit_code: Some(0),
                duration_seconds: Some(120),
                pushed: true,
                lines_added: 42,
                lines_removed: 10,
                files_changed: Some("a.py,b.py".into()),
                summary: Some("summary text".into()),
                diff_patch: Some("diff content".into()),
            },
        )
        .await
        .unwrap();

        let updated = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(updated.agent_exit_code, Some(0));
        assert_eq!(updated.duration_seconds, Some(120));
        assert!(updated.pushed);
        assert_eq!(updated.lines_added, 42);
        assert_eq!(updated.lines_removed, 10);
        assert_eq!(updated.files_changed.as_deref(), Some("a.py,b.py"));
        assert_eq!(updated.summary.as_deref(), Some("summary text"));
        assert_eq!(updated.diff_patch.as_deref(), Some("diff content"));
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
            db.create_task(&make_req(&format!("Task {i}")), "m")
                .await
                .unwrap();
        }

        let (page1, total) = db.list_tasks(None, 3, 0).await.unwrap();
        assert_eq!(total, 10);
        assert_eq!(page1.len(), 3);

        let (page2, _) = db.list_tasks(None, 3, 3).await.unwrap();
        assert_eq!(page2.len(), 3);
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[tokio::test]
    async fn update_and_get_logs() {
        let db = test_db().await;
        let task = db.create_task(&make_req("Logs test"), "m").await.unwrap();

        let none = db.get_task_logs(&task.id).await.unwrap();
        assert!(none.is_none());

        db.update_task_logs(&task.id, "Agent output log content")
            .await
            .unwrap();

        let logs = db.get_task_logs(&task.id).await.unwrap().unwrap();
        assert!(logs.contains("Agent output"));

        db.update_task_logs(&task.id, "Updated logs")
            .await
            .unwrap();
        let logs = db.get_task_logs(&task.id).await.unwrap().unwrap();
        assert_eq!(logs, "Updated logs");
    }

    // ── platform_config tests ──

    #[tokio::test]
    async fn set_and_get_config() {
        let db = test_db().await;

        // Initially empty
        let val = db.get_config("agent.claude-code.api_key").await.unwrap();
        assert!(val.is_none());

        // Set a value
        db.set_config("agent.claude-code.api_key", "sk-test-123", true)
            .await
            .unwrap();
        let val = db.get_config("agent.claude-code.api_key").await.unwrap();
        assert_eq!(val.as_deref(), Some("sk-test-123"));

        // Upsert overwrites
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
        assert_eq!(all.get("tool.tavily.api_key").unwrap(), "tvly-123");
    }

    #[tokio::test]
    async fn get_all_config_items() {
        let db = test_db().await;

        db.set_config("agent.claude-code.api_key", "sk-cc", true).await.unwrap();
        db.set_config("git.github_token", "ghp-abc", true).await.unwrap();
        db.set_config("agent.claude-code.default_model", "sonnet", false).await.unwrap();

        let items = db.get_all_config_items().await.unwrap();
        assert_eq!(items.len(), 3);

        // Encrypted items should have encrypted=true
        let api_key = items.iter().find(|i| i.key == "agent.claude-code.api_key").unwrap();
        assert!(api_key.encrypted);
        assert_eq!(api_key.value, "sk-cc"); // raw value in DB layer

        // Non-encrypted items
        let model = items.iter().find(|i| i.key == "agent.claude-code.default_model").unwrap();
        assert!(!model.encrypted);
    }

    #[tokio::test]
    async fn delete_config() {
        let db = test_db().await;

        db.set_config("temp.key", "value", false).await.unwrap();
        assert!(db.get_config("temp.key").await.unwrap().is_some());

        db.delete_config("temp.key").await.unwrap();
        assert!(db.get_config("temp.key").await.unwrap().is_none());

        // Deleting non-existent key should not error
        db.delete_config("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn status_transitions() {
        let db = test_db().await;
        let task = db
            .create_task(&make_req("Status test"), "m")
            .await
            .unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());

        db.update_task_status(&task.id, TaskStatus::Running, Some("container-123"), None)
            .await
            .unwrap();
        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Running);
        assert!(t.started_at.is_some());
        assert_eq!(t.container_id.as_deref(), Some("container-123"));

        db.update_task_status(&task.id, TaskStatus::Failed, None, Some("boom"))
            .await
            .unwrap();
        let t = db.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Failed);
        assert!(t.finished_at.is_some());
        assert_eq!(t.error.as_deref(), Some("boom"));
    }
}
