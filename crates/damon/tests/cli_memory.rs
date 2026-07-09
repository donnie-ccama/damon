use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::Path;

fn damon(root: &Path, cfg: &Path) -> Command {
    let mut cmd = Command::cargo_bin("damon").unwrap();
    cmd.env("DAMON_ROOT", root).env("DAMON_CONFIG_DIR", cfg);
    cmd
}

fn seed_agent(root: &Path, cfg: &Path) {
    damon(root, cfg)
        .args(["team", "new", "Newsletter"])
        .assert()
        .success();
    damon(root, cfg)
        .args(["agent", "new", "newsletter/Scout", "--repo-new"])
        .assert()
        .success();
}

#[test]
fn memory_prints_all_surfaces_with_headers() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    damon(root.path(), cfg.path())
        .args(["memory", "newsletter/scout"])
        .assert()
        .success()
        .stdout(contains("── AGENT.md ──"))
        .stdout(contains("── USER.md ──"))
        .stdout(contains("── MEMORY.md ──"))
        .stdout(contains("# Scout"))
        .stdout(contains("Write-back protocol"));
}

#[test]
fn memory_prints_single_file_without_header() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    damon(root.path(), cfg.path())
        .args(["memory", "newsletter/scout", "MEMORY.md"])
        .assert()
        .success()
        .stdout(contains("Write-back protocol"))
        .stdout(contains("──").not());
}

#[test]
fn memory_rejects_traversal() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    damon(root.path(), cfg.path())
        .args(["memory", "newsletter/scout", "../agent.toml"])
        .assert()
        .failure();
}
