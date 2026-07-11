use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

/// Temp config dir pinning the launcher, so the test never reads the real
/// user config (whose default is now the workspace bootstrap).
fn cfg_dir(socket: &str, terminal: &str) -> tempfile::TempDir {
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(
        cfg.path().join("config.toml"),
        format!("[tmux]\nsocket = \"{socket}\"\n[terminal]\n{terminal}\n"),
    )
    .unwrap();
    cfg
}

#[test]
fn ui_without_a_tty_fails_with_a_clear_message() {
    let cfg = cfg_dir("cortado-test-uitty", "launcher = \"print\"");
    // assert_cmd pipes stdout, so is_terminal() is false in the child.
    Command::cargo_bin("cortado")
        .unwrap()
        .env("CORTADO_CONFIG_DIR", cfg.path())
        .arg("ui")
        .assert()
        .failure()
        .stderr(contains("interactive terminal"));
}

#[test]
fn ui_in_workspace_mode_creates_workspace_and_prints_attach_hint() {
    let socket = format!("cortado-test-uiboot-{}", std::process::id());
    let cfg = cfg_dir(&socket, "launcher = \"workspace\"\nwindow = \"print\"");
    let root = tempfile::tempdir().unwrap();

    Command::cargo_bin("cortado")
        .unwrap()
        .env("CORTADO_CONFIG_DIR", cfg.path())
        .env("CORTADO_ROOT", root.path())
        .arg("ui")
        .assert()
        .success()
        .stdout(
            contains("workspace cortado_workspace").and(contains(format!(
                "tmux -L {socket} attach -t cortado_workspace"
            ))),
        );

    let tmux = cortado_tmux::Tmux::new(socket.clone());
    assert!(tmux.has("cortado_workspace").unwrap());
    tmux.kill_server().ok();
}
