use assert_cmd::Command;
use predicates::str::contains;

// NOTE: the tmux-era workspace-bootstrap mode (`ui_in_workspace_mode_...`)
// was removed here — Herdr now owns pane/window layout, so `cortado ui`
// always draws the rail directly; there is no more separate "become the
// workspace" launch path to test. See task-8-report.md.

#[test]
fn ui_without_a_tty_fails_with_a_clear_message() {
    let cfg = tempfile::tempdir().unwrap();
    // assert_cmd pipes stdout, so is_terminal() is false in the child.
    Command::cargo_bin("cortado")
        .unwrap()
        .env("CORTADO_CONFIG_DIR", cfg.path())
        .arg("ui")
        .assert()
        .failure()
        .stderr(contains("interactive terminal"));
}
