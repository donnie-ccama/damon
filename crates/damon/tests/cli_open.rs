use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

struct Env {
    root: tempfile::TempDir,
    cfg: tempfile::TempDir,
    socket: String,
}

fn setup(tag: &str) -> Env {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let socket = format!("damon-test-{tag}-{}", std::process::id());
    std::fs::write(
        cfg.path().join("config.toml"),
        format!("[tmux]\nsocket = \"{socket}\"\n[terminal]\nlauncher = \"print\"\n"),
    )
    .unwrap();
    let e = Env { root, cfg, socket };
    damon(&e)
        .args(["team", "new", "Newsletter"])
        .assert()
        .success();
    damon(&e)
        .args(["agent", "new", "newsletter/Scout", "--repo-new"])
        .assert()
        .success();
    e
}

fn damon(e: &Env) -> Command {
    let mut cmd = Command::cargo_bin("damon").unwrap();
    cmd.env("DAMON_ROOT", e.root.path())
        .env("DAMON_CONFIG_DIR", e.cfg.path())
        .env("DAMON_BIN_CLAUDE", "sleep")
        .env("DAMON_CLAUDE_ARGS", "30"); // test seam: args for the substitute binary
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

#[test]
fn open_spawns_session_regenerates_bridge_and_logs() {
    let e = setup("open");
    damon(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("damon_newsletter_scout_1").and(contains("attach with:")));

    let agent = e.root.path().join("teams/newsletter/agents/scout");
    assert!(agent.join("worktree/CLAUDE.md").exists());
    let log = std::fs::read_to_string(agent.join("logs/sessions.jsonl")).unwrap();
    assert!(log.contains("\"event\":\"spawn\""));
    assert!(log.contains("damon_newsletter_scout_1"));

    damon(&e)
        .args(["sessions"])
        .assert()
        .success()
        .stdout(contains("newsletter/scout"));

    // reattach (no --new) does not create a second session
    damon(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("_1"));
    // --new creates _2
    damon(&e)
        .args(["open", "scout", "--new"])
        .assert()
        .success()
        .stdout(contains("_2"));

    damon(&e)
        .args(["kill", "newsletter/scout"])
        .assert()
        .success();
    damon(&e)
        .args(["sessions"])
        .assert()
        .success()
        .stdout(contains("scout").not());
}

#[test]
fn open_rejects_unknown_model_and_m2_features() {
    let e = setup("reject");
    damon(&e)
        .args(["open", "scout", "--model", "nope"])
        .assert()
        .failure()
        .stderr(contains("model"));
    damon(&e)
        .args(["open", "scout", "--model", "kimi"])
        .assert()
        .failure()
        .stderr(contains("M2"));
}

#[test]
fn open_reattaches_highest_n_numerically() {
    let e = setup("numeric");
    for n in ["9", "10"] {
        std::process::Command::new("tmux")
            .args([
                "-L",
                &e.socket,
                "new-session",
                "-d",
                "-s",
                &format!("damon_newsletter_scout_{n}"),
                "--",
                "sleep",
                "30",
            ])
            .status()
            .unwrap();
    }
    // lexically "damon_newsletter_scout_2"-style ordering would pick _9; numeric must pick _10
    damon(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("_10"));
}
