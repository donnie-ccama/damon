use std::path::Path;
use std::process::Command;

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Seed an origin repo with one commit; returns its path.
fn seed_origin(tmp: &Path) -> std::path::PathBuf {
    let origin = tmp.join("origin");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-b", "main"]);
    git(&origin, &["config", "user.email", "t@example.com"]);
    git(&origin, &["config", "user.name", "t"]);
    std::fs::write(origin.join("README.md"), "seed").unwrap();
    git(&origin, &["add", "-A"]);
    git(&origin, &["commit", "-m", "seed"]);
    origin
}

#[test]
fn init_new_creates_repo_on_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("worktree");
    damon_git::init_new(&wt, "agent/scout").unwrap();
    assert_eq!(git(&wt, &["branch", "--show-current"]), "agent/scout");
}

#[test]
fn clone_checks_out_agent_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("worktree");
    damon_git::clone_repo(origin.to_str().unwrap(), &wt, "agent/scout").unwrap();
    assert_eq!(git(&wt, &["branch", "--show-current"]), "agent/scout");
    assert!(wt.join("README.md").exists());
}

#[test]
fn worktree_add_and_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("worktree");
    damon_git::worktree_add(&origin, &wt, "agent/scout").unwrap();
    assert_eq!(git(&wt, &["branch", "--show-current"]), "agent/scout");
    damon_git::worktree_remove(&origin, &wt).unwrap();
    assert!(!wt.exists());
}

#[test]
fn exclude_appends_once_to_common_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("worktree");
    damon_git::worktree_add(&origin, &wt, "agent/scout").unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md"]).unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md"]).unwrap(); // idempotent
    let common = git(&wt, &["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    let text = std::fs::read_to_string(Path::new(&common).join("info/exclude")).unwrap();
    assert_eq!(text.matches("CLAUDE.md").count(), 1);
    assert_eq!(git(&wt, &["status", "--porcelain"]), ""); // bridge file invisible
}
