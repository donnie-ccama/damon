use assert_cmd::Command;
use predicates::str::contains;

struct Env {
    root: tempfile::TempDir,
    cfg: tempfile::TempDir,
    socket: String,
}

fn setup(tag: &str) -> Env {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let socket = format!("cortado-test-{tag}-{}", std::process::id());
    std::fs::write(
        cfg.path().join("config.toml"),
        format!("[tmux]\nsocket = \"{socket}\"\n[terminal]\nlauncher = \"print\"\n"),
    )
    .unwrap();
    let e = Env { root, cfg, socket };
    cortado(&e)
        .args(["team", "new", "Newsletter"])
        .assert()
        .success();
    cortado(&e)
        .args(["agent", "new", "newsletter/Scout", "--repo-new"])
        .assert()
        .success();
    e
}

fn cortado(e: &Env) -> Command {
    let mut cmd = Command::cargo_bin("cortado").unwrap();
    cmd.env("CORTADO_ROOT", e.root.path())
        .env("CORTADO_CONFIG_DIR", e.cfg.path())
        .env("CORTADO_BIN_CLAUDE", "sleep")
        .env("CORTADO_CLAUDE_ARGS", "30"); // test seam: args for the substitute binary
    cmd
}

impl Drop for Env {
    fn drop(&mut self) {
        std::process::Command::new("tmux")
            .args(["-L", &self.socket, "kill-server"])
            .output()
            .ok();
    }
}

// NOTE: `open`'s tmux-era tests that spawn/reattach/kill a session, or that
// assert on removed tmux launcher behavior, were deleted here (see
// .superpowers/sdd/task-5-report.md "Fix: cli_open triage" for the full
// list and reasons). Keeping them would either hit the developer's real
// default Herdr server (no `CORTADO_HERDR_SESSION` isolation seam existed in
// this file) or assert on tmux-specific behavior the Herdr rewrite removed.
// They are superseded by a planned isolated-session CLI round-trip test.
//
// `open_rejects_unknown_model` below is the one test in this file that is
// still fully hermetic under the new `open_session` control flow: model
// lookup happens before `Herdr::new`/`ensure_server()` are ever reached, so
// it cannot touch a Herdr server regardless of environment.
#[test]
fn open_rejects_unknown_model() {
    let e = setup("reject");
    cortado(&e)
        .args(["open", "scout", "--model", "nope"])
        .assert()
        .failure()
        .stderr(contains("model"));
}
