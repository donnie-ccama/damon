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

fn exclude_file(repo: &Path) -> std::path::PathBuf {
    let common = git(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    Path::new(&common).join("info/exclude")
}

#[test]
fn exclude_writes_marked_block_idempotently() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("repo");
    damon_git::init_new(&wt, "main").unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md", ".claude/settings.json"]).unwrap();
    let first = std::fs::read_to_string(exclude_file(&wt)).unwrap();
    assert!(first.contains("# damon begin\nCLAUDE.md\n.claude/settings.json\n# damon end\n"));
    damon_git::exclude(&wt, &["CLAUDE.md", ".claude/settings.json"]).unwrap();
    let second = std::fs::read_to_string(exclude_file(&wt)).unwrap();
    assert_eq!(first, second); // byte-identical on repeat
    assert_eq!(first.matches("# damon begin").count(), 1);
}

#[test]
fn exclude_merges_entries_across_calls() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("repo");
    damon_git::init_new(&wt, "main").unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md"]).unwrap();
    damon_git::exclude(&wt, &["AGENTS.md"]).unwrap();
    let text = std::fs::read_to_string(exclude_file(&wt)).unwrap();
    assert!(text.contains("# damon begin\nCLAUDE.md\nAGENTS.md\n# damon end\n"));
    assert_eq!(text.matches("# damon begin").count(), 1);
}

#[test]
fn exclude_preserves_user_lines_and_migrates_legacy_damon_lines() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("repo");
    damon_git::init_new(&wt, "main").unwrap();
    let file = exclude_file(&wt);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    // A user pattern plus a pre-M4 unmarked damon line.
    std::fs::write(&file, "user-pattern\nCLAUDE.md\n").unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md"]).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("user-pattern\n"), "{text:?}");
    // Legacy line absorbed into the block — exactly one CLAUDE.md remains.
    assert_eq!(text.matches("CLAUDE.md").count(), 1);
    assert!(text.contains("# damon begin\nCLAUDE.md\n# damon end\n"));
}

#[test]
fn exclude_remove_deletes_only_the_block() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("repo");
    damon_git::init_new(&wt, "main").unwrap();
    let file = exclude_file(&wt);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "user-pattern\n").unwrap();
    damon_git::exclude(&wt, &["CLAUDE.md", "AGENTS.md"]).unwrap();
    damon_git::exclude_remove(&wt).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text, "user-pattern\n");
}

#[test]
fn exclude_remove_without_exclude_file_is_ok() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("repo");
    damon_git::init_new(&wt, "main").unwrap();
    damon_git::exclude_remove(&wt).unwrap(); // file never written — no error
}

#[test]
fn common_dir_matches_for_source_repo_and_its_worktree() {
    isolate_git();
    let tmp = tempfile::tempdir().unwrap();
    let origin = seed_origin(tmp.path());
    let wt = tmp.path().join("wt");
    damon_git::worktree_add(&origin, &wt, "agent/scout").unwrap();
    let a = damon_git::common_dir(&origin).unwrap();
    let b = damon_git::common_dir(&wt).unwrap();
    assert_eq!(a.canonicalize().unwrap(), b.canonicalize().unwrap());
}
