//! cortado CLI <-> real herdr round-trip on an isolated named session.
//!
//! Never touches the developer's default herdr session: every invocation
//! carries `CORTADO_HERDR_SESSION` set to a per-process, per-test name, and
//! `IsoSession::drop` stops + deletes that named session afterward. See
//! `crates/cortado-herdr/tests/live.rs` for the wrapper-level counterpart of
//! this isolation pattern.
use assert_cmd::Command;
use predicates::str::contains;

fn herdr_available() -> bool {
    std::process::Command::new("herdr")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A herdr session name unique to this test process. `cortado open` starts
/// the session's server itself (via `ensure_server`), so this struct only
/// owns cleanup, not startup.
struct IsoSession {
    name: String,
}

impl IsoSession {
    fn new(tag: &str) -> IsoSession {
        IsoSession {
            name: format!("cortadocli{tag}{}", std::process::id()),
        }
    }
}

impl Drop for IsoSession {
    fn drop(&mut self) {
        std::process::Command::new("herdr")
            .args(["session", "stop", &self.name])
            .output()
            .ok();
        std::process::Command::new("herdr")
            .args(["session", "delete", &self.name])
            .output()
            .ok();
    }
}

fn run(args: &[&str], envs: &[(&str, String)]) -> assert_cmd::assert::Assert {
    let mut c = Command::cargo_bin("cortado").unwrap();
    for (k, v) in envs {
        c.env(k, v);
    }
    c.args(args).assert()
}

#[test]
fn open_sessions_kill_round_trip() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::new("");
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let envs: Vec<(&str, String)> = vec![
        ("CORTADO_ROOT", root.path().to_str().unwrap().to_string()),
        (
            "CORTADO_CONFIG_DIR",
            cfg.path().to_str().unwrap().to_string(),
        ),
        ("CORTADO_HERDR_SESSION", iso.name.clone()),
        // Runtime substitute: same seam cli_open.rs uses (RuntimeId::binary()
        // honors CORTADO_BIN_<RUNTIME>; open.rs appends CORTADO_<RUNTIME>_ARGS
        // split on whitespace) so `open` spawns a long-lived dummy process
        // ("sleep 30") instead of real Claude Code.
        ("CORTADO_BIN_CLAUDE", "sleep".to_string()),
        ("CORTADO_CLAUDE_ARGS", "30".to_string()),
    ];

    run(&["team", "new", "Demo"], &envs).success();
    run(&["agent", "new", "demo/Scout", "--repo-new"], &envs).success();
    run(&["open", "scout"], &envs)
        .success()
        .stdout(contains("cortado_demo_scout_1"));
    // Reattach: a plain `open` on an agent with a live session focuses it
    // rather than spawning a second one.
    run(&["open", "scout"], &envs)
        .success()
        .stdout(contains("cortado_demo_scout_1"));
    // `--new` always spawns a fresh session regardless of the live one.
    run(&["open", "scout", "--new"], &envs)
        .success()
        .stdout(contains("cortado_demo_scout_2"));
    run(&["sessions"], &envs)
        .success()
        .stdout(contains("demo/scout"));
    run(&["kill", "scout"], &envs)
        .success()
        .stdout(contains("killed"));
}

/// A failed hermetic pre-check (bad runtime binary) must not spawn anything
/// or append a spawn event to sessions.jsonl. With Fix 1's reorder, the
/// runtime-binary check now runs before Herdr is ever contacted, so this
/// path is purely hermetic — a live herdr session is not strictly required
/// to observe the failure, but `CORTADO_HERDR_SESSION` is still set as a
/// belt-and-suspenders guard in case that ordering ever regresses.
#[test]
fn failed_binary_check_does_not_log_a_spawn_event() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::new("badbin");
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let envs: Vec<(&str, String)> = vec![
        ("CORTADO_ROOT", root.path().to_str().unwrap().to_string()),
        (
            "CORTADO_CONFIG_DIR",
            cfg.path().to_str().unwrap().to_string(),
        ),
        ("CORTADO_HERDR_SESSION", iso.name.clone()),
        (
            "CORTADO_BIN_CLAUDE",
            "/nonexistent-cortado-runtime-binary".to_string(),
        ),
    ];

    run(&["team", "new", "Demo"], &envs).success();
    run(&["agent", "new", "demo/Scout", "--repo-new"], &envs).success();
    run(&["open", "scout"], &envs)
        .failure()
        .stderr(contains("was not found"));

    let sessions_log = root
        .path()
        .join("teams")
        .join("demo")
        .join("agents")
        .join("scout")
        .join("logs")
        .join("sessions.jsonl");
    assert!(
        !sessions_log.exists(),
        "a failed open must not append a spawn event"
    );
}

/// Restores keyring-error coverage deleted from cli_open.rs in Task 5 (see
/// the comment in that file). `open` now resolves model env vars (and thus
/// the keyring) hermetically, before it ever contacts Herdr — this test no
/// longer strictly needs a live herdr session to hit the error, but keeps
/// the isolated session anyway as a safety net in case the hermetic ordering
/// regresses.
#[test]
fn open_missing_key_names_the_fix() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::new("key");
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(
        cfg.path().join("models.toml"),
        r#"[models.claude]
label = "Claude"
runtime = "claude"

[models.brokenkey]
label = "Broken"
runtime = "claude"
env = { ANTHROPIC_API_KEY = "${keyring:nonexistent-acct}" }
"#,
    )
    .unwrap();
    let envs: Vec<(&str, String)> = vec![
        ("CORTADO_ROOT", root.path().to_str().unwrap().to_string()),
        (
            "CORTADO_CONFIG_DIR",
            cfg.path().to_str().unwrap().to_string(),
        ),
        ("CORTADO_HERDR_SESSION", iso.name.clone()),
        ("CORTADO_NO_KEYRING", "1".to_string()),
    ];

    run(&["team", "new", "Demo"], &envs).success();
    run(&["agent", "new", "demo/Scout", "--repo-new"], &envs).success();
    run(&["open", "scout", "--model", "brokenkey"], &envs)
        .failure()
        .stderr(contains("run: cortado key set"));
}
