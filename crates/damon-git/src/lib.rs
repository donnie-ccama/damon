//! Git operations for damon agents. Shells out to the git CLI.
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(thiserror::Error, Debug)]
pub enum GitError {
    #[error("failed to run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {args} failed: {stderr}")]
    Failed { args: String, stderr: String },
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn git(cwd: Option<&Path>, args: &[&str]) -> Result<String, GitError> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(GitError::Failed {
            args: args.join(" "),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn init_new(worktree: &Path, branch: &str) -> Result<(), GitError> {
    std::fs::create_dir_all(worktree).map_err(|source| GitError::Io {
        path: worktree.to_path_buf(),
        source,
    })?;
    git(Some(worktree), &["init", "-b", branch])?;
    Ok(())
}

pub fn clone_repo(url: &str, worktree: &Path, branch: &str) -> Result<(), GitError> {
    git(None, &["clone", url, &worktree.to_string_lossy()])?;
    git(Some(worktree), &["checkout", "-B", branch])?;
    Ok(())
}

pub fn worktree_add(existing_repo: &Path, worktree: &Path, branch: &str) -> Result<(), GitError> {
    let wt = worktree.to_string_lossy();
    let branch_exists = git(
        Some(existing_repo),
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok();
    if branch_exists {
        git(Some(existing_repo), &["worktree", "add", &wt, branch])?;
    } else {
        git(Some(existing_repo), &["worktree", "add", "-b", branch, &wt])?;
    }
    Ok(())
}

pub fn worktree_remove(existing_repo: &Path, worktree: &Path) -> Result<(), GitError> {
    git(
        Some(existing_repo),
        &["worktree", "remove", "--force", &worktree.to_string_lossy()],
    )?;
    Ok(())
}

/// Append entries to <git-common-dir>/info/exclude (idempotent). Keeps
/// generated bridge files out of `git status` without touching tracked files.
pub fn exclude(worktree: &Path, entries: &[&str]) -> Result<(), GitError> {
    let common = git(
        Some(worktree),
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let path = Path::new(&common).join("info").join("exclude");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| GitError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut add = String::new();
    for entry in entries {
        if !existing.lines().any(|l| l.trim() == *entry) {
            add.push_str(entry);
            add.push('\n');
        }
    }
    if !add.is_empty() {
        let mut text = existing;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&add);
        std::fs::write(&path, text).map_err(|source| GitError::Io {
            path: path.clone(),
            source,
        })?;
    }
    Ok(())
}
