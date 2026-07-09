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

const BLOCK_BEGIN: &str = "# damon begin";
const BLOCK_END: &str = "# damon end";
/// Every bridge filename damon has ever excluded — used to absorb legacy
/// unmarked lines from pre-block installs into the block.
const KNOWN_PATTERNS: [&str; 3] = ["CLAUDE.md", "AGENTS.md", ".claude/settings.json"];

/// Absolute git common dir for the repo containing `path`.
pub fn common_dir(path: &Path) -> Result<PathBuf, GitError> {
    let out = git(
        Some(path),
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Ok(PathBuf::from(out))
}

fn exclude_path(repo: &Path) -> Result<PathBuf, GitError> {
    Ok(common_dir(repo)?.join("info").join("exclude"))
}

/// Ensure `entries` are ignored via a sentinel-delimited block in
/// <git-common-dir>/info/exclude. Idempotent; lines outside the markers are
/// never touched, except legacy damon patterns, which migrate into the block.
/// A missing exclude file starts empty; any other read error propagates
/// rather than risking a clobber.
pub fn exclude(worktree: &Path, entries: &[&str]) -> Result<(), GitError> {
    let path = exclude_path(worktree)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| GitError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(GitError::Io {
                path: path.clone(),
                source: e,
            })
        }
    };
    let updated = upsert_block(&existing, entries);
    if updated != existing {
        write_file(&path, &updated)?;
    }
    Ok(())
}

/// Remove damon's block (and any legacy damon lines). `repo` is any path
/// inside the repo — the source project dir works after the agent worktree
/// is gone. A missing exclude file is a no-op; any other read error propagates.
pub fn exclude_remove(repo: &Path) -> Result<(), GitError> {
    let path = exclude_path(repo)?;
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(GitError::Io {
                path: path.clone(),
                source: e,
            })
        }
    };
    let (before, _block, after) = split_block(&existing);
    let mut out = String::new();
    for l in before.iter().chain(after.iter()) {
        if !KNOWN_PATTERNS.contains(&l.trim()) {
            out.push_str(l);
            out.push('\n');
        }
    }
    if out != existing {
        write_file(&path, &out)?;
    }
    Ok(())
}

/// (lines before the block, lines inside it, lines after it).
/// Without markers everything is "before".
fn split_block(text: &str) -> (Vec<&str>, Vec<&str>, Vec<&str>) {
    let (mut before, mut block, mut after) = (Vec::new(), Vec::new(), Vec::new());
    let mut state = 0u8; // 0 before, 1 inside, 2 after
    for line in text.lines() {
        match (state, line.trim()) {
            (0, t) if t == BLOCK_BEGIN => state = 1,
            (1, t) if t == BLOCK_END => state = 2,
            (0, _) => before.push(line),
            (1, _) => block.push(line),
            _ => after.push(line),
        }
    }
    (before, block, after)
}

fn upsert_block(existing: &str, entries: &[&str]) -> String {
    let (before, block, after) = split_block(existing);
    // Union of current block + new entries, first-seen order.
    let mut merged: Vec<&str> = Vec::new();
    for e in block
        .iter()
        .map(|l| l.trim())
        .chain(entries.iter().copied())
    {
        if !e.is_empty() && !merged.contains(&e) {
            merged.push(e);
        }
    }
    // Outside the markers only damon's own patterns are ever dropped
    // (legacy pre-block lines); everything else is preserved verbatim.
    let mut out = String::new();
    for l in before
        .iter()
        .filter(|l| !KNOWN_PATTERNS.contains(&l.trim()))
    {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(BLOCK_BEGIN);
    out.push('\n');
    for e in &merged {
        out.push_str(e);
        out.push('\n');
    }
    out.push_str(BLOCK_END);
    out.push('\n');
    for l in after.iter().filter(|l| !KNOWN_PATTERNS.contains(&l.trim())) {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Same-dir temp file + rename; info/exclude is a user-repo file, so a torn
/// write is not acceptable. Temp file is removed if the rename fails.
fn write_file(path: &Path, content: &str) -> Result<(), GitError> {
    let io = |p: &Path, e: std::io::Error| GitError::Io {
        path: p.to_path_buf(),
        source: e,
    };
    let tmp = path.with_extension("damon-tmp");
    std::fs::write(&tmp, content).map_err(|e| io(&tmp, e))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io(path, e));
    }
    Ok(())
}
