use cortado_term::workspace;
use cortado_tmux::Tmux;
use std::collections::BTreeMap;

fn scratch(tag: &str) -> Tmux {
    Tmux::new(format!("cortado-test-{}-{}", tag, std::process::id()))
}

fn sleep_cmd() -> Vec<String> {
    vec!["sleep".to_string(), "30".to_string()]
}

#[test]
fn workspace_viewer_lifecycle() {
    let t = scratch("wsviewer");
    let tmp = tempfile::tempdir().unwrap();

    // A fake agent session, as open_session would have spawned it.
    t.spawn("cortado_t_a_1", tmp.path(), &BTreeMap::new(), &sleep_cmd())
        .unwrap();

    // ensure_workspace is idempotent and applies options.
    workspace::ensure_workspace(&t, tmp.path(), &sleep_cmd()).unwrap();
    workspace::ensure_workspace(&t, tmp.path(), &sleep_cmd()).unwrap();
    assert!(t.has(workspace::WORKSPACE_SESSION).unwrap());
    assert_eq!(
        t.show_session_option(workspace::WORKSPACE_SESSION, "mouse")
            .unwrap(),
        "on"
    );

    // First open splits a tagged viewer pane; second open reuses it.
    let p1 = workspace::open_viewer(&t, "cortado_t_a_1", "t/a").unwrap();
    let p2 = workspace::open_viewer(&t, "cortado_t_a_1", "t/a").unwrap();
    assert_eq!(p1, p2, "same agent must not get a duplicate viewer pane");
    let panes = t.list_panes(workspace::WORKSPACE_SESSION).unwrap();
    assert_eq!(panes.len(), 2, "rail + one viewer, got {panes:?}");

    // Killing the workspace never touches the agent session.
    t.kill(workspace::WORKSPACE_SESSION).unwrap();
    assert!(t.has("cortado_t_a_1").unwrap());

    t.kill_server().ok();
}

#[test]
fn viewer_pane_attaches_nested_client() {
    let t = scratch("wsnest");
    let tmp = tempfile::tempdir().unwrap();
    t.spawn("cortado_t_a_1", tmp.path(), &BTreeMap::new(), &sleep_cmd())
        .unwrap();
    workspace::ensure_workspace(&t, tmp.path(), &sleep_cmd()).unwrap();
    workspace::open_viewer(&t, "cortado_t_a_1", "t/a").unwrap();

    // The viewer's inner `tmux attach` becomes a client of the agent session.
    // Poll briefly: the pane process needs a moment to start.
    let mut attached = false;
    for _ in 0..20 {
        if t.has_client("cortado_t_a_1").unwrap() {
            attached = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(attached, "viewer pane never attached to the agent session");

    t.kill_server().ok();
}
