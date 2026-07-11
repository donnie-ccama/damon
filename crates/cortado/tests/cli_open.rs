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

#[test]
fn open_spawns_session_regenerates_bridge_and_logs() {
    let e = setup("open");
    cortado(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("cortado_newsletter_scout_1").and(contains("attach with:")));

    let agent = e.root.path().join("teams/newsletter/agents/scout");
    assert!(agent.join("worktree/CLAUDE.md").exists());
    let log = std::fs::read_to_string(agent.join("logs/sessions.jsonl")).unwrap();
    assert!(log.contains("\"event\":\"spawn\""));
    assert!(log.contains("cortado_newsletter_scout_1"));

    cortado(&e)
        .args(["sessions"])
        .assert()
        .success()
        .stdout(contains("newsletter/scout"));

    // reattach (no --new) does not create a second session
    cortado(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("_1"));
    // --new creates _2
    cortado(&e)
        .args(["open", "scout", "--new"])
        .assert()
        .success()
        .stdout(contains("_2"));

    cortado(&e)
        .args(["kill", "newsletter/scout"])
        .assert()
        .success();
    cortado(&e)
        .args(["sessions"])
        .assert()
        .success()
        .stdout(contains("scout").not());
}

#[test]
fn open_rejects_unknown_model() {
    let e = setup("reject");
    cortado(&e)
        .args(["open", "scout", "--model", "nope"])
        .assert()
        .failure()
        .stderr(contains("model"));
}

#[test]
fn open_opencode_spawns_and_writes_agents_md() {
    let e = setup("opencode");
    // Create an opencode agent
    cortado(&e)
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
    cortado(&e)
        .env("CORTADO_BIN_OPENCODE", "sleep")
        .env("CORTADO_OPENCODE_ARGS", "30")
        .args(["open", "opencode"])
        .assert()
        .success()
        .stdout(contains("cortado_newsletter_opencode_1"));

    // Verify AGENTS.md was written
    let agent = e.root.path().join("teams/newsletter/agents/opencode");
    assert!(agent.join("worktree/AGENTS.md").exists());
    let agents_content = std::fs::read_to_string(agent.join("worktree/AGENTS.md")).unwrap();
    assert!(agents_content.contains("# opencode — Cortado OpenCode agent"));
}

#[test]
fn open_resolves_keyring_placeholder_via_seam() {
    let e = setup("keyseam");
    std::fs::write(
        e.cfg.path().join("models.toml"),
        "[models.sealed]\nlabel = \"Sealed\"\nruntime = \"claude\"\nenv = { FAKE_TOKEN = \"${keyring:cortadotest}\" }\n",
    )
    .unwrap();
    cortado(&e)
        .env("CORTADO_KEY_CORTADOTEST", "sekrit-42")
        .args(["open", "scout", "--model", "sealed"])
        .assert()
        .success();
    let out = std::process::Command::new("tmux")
        .args([
            "-L",
            &e.socket,
            "show-environment",
            "-t",
            "cortado_newsletter_scout_1",
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
        "[models.sealed]\nlabel = \"Sealed\"\nruntime = \"claude\"\nenv = { T = \"${keyring:cortado-test-noexist-xyz}\" }\n",
    )
    .unwrap();
    cortado(&e)
        .env("CORTADO_NO_KEYRING", "1")
        .args(["open", "scout", "--model", "sealed"])
        .assert()
        .failure()
        .stderr(contains("cortado key set cortado-test-noexist-xyz"));
}

#[test]
fn open_rejects_empty_keyring_account() {
    let e = setup("keyempty");
    std::fs::write(
        e.cfg.path().join("models.toml"),
        "[models.sealed]\nlabel = \"Sealed\"\nruntime = \"claude\"\nenv = { T = \"${keyring:}\" }\n",
    )
    .unwrap();
    cortado(&e)
        .args(["open", "scout", "--model", "sealed"])
        .assert()
        .failure()
        .stderr(contains(
            "has an empty ${keyring:} account for \"T\" — fix models.toml",
        ));
}

/// Like `setup`, but the config selects the single-window workspace with a
/// print window (no OS window in tests).
fn setup_workspace(tag: &str) -> Env {
    let e = setup(tag);
    std::fs::write(
        e.cfg.path().join("config.toml"),
        format!(
            "[tmux]\nsocket = \"{}\"\n[terminal]\nlauncher = \"workspace\"\nwindow = \"print\"\n",
            e.socket
        ),
    )
    .unwrap();
    e
}

#[test]
fn workspace_mode_spawns_agent_sessions_with_inner_tmux_disabled() {
    let e = setup_workspace("wsmode");
    cortado(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("cortado_newsletter_scout_1"));

    let tmux = cortado_tmux::Tmux::new(e.socket.clone());
    let sessions = tmux.list().unwrap();
    let agent_session = sessions
        .iter()
        .find(|s| s.starts_with("cortado_") && *s != "cortado_workspace")
        .expect("agent session spawned");
    assert_eq!(
        tmux.show_session_option(agent_session, "prefix").unwrap(),
        "None"
    );
    assert_eq!(
        tmux.show_session_option(agent_session, "status").unwrap(),
        "off"
    );
    // And the workspace exists with a viewer pane tagged for it.
    let panes = tmux.list_panes("cortado_workspace").unwrap();
    assert!(panes
        .iter()
        .any(|p| p.session_tag.as_deref() == Some(agent_session.as_str())));
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
                &format!("cortado_newsletter_scout_{n}"),
                "--",
                "sleep",
                "30",
            ])
            .status()
            .unwrap();
    }
    // lexically "cortado_newsletter_scout_2"-style ordering would pick _9; numeric must pick _10
    cortado(&e)
        .args(["open", "scout"])
        .assert()
        .success()
        .stdout(contains("_10"));
}
