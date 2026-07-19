use assert_cmd::Command;
use predicates::str::contains;

struct Env {
    root: tempfile::TempDir,
    cfg: tempfile::TempDir,
}

fn setup(_tag: &str) -> Env {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let e = Env { root, cfg };
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

// NOTE: `open`'s tmux-era tests that spawned/reattached/killed a session, or
// asserted on removed tmux launcher behavior, were deleted from this file.
// Under the Herdr rewrite, `open_session` contacts Herdr (`Herdr::new` +
// `ensure_server()` + `list()`) before most of what those tests exercised —
// keeping them here would either hit the developer's real default Herdr
// server (this file has no `CORTADO_HERDR_SESSION` isolation seam) or assert
// on tmux-specific behavior that no longer exists. Coverage that needs to
// touch a real (isolated) Herdr server lives in `herdr_cli.rs` instead,
// where each test owns a named session via `CORTADO_HERDR_SESSION` and
// cleans it up on drop.
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
