use cortado_tmux::Tmux;
use std::collections::BTreeMap;

fn scratch(tag: &str) -> Tmux {
    Tmux::new(format!("cortado-test-{}-{}", tag, std::process::id()))
}

#[test]
fn spawn_list_kill_lifecycle() {
    let t = scratch("lifecycle");
    let tmp = tempfile::tempdir().unwrap();
    let mut env = BTreeMap::new();
    env.insert("CORTADO_AGENT".to_string(), "scout".to_string());
    t.spawn(
        "cortado_newsletter_scout_1",
        tmp.path(),
        &env,
        &["sleep".to_string(), "30".to_string()],
    )
    .unwrap();
    assert!(t.has("cortado_newsletter_scout_1").unwrap());
    assert_eq!(
        t.list().unwrap(),
        vec!["cortado_newsletter_scout_1".to_string()]
    );
    t.kill("cortado_newsletter_scout_1").unwrap();
    assert!(!t.has("cortado_newsletter_scout_1").unwrap());
    t.kill_server().ok();
}

#[test]
fn spawn_error_redacts_env_secret() {
    let t = scratch("redact");
    let tmp = tempfile::tempdir().unwrap();
    let mut env = BTreeMap::new();
    env.insert("SEALED_TOKEN".to_string(), "sekrit-value-42".to_string());
    // First spawn succeeds and leaves the session running.
    t.spawn(
        "cortado_newsletter_scout_1",
        tmp.path(),
        &env,
        &["sleep".to_string(), "30".to_string()],
    )
    .unwrap();
    // Second spawn with the same session name deterministically fails
    // ("duplicate session"), which is when the secret-bearing args used
    // to leak into the error's Display output.
    let err = t
        .spawn(
            "cortado_newsletter_scout_1",
            tmp.path(),
            &env,
            &["sleep".to_string(), "30".to_string()],
        )
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("SEALED_TOKEN=***"),
        "expected redacted key in error, got: {msg}"
    );
    assert!(
        !msg.contains("sekrit-value-42"),
        "secret value leaked into error: {msg}"
    );

    t.kill("cortado_newsletter_scout_1").ok();
    t.kill_server().ok();
}

#[test]
fn pane_lifecycle_split_tag_list_layout() {
    let t = scratch("panes");
    let tmp = tempfile::tempdir().unwrap();
    let env = BTreeMap::new();
    t.spawn(
        "cortado_workspace",
        tmp.path(),
        &env,
        &["sleep".to_string(), "30".to_string()],
    )
    .unwrap();

    // Split with env; returned id names a real pane.
    let mut split_env = BTreeMap::new();
    split_env.insert("TMUX".to_string(), String::new());
    let pane = t
        .split_window(
            "cortado_workspace:0",
            true,
            &split_env,
            &["sleep".to_string(), "30".to_string()],
        )
        .unwrap();
    assert!(pane.starts_with('%'), "pane id, got {pane}");

    // Tag it; list_panes surfaces the tag; the rail pane stays untagged.
    t.set_pane_option(&pane, "@cortado_session", "cortado_t_a_1")
        .unwrap();
    t.set_pane_option(&pane, "@cortado_agent", "t/a").unwrap();
    let panes = t.list_panes("cortado_workspace").unwrap();
    assert_eq!(panes.len(), 2);
    assert_eq!(
        panes
            .iter()
            .find(|p| p.id == pane)
            .unwrap()
            .session_tag
            .as_deref(),
        Some("cortado_t_a_1")
    );
    assert_eq!(panes.iter().filter(|p| p.session_tag.is_none()).count(), 1);

    // Layout + selection + options: exercised for errors, not geometry.
    t.set_window_option("cortado_workspace:0", "main-pane-width", "34")
        .unwrap();
    t.select_layout("cortado_workspace:0", "main-vertical")
        .unwrap();
    t.select_pane(&pane).unwrap();
    t.set_session_options("cortado_workspace", &[("mouse", "on")])
        .unwrap();
    assert_eq!(
        t.show_session_option("cortado_workspace", "mouse").unwrap(),
        "on"
    );
    // No client attached in tests.
    assert!(!t.has_client("cortado_workspace").unwrap());

    t.kill_server().ok();
}

#[test]
fn list_without_server_is_empty() {
    let t = scratch("noserver");
    assert!(t.list().unwrap().is_empty());
}

#[test]
fn version_parses() {
    let (major, _minor) = cortado_tmux::version().unwrap();
    assert!(major >= 3);
}

#[test]
fn list_info_reports_metadata_and_model() {
    let tmux = scratch("info");
    let env = std::collections::BTreeMap::new();
    tmux.spawn(
        "cortado_team_agent_1",
        std::path::Path::new("/tmp"),
        &env,
        &["sleep".to_string(), "30".to_string()],
    )
    .unwrap();
    tmux.set_option("cortado_team_agent_1", "@cortado_model", "claude")
        .unwrap();

    let info = tmux.list_info().unwrap();
    let s = info
        .iter()
        .find(|s| s.name == "cortado_team_agent_1")
        .unwrap();
    assert!(s.created_unix > 1_500_000_000, "created={}", s.created_unix);
    assert_eq!(s.model.as_deref(), Some("claude"));

    tmux.kill("cortado_team_agent_1").ok();
    tmux.kill_server().ok();
}
