use assert_cmd::Command;
use predicates::str::contains;
use std::path::Path;

fn damon(root: &Path, cfg: &Path) -> Command {
    let mut cmd = Command::cargo_bin("damon").unwrap();
    cmd.env("DAMON_ROOT", root).env("DAMON_CONFIG_DIR", cfg);
    cmd
}

fn git(cwd: &Path, args: &[&str]) {
    assert!(std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap().status.success());
}

#[test]
fn agent_new_repo_new_scaffolds_everything() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    damon(root.path(), cfg.path()).args(["team", "new", "Newsletter"]).assert().success();
    damon(root.path(), cfg.path())
        .args(["agent", "new", "newsletter/Scout", "--role", "Researches topics", "--repo-new"])
        .assert()
        .success();
    let agent = root.path().join("teams/newsletter/agents/scout");
    assert!(agent.join("agent.toml").exists());
    assert!(agent.join("memory/AGENT.md").exists());
    assert!(agent.join("memory/MEMORY.md").exists());
    assert!(agent.join("worktree/.git").exists());
    assert!(agent.join("logs").is_dir());
    let toml = std::fs::read_to_string(agent.join("agent.toml")).unwrap();
    assert!(toml.contains("source = \"new\""));
    assert!(toml.contains("branch = \"agent/scout\""));

    damon(root.path(), cfg.path()).args(["agent", "ls"]).assert().success().stdout(contains("newsletter/scout"));
}

#[test]
fn agent_new_worktree_attaches_to_existing_repo() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    git(project.path(), &["init", "-b", "main"]);
    git(project.path(), &["config", "user.email", "t@example.com"]);
    git(project.path(), &["config", "user.name", "t"]);
    std::fs::write(project.path().join("README.md"), "x").unwrap();
    git(project.path(), &["add", "-A"]);
    git(project.path(), &["commit", "-m", "seed"]);

    damon(root.path(), cfg.path()).args(["team", "new", "Web"]).assert().success();
    damon(root.path(), cfg.path())
        .args(["agent", "new", "web/Fixer", "--repo-worktree", project.path().to_str().unwrap()])
        .assert()
        .success();
    let wt = root.path().join("teams/web/agents/fixer/worktree");
    assert!(wt.join("README.md").exists());

    // rm detaches the worktree from the project repo
    damon(root.path(), cfg.path()).args(["agent", "rm", "web/fixer", "--yes"]).assert().success();
    assert!(!wt.exists());
}

#[test]
fn agent_new_requires_team_and_repo_flag() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    damon(root.path(), cfg.path()).args(["agent", "new", "ghost/Scout", "--repo-new"]).assert().failure().stderr(contains("team"));
}
