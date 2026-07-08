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
    assert_eq!(
        t.list().unwrap(),
        vec!["damon_newsletter_scout_1".to_string()]
    );
    t.kill("damon_newsletter_scout_1").unwrap();
    assert!(!t.has("damon_newsletter_scout_1").unwrap());
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
        "damon_newsletter_scout_1",
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
            "damon_newsletter_scout_1",
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

    t.kill("damon_newsletter_scout_1").ok();
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
