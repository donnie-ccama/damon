use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

fn damon(root: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("damon").unwrap();
    cmd.env("DAMON_ROOT", root).env("DAMON_CONFIG_DIR", cfg);
    cmd
}

#[test]
fn doctor_fails_with_hints_when_required_tools_missing() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    damon(root.path(), cfg.path())
        .arg("doctor")
        .env("PATH", "") // nothing findable
        .assert()
        .failure()
        .stdout(contains("git").and(contains("tmux")).and(contains("install")));
}

#[test]
fn doctor_succeeds_when_git_and_tmux_present() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    damon(root.path(), cfg.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("git").and(contains("ok")));
}
