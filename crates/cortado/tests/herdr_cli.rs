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
    run(&["sessions"], &envs)
        .success()
        .stdout(contains("demo/scout"));
    run(&["kill", "scout"], &envs)
        .success()
        .stdout(contains("killed"));
}

/// Restores keyring-error coverage deleted from cli_open.rs in Task 5 (see
/// the comment in that file): `open` now contacts Herdr, via
/// `ensure_server`/`list`, before it resolves model env vars, so the
/// keyring-missing-key path needs an isolated session too, not just
/// `CORTADO_NO_KEYRING`.
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
