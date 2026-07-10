use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

fn cortado(root: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("cortado").unwrap();
    cmd.env("CORTADO_ROOT", root).env("CORTADO_CONFIG_DIR", cfg);
    cmd
}

#[test]
fn team_new_ls_rm_lifecycle() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "new", "Newsletter Team"])
        .assert()
        .success()
        .stdout(contains("newsletter-team"));
    cortado(root.path(), cfg.path())
        .args(["team", "ls"])
        .assert()
        .success()
        .stdout(contains("newsletter-team").and(contains("Newsletter Team")));
    cortado(root.path(), cfg.path())
        .args(["team", "rm", "newsletter-team"])
        .assert()
        .success();
    cortado(root.path(), cfg.path())
        .args(["team", "ls"])
        .assert()
        .success()
        .stdout(contains("newsletter-team").not());
}

#[test]
fn team_rm_refuses_nonempty_without_force() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "new", "Busy"])
        .assert()
        .success();
    let agent_dir = root.path().join("teams/busy/agents/scout");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("agent.toml"), "[agent]\nname = \"Scout\"\nruntime = \"claude\"\ndefault_model = \"claude\"\n[repo]\nsource = \"new\"\nbranch = \"agent/scout\"\n").unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "rm", "busy"])
        .assert()
        .failure()
        .stderr(contains("--force"));
    cortado(root.path(), cfg.path())
        .args(["team", "rm", "busy", "--force"])
        .assert()
        .success();
}

#[test]
fn team_rm_refuses_when_only_agent_is_invalid() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "new", "Fragile"])
        .assert()
        .success();
    let agent_dir = root.path().join("teams/fragile/agents/broken");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(agent_dir.join("agent.toml"), "not [valid toml").unwrap();
    // an unparseable agent still counts as an agent — rm must refuse without --force
    cortado(root.path(), cfg.path())
        .args(["team", "rm", "fragile"])
        .assert()
        .failure()
        .stderr(contains("--force"));
    cortado(root.path(), cfg.path())
        .args(["team", "rm", "fragile", "--force"])
        .assert()
        .success();
}

#[test]
fn team_ls_shows_invalid_team_with_error() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "new", "Good"])
        .assert()
        .success();
    let bad_dir = root.path().join("teams/bad");
    std::fs::create_dir_all(&bad_dir).unwrap();
    std::fs::write(bad_dir.join("team.toml"), "not [valid toml").unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "ls"])
        .assert()
        .success()
        .stdout(
            contains("good")
                .and(contains("bad"))
                .and(contains("INVALID")),
        );
}

#[test]
fn team_ls_reports_stray_dirs() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "new", "Good"])
        .assert()
        .success();
    std::fs::create_dir_all(root.path().join("teams/Bad Name")).unwrap();
    cortado(root.path(), cfg.path())
        .args(["team", "ls"])
        .assert()
        .success()
        .stdout(
            contains("good")
                .and(contains("Bad Name"))
                .and(contains("INVALID NAME")),
        );
}
