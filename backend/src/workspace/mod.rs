use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// 工作区管理器 — 管理 git worktree 和普通工作目录
pub struct WorkspaceManager {
    /// 工作区根目录，默认 ~/.ccodebox/workspaces
    base_dir: PathBuf,
}

/// 工作区创建结果
pub struct WorkspaceInfo {
    /// 工作目录路径
    pub path: PathBuf,
    /// git 分支名（如果创建了 worktree）
    pub branch: Option<String>,
}

impl WorkspaceManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// 默认路径: ~/.ccodebox/workspaces
    pub fn default_base_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ccodebox")
            .join("workspaces")
    }

    /// 为 stage run 创建或复用工作目录
    ///
    /// 一个 task 只有一个 worktree，所有 stage run 共享。
    /// 首次调用（needs_branch=true 的 stage）创建 git worktree。
    /// 后续调用（needs_branch=false 的 stage 或重试）复用已有目录。
    pub async fn create_workspace(
        &self,
        project_name: &str,
        task_id: &str,
        _stage_name: &str,
        _run_number: i32,
        repo_path: &Path,
        needs_branch: bool,
    ) -> Result<WorkspaceInfo> {
        // Task-level workspace: one directory per task, not per stage run
        let workspace_path = self.base_dir.join(project_name).join(task_id);
        let branch_name = format!("ccodebox/{task_id}");

        if workspace_path.exists() {
            // Worktree already exists from a previous stage or retry — reuse it
            // Check if it's a valid git worktree
            let is_git = workspace_path.join(".git").exists();
            return Ok(WorkspaceInfo {
                path: workspace_path,
                branch: if is_git { Some(branch_name) } else { None },
            });
        }

        if needs_branch {
            // First stage: create git worktree with new branch
            tokio::fs::create_dir_all(workspace_path.parent().unwrap())
                .await
                .context("创建项目工作目录失败")?;

            let output = tokio::process::Command::new("git")
                .args([
                    "worktree",
                    "add",
                    workspace_path.to_str().unwrap(),
                    "-b",
                    &branch_name,
                ])
                .current_dir(repo_path)
                .output()
                .await
                .context("执行 git worktree add 失败")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("git worktree add 失败: {stderr}");
            }

            Ok(WorkspaceInfo {
                path: workspace_path,
                branch: Some(branch_name),
            })
        } else {
            // needs_branch=false but no existing worktree — shouldn't happen
            // in normal flow, but create a plain directory as fallback
            tokio::fs::create_dir_all(&workspace_path)
                .await
                .context("创建工作目录失败")?;

            Ok(WorkspaceInfo {
                path: workspace_path,
                branch: None,
            })
        }
    }

    /// 收集工作目录中的 git diff（排除构建产物和日志）
    pub async fn collect_diff(workspace_path: &Path) -> Result<Option<String>> {
        // Ensure .gitignore exists in worktree to exclude build artifacts
        let gitignore_path = workspace_path.join(".gitignore");
        if !gitignore_path.exists() {
            let _ = tokio::fs::write(
                &gitignore_path,
                "# CCodeBoX auto-generated\ntarget/\nnode_modules/\n.next/\ndist/\nbuild/\n*.log\n.ccodebox-agent.log\n.ccodebox-prompt.md\n",
            )
            .await;
        } else {
            // Append our exclusions if not already present
            let content = tokio::fs::read_to_string(&gitignore_path).await.unwrap_or_default();
            if !content.contains(".ccodebox-agent.log") {
                let _ = tokio::fs::write(
                    &gitignore_path,
                    format!("{content}\n# CCodeBoX\n.ccodebox-agent.log\n.ccodebox-prompt.md\n"),
                )
                .await;
            }
        }

        let output = tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(workspace_path)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(None);
        }

        let output = tokio::process::Command::new("git")
            .args([
                "diff", "--cached",
                // Exclude known build artifact dirs as safety net
                "--", ".", ":(exclude)target", ":(exclude)node_modules",
                ":(exclude).next", ":(exclude)dist", ":(exclude)build",
            ])
            .current_dir(workspace_path)
            .output()
            .await?;

        if output.status.success() {
            let diff = String::from_utf8_lossy(&output.stdout).to_string();
            if diff.is_empty() {
                Ok(None)
            } else {
                Ok(Some(diff))
            }
        } else {
            Ok(None)
        }
    }

    /// 清理工作目录（移除 worktree）
    pub async fn cleanup_workspace(
        &self,
        repo_path: &Path,
        workspace_path: &Path,
    ) -> Result<()> {
        // 尝试移除 git worktree
        let _ = tokio::process::Command::new("git")
            .args([
                "worktree",
                "remove",
                workspace_path.to_str().unwrap(),
                "--force",
            ])
            .current_dir(repo_path)
            .output()
            .await;

        // 如果 worktree remove 不适用（普通目录），直接删除
        if workspace_path.exists() {
            tokio::fs::remove_dir_all(workspace_path).await.ok();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_git_repo(dir: &Path) {
        tokio::process::Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .await
            .unwrap();

        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .await
            .unwrap();

        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .await
            .unwrap();

        // 创建初始 commit（worktree 需要至少一个 commit）
        tokio::fs::write(dir.join("README.md"), "# Test").await.unwrap();

        tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir)
            .output()
            .await
            .unwrap();

        tokio::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir)
            .output()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_workspace_without_branch() {
        let base = TempDir::new().unwrap();
        let mgr = WorkspaceManager::new(base.path().to_path_buf());

        let info = mgr
            .create_workspace("test-project", "t001", "coding", 1, Path::new("/tmp"), false)
            .await
            .unwrap();

        assert!(info.path.exists());
        assert!(info.branch.is_none());
        // Task-level workspace: path ends with task_id, not stage--run
        assert!(info.path.to_str().unwrap().contains("t001"));
    }

    #[tokio::test]
    async fn create_workspace_with_worktree() {
        let base = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        setup_git_repo(repo.path()).await;

        let mgr = WorkspaceManager::new(base.path().to_path_buf());

        let info = mgr
            .create_workspace("test-project", "t001", "coding", 1, repo.path(), true)
            .await
            .unwrap();

        assert!(info.path.exists());
        assert_eq!(info.branch.as_deref(), Some("ccodebox/t001"));

        // 验证 worktree 中有文件
        assert!(info.path.join("README.md").exists());
    }

    #[tokio::test]
    async fn reuse_workspace_across_stages() {
        let base = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        setup_git_repo(repo.path()).await;

        let mgr = WorkspaceManager::new(base.path().to_path_buf());

        // coding stage 1: creates worktree
        let ws1 = mgr
            .create_workspace("proj", "task-1", "coding", 1, repo.path(), true)
            .await
            .unwrap();
        assert!(ws1.path.join("README.md").exists());

        // testing stage 1: reuses same worktree
        let ws2 = mgr
            .create_workspace("proj", "task-1", "testing", 1, repo.path(), false)
            .await
            .unwrap();
        assert_eq!(ws1.path, ws2.path);
        assert!(ws2.path.join("README.md").exists());

        // coding stage 2 (retry): still reuses same worktree
        let ws3 = mgr
            .create_workspace("proj", "task-1", "coding", 2, repo.path(), true)
            .await
            .unwrap();
        assert_eq!(ws1.path, ws3.path);
        assert!(ws3.branch.is_some());
    }

    #[tokio::test]
    async fn collect_diff_with_changes() {
        let repo = TempDir::new().unwrap();
        setup_git_repo(repo.path()).await;

        // 在 repo 中做一些修改
        tokio::fs::write(repo.path().join("new_file.txt"), "hello").await.unwrap();

        let diff = WorkspaceManager::collect_diff(repo.path()).await.unwrap();
        assert!(diff.is_some());
        assert!(diff.unwrap().contains("new_file.txt"));
    }

    #[tokio::test]
    async fn collect_diff_no_changes() {
        let repo = TempDir::new().unwrap();
        setup_git_repo(repo.path()).await;

        // Pre-create the .gitignore that collect_diff would auto-create, and commit it
        std::fs::write(
            repo.path().join(".gitignore"),
            "# CCodeBoX auto-generated\ntarget/\nnode_modules/\n.next/\ndist/\nbuild/\n*.log\n.ccodebox-agent.log\n.ccodebox-prompt.md\n",
        )
        .unwrap();
        tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo.path())
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["commit", "-m", "add gitignore"])
            .current_dir(repo.path())
            .output()
            .await
            .unwrap();

        let diff = WorkspaceManager::collect_diff(repo.path()).await.unwrap();
        assert!(diff.is_none());
    }

    #[tokio::test]
    async fn cleanup_workspace_worktree() {
        let base = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        setup_git_repo(repo.path()).await;

        let mgr = WorkspaceManager::new(base.path().to_path_buf());

        let info = mgr
            .create_workspace("test-project", "t002", "coding", 1, repo.path(), true)
            .await
            .unwrap();

        assert!(info.path.exists());

        mgr.cleanup_workspace(repo.path(), &info.path)
            .await
            .unwrap();

        assert!(!info.path.exists());
    }
}
