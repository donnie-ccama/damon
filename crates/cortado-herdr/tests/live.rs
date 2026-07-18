//! Round-trip against a real, isolated herdr server (named session).
//! Skipped when herdr is not installed.
use cortado_herdr::{AgentStatus, Herdr};
use std::process::Command;

fn herdr_available() -> bool {
    Command::new("herdr").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Starts `herdr --session <name> server` detached; stops + deletes on drop.
struct IsoSession {
    name: String,
}

impl IsoSession {
    fn start() -> IsoSession {
        let name = format!("cortadotest{}", std::process::id());
        let h = Herdr::new("herdr".into(), "Cortado".into(), Some(name.clone()));
        h.ensure_server().expect("isolated herdr server should start");
        IsoSession { name }
    }
    fn herdr(&self) -> Herdr {
        Herdr::new("herdr".into(), "Cortado".into(), Some(self.name.clone()))
    }
}

impl Drop for IsoSession {
    fn drop(&mut self) {
        Command::new("herdr").args(["session", "stop", &self.name]).output().ok();
        Command::new("herdr").args(["session", "delete", &self.name]).output().ok();
    }
}

#[test]
fn full_agent_round_trip() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::start();
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
    assert!(!text.is_empty(), "agent read returned empty; raw output shape changed?");
    // Live-verified: `agent read` result nests text under "read": {"text": "..."}.
    // A weak non-empty check alone would still pass on the raw-JSON fallback
    // path, masking a wrong field name — assert the actual sent text is present.
    assert!(text.contains("echo hi"), "expected sent text in agent read output, got: {text}");

    // close removes it
    h.close(&live[0].pane_id).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(h.list().unwrap().is_empty());
}

#[test]
fn server_down_is_typed() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let h = Herdr::new("herdr".into(), "Cortado".into(), Some("neverstarted999".into()));
    match h.list() {
        Err(cortado_herdr::HerdrError::ServerDown(_)) => {}
        other => panic!("expected ServerDown, got {other:?}"),
    }
    // Clean up the session dir herdr may have scaffolded for the name probe.
    Command::new("herdr").args(["session", "delete", "neverstarted999"]).output().ok();
}

#[test]
fn missing_binary_is_not_installed() {
    let h = Herdr::new("definitely-not-herdr-xyz".into(), "Cortado".into(), None);
    match h.list() {
        Err(cortado_herdr::HerdrError::NotInstalled) => {}
        other => panic!("expected NotInstalled, got {other:?}"),
    }
}
