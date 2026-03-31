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

    /// 为 stage run 创建工作目录
    ///
    /// needs_branch=true → git worktree add（基于 repo_path）
    /// needs_branch=false → 创建普通目录
    pub async fn create_workspace(
        &self,
        project_name: &str,
        task_id: &str,
        stage_name: &str,
        run_number: i32,
        repo_path: &Path,
        needs_branch: bool,
    ) -> Result<WorkspaceInfo> {
        let dir_name = format!("{task_id}--{stage_name}--{run_number}");
        let workspace_path = self.base_dir.join(project_name).join(&dir_name);

        tokio::fs::create_dir_all(&workspace_path)
            .await
            .context("创建工作目录失败")?;

        if needs_branch {
            let branch_name = format!("ccodebox/{task_id}");

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
            Ok(WorkspaceInfo {
                path: workspace_path,
                branch: None,
            })
        }
    }

    /// 收集工作目录中的 git diff
    pub async fn collect_diff(workspace_path: &Path) -> Result<Option<String>> {
        let output = tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(workspace_path)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(None);
        }

        let output = tokio::process::Command::new("git")
            .args(["diff", "--cached"])
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
        assert!(info
            .path
            .to_str()
            .unwrap()
            .contains("t001--coding--1"));
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
