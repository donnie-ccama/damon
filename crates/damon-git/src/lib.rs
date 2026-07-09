//! Git operations for damon agents. Shells out to the git CLI.
use std::path::{Path, PathBuf};
use std::process::Command;

use fs4::fs_std::FileExt;

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

/// The bridge filenames damon-git recognizes for legacy-line migration.
/// Kept in sync with `damon_core::bridge::write_bridges` by a cross-crate
/// test in the `damon` crate (`tests/bridge_exclude_sync.rs`).
pub fn known_patterns() -> &'static [&'static str] {
    &KNOWN_PATTERNS
}

/// Absolute git common dir for the repo containing `path`.
pub fn common_dir(path: &Path) -> Result<PathBuf, GitError> {
    let out = git(
        Some(path),
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Ok(PathBuf::from(out))
}

/// Run `f` while holding an exclusive advisory lock scoped to this repo's
/// exclude file, so two concurrent `damon open`s cannot lose an update. The
/// lock lives on a stable sidecar file (never renamed) — locking the exclude
/// file itself is unsound because `write_file` swaps its inode via rename.
/// flock is released on fd close, so a crash leaves no stale lock.
fn with_exclude_lock<T>(
    common: &Path,
    f: impl FnOnce() -> Result<T, GitError>,
) -> Result<T, GitError> {
    let info = common.join("info");
    std::fs::create_dir_all(&info).map_err(|source| GitError::Io {
        path: info.clone(),
        source,
    })?;
    let lock_path = info.join(".damon-exclude.lock");
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|source| GitError::Io {
            path: lock_path.clone(),
            source,
        })?;
    lock.lock_exclusive().map_err(|source| GitError::Io {
        path: lock_path.clone(),
        source,
    })?;
    let result = f();
    let _ = FileExt::unlock(&lock); // also released on drop
    result
}

/// Ensure `entries` are ignored via a sentinel-delimited block in
/// <git-common-dir>/info/exclude. Idempotent; lines outside the markers are
/// never touched, except legacy damon patterns, which migrate into the block.
/// A missing exclude file starts empty; any other read error propagates
/// rather than risking a clobber.
pub fn exclude(worktree: &Path, entries: &[&str]) -> Result<(), GitError> {
    let common = common_dir(worktree)?;
    with_exclude_lock(&common, || {
        let path = common.join("info").join("exclude");
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
    })
}

/// Remove damon's block (and any legacy damon lines). `repo` is any path
/// inside the repo — the source project dir works after the agent worktree
/// is gone. A missing exclude file is a no-op; any other read error propagates.
pub fn exclude_remove(repo: &Path) -> Result<(), GitError> {
    let common = common_dir(repo)?;
    with_exclude_lock(&common, || {
        let path = common.join("info").join("exclude");
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
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn exclude_lock_serializes_critical_sections() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().to_path_buf();
        let inside = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let (common, inside, max) = (common.clone(), inside.clone(), max.clone());
            handles.push(std::thread::spawn(move || {
                for _ in 0..25 {
                    with_exclude_lock(&common, || {
                        let n = inside.fetch_add(1, Ordering::SeqCst) + 1;
                        max.fetch_max(n, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_micros(200));
                        inside.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // With a real flock, at most one thread is ever inside the closure.
        assert_eq!(
            max.load(Ordering::SeqCst),
            1,
            "critical sections overlapped"
        );
    }
}
