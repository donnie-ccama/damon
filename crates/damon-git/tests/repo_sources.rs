use std::path::Path;
use std::process::Command;

/// Neutralize the machine's global/system git config for this test process
/// (affects git spawned by both the test helper and the library under test).
/// All tests set identical values, so parallel calls are benign.
fn isolate_git() {
    std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
    std::env::set_var("GIT_CONFIG_SYSTEM", "/dev/null");
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
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
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("worktree");
    damon_git::init_new(&wt, "agent/scout").unwrap();
    assert_eq!(git(&wt, &["branch", "--show-current"]), "agent/scout");
}

#[test]
fn clone_checks_out_agent_branch() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("worktree");
    damon_git::clone_repo(origin.to_str().unwrap(), &wt, "agent/scout").unwrap();
    assert_eq!(git(&wt, &["branch", "--show-current"]), "agent/scout");
    assert!(wt.join("README.md").exists());
}

#[test]
fn worktree_add_and_remove() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("worktree");
    damon_git::worktree_add(&origin, &wt, "agent/scout").unwrap();
    assert_eq!(git(&wt, &["branch", "--show-current"]), "agent/scout");
    damon_git::worktree_remove(&origin, &wt).unwrap();
    assert!(!wt.exists());
}

#[test]
fn worktree_add_attaches_existing_branch_and_reports_real_errors() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt1 = tmp.path().join("wt1");
    damon_git::worktree_add(&origin, &wt1, "agent/scout").unwrap();
    damon_git::worktree_remove(&origin, &wt1).unwrap();
    // branch agent/scout still exists; re-add must attach, not fail on -b
    let wt2 = tmp.path().join("wt2");
    damon_git::worktree_add(&origin, &wt2, "agent/scout").unwrap();
    // colliding non-empty path surfaces the real error
    let bad = tmp.path().join("bad");
    std::fs::create_dir_all(bad.join("stuff")).unwrap();
    std::fs::write(bad.join("stuff/x"), "x").unwrap();
    let err = damon_git::worktree_add(&origin, &bad, "agent/other").unwrap_err();
    assert!(err.to_string().contains("already exists"), "got: {err}");
    // The error must come from the single deterministic `-b` attempt, not a
    // masked second fallback invocation (the old code retried without -b and
    // surfaced that second attempt's error instead).
    assert!(err.to_string().contains("add -b agent/other"), "got: {err}");
}

#[test]
fn exclude_appends_once_to_common_dir() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("worktree");
    damon_git::worktree_add(&origin, &wt, "agent/scout").unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md"]).unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md"]).unwrap(); // idempotent
    let common = git(
        &wt,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    let text = std::fs::read_to_string(Path::new(&common).join("info/exclude")).unwrap();
    assert_eq!(text.matches("CLAUDE.md").count(), 1);
    assert_eq!(git(&wt, &["status", "--porcelain"]), ""); // bridge file invisible
}
