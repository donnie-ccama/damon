use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::Path;

fn cortado(root: &Path, cfg: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cortado").unwrap();
    cmd.env("CORTADO_ROOT", root).env("CORTADO_CONFIG_DIR", cfg);
    cmd
}

fn seed_agent(root: &Path, cfg: &Path) {
    cortado(root, cfg)
        .args(["team", "new", "Newsletter"])
        .assert()
        .success();
    cortado(root, cfg)
        .args(["agent", "new", "newsletter/Scout", "--repo-new"])
        .assert()
        .success();
}

#[test]
fn memory_prints_all_surfaces_with_headers() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    cortado(root.path(), cfg.path())
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
    cortado(root.path(), cfg.path())
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
    cortado(root.path(), cfg.path())
        .args(["memory", "newsletter/scout", "../agent.toml"])
        .assert()
        .failure();
}

#[test]
fn memory_edit_launches_editor_and_propagates_exit() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    // `true` ignores its file argument and exits 0.
    cortado(root.path(), cfg.path())
        .env("VISUAL", "")
        .env("EDITOR", "true")
        .args(["memory", "newsletter/scout", "--edit"])
        .assert()
        .success();
    // `false` exits 1 — cortado must propagate it.
    cortado(root.path(), cfg.path())
        .env("VISUAL", "")
        .env("EDITOR", "false")
        .args(["memory", "newsletter/scout", "--edit"])
        .assert()
        .code(1);
}

#[test]
fn memory_edit_refuses_nonexistent_file() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    cortado(root.path(), cfg.path())
        .env("VISUAL", "")
        .env("EDITOR", "true")
        .args(["memory", "newsletter/scout", "TYPO.md", "--edit"])
        .assert()
        .failure(); // --edit never creates files
}

#[test]
fn memory_errors_when_memory_dir_is_missing() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    seed_agent(root.path(), cfg.path());
    std::fs::remove_dir_all(root.path().join("teams/newsletter/agents/scout/memory")).unwrap();
    cortado(root.path(), cfg.path())
        .args(["memory", "newsletter/scout"])
        .assert()
        .failure()
        .stderr(contains("no memory directory"));
}
