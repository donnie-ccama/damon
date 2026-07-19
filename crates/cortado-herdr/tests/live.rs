//! Round-trip against a real, isolated herdr server (named session).
//! Skipped when herdr is not installed.
use cortado_herdr::{AgentStatus, Herdr};
use std::process::Command;

fn herdr_available() -> bool {
    Command::new("herdr")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Starts `herdr --session <name> server` detached; stops + deletes on drop.
/// `tag` disambiguates sessions between tests in this file — the process id
/// alone collides when two IsoSession-backed tests run concurrently (the
/// default `cargo test` threading), since they share one pid.
struct IsoSession {
    name: String,
}

impl IsoSession {
    fn start(tag: &str) -> IsoSession {
        let name = format!("cortadotest{tag}{}", std::process::id());
        let h = Herdr::new("herdr".into(), "Cortado".into(), Some(name.clone()));
        h.ensure_server()
            .expect("isolated herdr server should start");
        IsoSession { name }
    }
    fn herdr(&self) -> Herdr {
        Herdr::new("herdr".into(), "Cortado".into(), Some(self.name.clone()))
    }
}

impl Drop for IsoSession {
    fn drop(&mut self) {
        Command::new("herdr")
            .args(["session", "stop", &self.name])
            .output()
            .ok();
        Command::new("herdr")
            .args(["session", "delete", &self.name])
            .output()
            .ok();
    }
}

#[test]
fn full_agent_round_trip() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::start("full");
    let h = iso.herdr();

    // ensure_workspace is idempotent
    let ws = h.ensure_workspace().unwrap();
    assert_eq!(h.ensure_workspace().unwrap(), ws);

    // start a dummy agent
    let mut env = std::collections::BTreeMap::new();
    env.insert("CORTADO_TEAM".to_string(), "demo".to_string());
    let started = h
        .start(
            "cortado_demo_scout_1",
            std::path::Path::new("/tmp"),
            &env,
            &["sh".to_string(), "-c".to_string(), "sleep 120".to_string()],
            &ws,
            false,
        )
        .unwrap();
    assert_eq!(started.name, "cortado_demo_scout_1");

    // list sees it
    let live = h.list().unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].name, "cortado_demo_scout_1");
    assert!(matches!(
        live[0].status,
        AgentStatus::Unknown | AgentStatus::Idle | AgentStatus::Working
    ));

    // focus works
    h.focus("cortado_demo_scout_1").unwrap();

    // send + read plumbing (M7 consumers; verify they do not error)
    h.send("cortado_demo_scout_1", "echo hi").unwrap();
    let text = h.read("cortado_demo_scout_1", 50).unwrap();
    assert!(
        !text.is_empty(),
        "agent read returned empty; raw output shape changed?"
    );
    // Live-verified: `agent read` result nests text under "read": {"text": "..."}.
    // A weak non-empty check alone would still pass on the raw-JSON fallback
    // path, masking a wrong field name — assert the actual sent text is present.
    assert!(
        text.contains("echo hi"),
        "expected sent text in agent read output, got: {text}"
    );

    // close removes it
    h.close(&live[0].pane_id).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(h.list().unwrap().is_empty());
}

/// Two concurrent `ensure_workspace` calls (as two concurrent `cortado
/// open`s would trigger) must not create two "Cortado" workspaces. Guards
/// the fs4-flock fix around the list-then-create span.
#[test]
fn ensure_workspace_race_creates_one_workspace() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::start("ws");
    let h1 = iso.herdr();
    let h2 = iso.herdr();
    let t1 = std::thread::spawn(move || h1.ensure_workspace().unwrap());
    let t2 = std::thread::spawn(move || h2.ensure_workspace().unwrap());
    let id1 = t1.join().unwrap();
    let id2 = t2.join().unwrap();
    assert_eq!(id1, id2, "both calls should resolve to the same workspace");

    let out = Command::new("herdr")
        .args(["--session", &iso.name, "workspace", "list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let v = cortado_herdr::parse_envelope(&stdout).unwrap();
    let workspaces = cortado_herdr::parse_workspace_list(&v).unwrap();
    let cortado_count = workspaces
        .iter()
        .filter(|(_, label)| label == "Cortado")
        .count();
    assert_eq!(
        cortado_count, 1,
        "expected exactly one Cortado workspace, got {workspaces:?}"
    );
}

#[test]
fn server_down_is_typed() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let h = Herdr::new(
        "herdr".into(),
        "Cortado".into(),
        Some("neverstarted999".into()),
    );
    match h.list() {
        Err(cortado_herdr::HerdrError::ServerDown(_)) => {}
        other => panic!("expected ServerDown, got {other:?}"),
    }
    // Clean up the session dir herdr may have scaffolded for the name probe.
    Command::new("herdr")
        .args(["session", "delete", "neverstarted999"])
        .output()
        .ok();
}

#[test]
fn missing_binary_is_not_installed() {
    let h = Herdr::new("definitely-not-herdr-xyz".into(), "Cortado".into(), None);
    match h.list() {
        Err(cortado_herdr::HerdrError::NotInstalled) => {}
        other => panic!("expected NotInstalled, got {other:?}"),
    }
}

#[test]
fn ensure_server_timeout_reaps_child_and_errors() {
    // `false server` exits immediately; status polls fail; after ~5s we must
    // get ServerDown, not a hang or a leaked child.
    let h = Herdr::new("false".into(), "Cortado".into(), None);
    match h.ensure_server() {
        Err(cortado_herdr::HerdrError::ServerDown(_)) => {}
        other => panic!("expected ServerDown, got {other:?}"),
    }
}
