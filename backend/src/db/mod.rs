use anyhow::Result;
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::models::task::{
    AgentType, CreateTaskRequest, Task, TaskStatus, VerifyStatus,
};

pub struct TaskReportUpdate<'a> {
    pub rounds_used: i32,
    pub lint_status: Option<&'a str>,
    pub test_status: Option<&'a str>,
    pub lines_added: i32,
    pub lines_removed: i32,
    pub files_changed: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub diff_patch: Option<&'a str>,
}

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(url: &str) -> Result<Self> {
        let url = url.strip_prefix("sqlite:").unwrap_or(url);

        // In-memory DB for tests
        if url == ":memory:" {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await?;
            return Ok(Self { pool });
        }

        if let Some(parent) = std::path::Path::new(url).parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        let options = SqliteConnectOptions::new()
            .filename(url)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id              TEXT PRIMARY KEY,
                title           TEXT NOT NULL,
                prompt          TEXT NOT NULL,
                repo_url        TEXT,
                branch          TEXT,
                agent_type      TEXT NOT NULL DEFAULT 'claude-code',
                model           TEXT NOT NULL DEFAULT 'claude-sonnet-4-20250514',
                max_rounds      INTEGER NOT NULL DEFAULT 3,
                status          TEXT NOT NULL DEFAULT 'pending',
                container_id    TEXT,
                rounds_used     INTEGER DEFAULT 0,
                lint_status     TEXT,
                test_status     TEXT,
                lines_added     INTEGER DEFAULT 0,
                lines_removed   INTEGER DEFAULT 0,
                files_changed   TEXT,
                summary         TEXT,
                diff_patch      TEXT,
                error           TEXT,
                created_at      TEXT NOT NULL,
                started_at      TEXT,
                finished_at     TEXT
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_logs (
                task_id     TEXT PRIMARY KEY REFERENCES tasks(id),
                logs        TEXT NOT NULL,
                rounds      INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_task(&self, req: &CreateTaskRequest, default_model: &str) -> Result<Task> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let agent_type = req.agent_type.unwrap_or(AgentType::ClaudeCode);
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string());
        let max_rounds = req.max_rounds.unwrap_or(3);

        sqlx::query(
            "INSERT INTO tasks (id, title, prompt, repo_url, branch, agent_type, model, max_rounds, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)",
        )
        .bind(id.to_string())
        .bind(&req.title)
        .bind(&req.prompt)
        .bind(&req.repo_url)
        .bind(&req.branch)
        .bind(agent_type.as_str())
        .bind(&model)
        .bind(max_rounds)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(Task {
            id,
            title: req.title.clone(),
            prompt: req.prompt.clone(),
            repo_url: req.repo_url.clone(),
            branch: req.branch.clone(),
            agent_type,
            model,
            max_rounds,
            status: TaskStatus::Pending,
            container_id: None,
            rounds_used: 0,
            lint_status: None,
            test_status: None,
            lines_added: 0,
            lines_removed: 0,
            files_changed: None,
            summary: None,
            diff_patch: None,
            error: None,
            created_at: now,
            started_at: None,
            finished_at: None,
        })
    }

    pub async fn get_task(&self, id: Uuid) -> Result<Option<Task>> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_task(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn list_tasks(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Task>, i32)> {
        // Validate status against known values to prevent injection
        if let Some(s) = status {
            TaskStatus::from_db(s).ok_or_else(|| anyhow::anyhow!("invalid status filter: {s}"))?;
        }

        let (total, rows) = if let Some(s) = status {
            let total: i32 =
                sqlx::query("SELECT COUNT(*) as cnt FROM tasks WHERE status = ?")
                    .bind(s)
                    .fetch_one(&self.pool)
                    .await?
                    .get("cnt");

            let rows = sqlx::query(
                "SELECT * FROM tasks WHERE status = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(s)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

            (total, rows)
        } else {
            let total: i32 = sqlx::query("SELECT COUNT(*) as cnt FROM tasks")
                .fetch_one(&self.pool)
                .await?
                .get("cnt");

            let rows = sqlx::query(
                "SELECT * FROM tasks ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

            (total, rows)
        };

        let tasks: Vec<Task> = rows.iter().filter_map(|r| row_to_task(r).ok()).collect();

        Ok((tasks, total))
    }

    pub async fn update_task_status(
        &self,
        id: Uuid,
        status: TaskStatus,
        container_id: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut query = String::from("UPDATE tasks SET status = ?");
        let mut binds: Vec<String> = vec![status.as_str().to_string()];

        if let Some(cid) = container_id {
            query.push_str(", container_id = ?");
            binds.push(cid.to_string());
        }

        if let Some(e) = error {
            query.push_str(", error = ?");
            binds.push(e.to_string());
        }

        match status {
            TaskStatus::Running => {
                query.push_str(", started_at = ?");
                binds.push(now);
            }
            TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled => {
                query.push_str(", finished_at = ?");
                binds.push(now);
            }
            TaskStatus::Pending => {}
        }

        query.push_str(" WHERE id = ?");
        binds.push(id.to_string());

        let mut q = sqlx::query(&query);
        for b in &binds {
            q = q.bind(b);
        }
        q.execute(&self.pool).await?;

        Ok(())
    }

    pub async fn update_task_report(&self, id: Uuid, report: &TaskReportUpdate<'_>) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET
                rounds_used = ?, lint_status = ?, test_status = ?,
                lines_added = ?, lines_removed = ?, files_changed = ?,
                summary = ?, diff_patch = ?
             WHERE id = ?",
        )
        .bind(report.rounds_used)
        .bind(report.lint_status)
        .bind(report.test_status)
        .bind(report.lines_added)
        .bind(report.lines_removed)
        .bind(report.files_changed)
        .bind(report.summary)
        .bind(report.diff_patch)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_task_logs(&self, id: Uuid, logs: &str, rounds: i32) -> Result<()> {
        sqlx::query(
            "INSERT INTO task_logs (task_id, logs, rounds) VALUES (?, ?, ?)
             ON CONFLICT(task_id) DO UPDATE SET logs = excluded.logs, rounds = excluded.rounds",
        )
        .bind(id.to_string())
        .bind(logs)
        .bind(rounds)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_task_logs(&self, id: Uuid) -> Result<Option<(String, i32)>> {
        let row = sqlx::query("SELECT logs, rounds FROM task_logs WHERE task_id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| {
            let logs: String = r.get("logs");
            let rounds: i32 = r.get("rounds");
            (logs, rounds)
        }))
    }
}

fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Result<Task> {
    let id_str: String = row.get("id");
    let agent_type_str: String = row.get("agent_type");
    let status_str: String = row.get("status");
    let created_at_str: String = row.get("created_at");

    let agent_type = match agent_type_str.as_str() {
        "claude-code" => AgentType::ClaudeCode,
        "codex" => AgentType::Codex,
        other => anyhow::bail!("unknown agent_type: {other}"),
    };

    let status = TaskStatus::from_db(&status_str)
        .ok_or_else(|| anyhow::anyhow!("unknown status: {status_str}"))?;

    let parse_dt = |col: &str| -> Option<chrono::DateTime<Utc>> {
        let s: Option<String> = row.get(col);
        s.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    };

    let lint_status: Option<String> = row.get("lint_status");
    let test_status: Option<String> = row.get("test_status");

    Ok(Task {
        id: Uuid::parse_str(&id_str)?,
        title: row.get("title"),
        prompt: row.get("prompt"),
        repo_url: row.get("repo_url"),
        branch: row.get("branch"),
        agent_type,
        model: row.get("model"),
        max_rounds: row.get("max_rounds"),
        status,
        container_id: row.get("container_id"),
        rounds_used: row.get("rounds_used"),
        lint_status: lint_status.as_deref().and_then(VerifyStatus::from_db),
        test_status: test_status.as_deref().and_then(VerifyStatus::from_db),
        lines_added: row.get("lines_added"),
        lines_removed: row.get("lines_removed"),
        files_changed: row.get("files_changed"),
        summary: row.get("summary"),
        diff_patch: row.get("diff_patch"),
        error: row.get("error"),
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)?
            .with_timezone(&Utc),
        started_at: parse_dt("started_at"),
        finished_at: parse_dt("finished_at"),
    })
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
            max_rounds: None,
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
            max_rounds: Some(5),
        };

        let task = db.create_task(&req, "default").await.unwrap();
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.agent_type, AgentType::ClaudeCode);
        assert_eq!(task.model, "claude-opus-4-6");
        assert_eq!(task.max_rounds, 5);
        assert_eq!(task.repo_url.as_deref(), Some("https://github.com/user/repo"));

        let fetched = db.get_task(task.id).await.unwrap().unwrap();
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
        assert_eq!(task.max_rounds, 3);
    }

    #[tokio::test]
    async fn list_tasks_with_status_filter() {
        let db = test_db().await;

        for i in 0..5 {
            let task = db.create_task(&make_req(&format!("Task {i}")), "m").await.unwrap();
            if i < 2 {
                db.update_task_status(task.id, TaskStatus::Running, Some("cid"), None)
                    .await
                    .unwrap();
            }
        }

        let (all, total) = db.list_tasks(None, 20, 0).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(all.len(), 5);

        let (running, running_total) = db.list_tasks(Some("running"), 20, 0).await.unwrap();
        assert_eq!(running_total, 2);
        assert_eq!(running.len(), 2);
        assert!(running.iter().all(|t| t.status == TaskStatus::Running));
    }

    #[tokio::test]
    async fn update_task_report() {
        let db = test_db().await;
        let task = db.create_task(&make_req("Report test"), "m").await.unwrap();
        db.update_task_report(
            task.id,
            &TaskReportUpdate {
                rounds_used: 2,
                lint_status: Some("pass"),
                test_status: Some("fail"),
                lines_added: 42,
                lines_removed: 10,
                files_changed: Some("a.py,b.py"),
                summary: Some("summary text"),
                diff_patch: Some("diff content"),
            },
        )
        .await
        .unwrap();

        let updated = db.get_task(task.id).await.unwrap().unwrap();
        assert_eq!(updated.rounds_used, 2);
        assert_eq!(updated.lint_status, Some(VerifyStatus::Pass));
        assert_eq!(updated.test_status, Some(VerifyStatus::Fail));
        assert_eq!(updated.lines_added, 42);
        assert_eq!(updated.lines_removed, 10);
        assert_eq!(updated.files_changed.as_deref(), Some("a.py,b.py"));
        assert_eq!(updated.summary.as_deref(), Some("summary text"));
        assert_eq!(updated.diff_patch.as_deref(), Some("diff content"));
    }

    #[tokio::test]
    async fn get_nonexistent_task() {
        let db = test_db().await;
        let result = db.get_task(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn pagination() {
        let db = test_db().await;
        for i in 0..10 {
            db.create_task(&make_req(&format!("Task {i}")), "m").await.unwrap();
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

        // No logs yet
        let none = db.get_task_logs(task.id).await.unwrap();
        assert!(none.is_none());

        db.update_task_logs(task.id, "Round 1 output\n---\nRound 2 output", 2)
            .await
            .unwrap();

        let (logs, rounds) = db.get_task_logs(task.id).await.unwrap().unwrap();
        assert_eq!(rounds, 2);
        assert!(logs.contains("Round 1 output"));

        // Upsert overwrites
        db.update_task_logs(task.id, "Updated logs", 3).await.unwrap();
        let (logs, rounds) = db.get_task_logs(task.id).await.unwrap().unwrap();
        assert_eq!(rounds, 3);
        assert_eq!(logs, "Updated logs");
    }

    #[tokio::test]
    async fn status_transitions() {
        let db = test_db().await;
        let task = db.create_task(&make_req("Status test"), "m").await.unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());

        db.update_task_status(task.id, TaskStatus::Running, Some("container-123"), None)
            .await
            .unwrap();
        let t = db.get_task(task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Running);
        assert!(t.started_at.is_some());
        assert_eq!(t.container_id.as_deref(), Some("container-123"));

        db.update_task_status(task.id, TaskStatus::Failed, None, Some("boom"))
            .await
            .unwrap();
        let t = db.get_task(task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Failed);
        assert!(t.finished_at.is_some());
        assert_eq!(t.error.as_deref(), Some("boom"));
    }
}
