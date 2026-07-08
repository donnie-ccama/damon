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
fn open_rejects_unknown_model() {
    let e = setup("reject");
    damon(&e)
        .args(["open", "scout", "--model", "nope"])
        .assert()
        .failure()
        .stderr(contains("model"));
}

#[test]
fn open_opencode_spawns_and_writes_agents_md() {
    let e = setup("opencode");
    // Create an opencode agent
    damon(&e)
        .args([
            "agent",
            "new",
            "newsletter/opencode",
            "--runtime",
            "opencode",
            "--repo-new",
        ])
        .assert()
        .success();

    // Open it with env vars for the sleep substitute binary
    damon(&e)
        .env("DAMON_BIN_OPENCODE", "sleep")
        .env("DAMON_OPENCODE_ARGS", "30")
        .args(["open", "opencode"])
        .assert()
        .success()
        .stdout(contains("damon_newsletter_opencode_1"));

    // Verify AGENTS.md was written
    let agent = e.root.path().join("teams/newsletter/agents/opencode");
    assert!(agent.join("worktree/AGENTS.md").exists());
    let agents_content = std::fs::read_to_string(agent.join("worktree/AGENTS.md")).unwrap();
    assert!(agents_content.contains("# opencode — Damon OpenCode agent"));
}

#[test]
fn open_resolves_keyring_placeholder_via_seam() {
    let e = setup("keyseam");
    std::fs::write(
        e.cfg.path().join("models.toml"),
        "[models.sealed]\nlabel = \"Sealed\"\nruntime = \"claude\"\nenv = { FAKE_TOKEN = \"${keyring:damontest}\" }\n",
    )
    .unwrap();
    damon(&e)
        .env("DAMON_KEY_DAMONTEST", "sekrit-42")
        .args(["open", "scout", "--model", "sealed"])
        .assert()
        .success();
    let out = std::process::Command::new("tmux")
        .args([
            "-L",
            &e.socket,
            "show-environment",
            "-t",
            "damon_newsletter_scout_1",
            "FAKE_TOKEN",
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("FAKE_TOKEN=sekrit-42"));
}

#[test]
fn open_missing_key_names_the_fix() {
    let e = setup("keymiss");
    std::fs::write(
        e.cfg.path().join("models.toml"),
        "[models.sealed]\nlabel = \"Sealed\"\nruntime = \"claude\"\nenv = { T = \"${keyring:damon-test-noexist-xyz}\" }\n",
    )
    .unwrap();
    damon(&e)
        .env("DAMON_NO_KEYRING", "1")
        .args(["open", "scout", "--model", "sealed"])
        .assert()
        .failure()
        .stderr(contains("damon key set damon-test-noexist-xyz"));
}

#[test]
fn open_rejects_empty_keyring_account() {
    let e = setup("keyempty");
    std::fs::write(
        e.cfg.path().join("models.toml"),
        "[models.sealed]\nlabel = \"Sealed\"\nruntime = \"claude\"\nenv = { T = \"${keyring:}\" }\n",
    )
    .unwrap();
    damon(&e)
        .args(["open", "scout", "--model", "sealed"])
        .assert()
        .failure()
        .stderr(contains(
            "has an empty ${keyring:} account for \"T\" — fix models.toml",
        ));
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
