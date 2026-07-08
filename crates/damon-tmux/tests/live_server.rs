use damon_tmux::Tmux;
use std::collections::BTreeMap;

fn scratch(tag: &str) -> Tmux {
    Tmux::new(format!("damon-test-{}-{}", tag, std::process::id()))
}

#[test]
fn spawn_list_kill_lifecycle() {
    let t = scratch("lifecycle");
    let tmp = tempfile::tempdir().unwrap();
    let mut env = BTreeMap::new();
    env.insert("DAMON_AGENT".to_string(), "scout".to_string());
    t.spawn(
        "damon_newsletter_scout_1",
        tmp.path(),
        &env,
        &["sleep".to_string(), "30".to_string()],
    )
    .unwrap();
    assert!(t.has("damon_newsletter_scout_1").unwrap());
    assert_eq!(t.list().unwrap(), vec!["damon_newsletter_scout_1".to_string()]);
    t.kill("damon_newsletter_scout_1").unwrap();
    assert!(!t.has("damon_newsletter_scout_1").unwrap());
    t.kill_server().ok();
}

#[test]
fn list_without_server_is_empty() {
    let t = scratch("noserver");
    assert!(t.list().unwrap().is_empty());
}

#[test]
fn version_parses() {
    let (major, _minor) = damon_tmux::version().unwrap();
    assert!(major >= 3);
}
