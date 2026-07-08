use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn hook_reflect_blocks_first_stop_then_allows() {
    let mut first = Command::cargo_bin("damon").unwrap();
    first
        .args(["hook", "reflect"])
        .write_stdin(r#"{"session_id":"s","stop_hook_active":false}"#)
        .assert()
        .code(2)
        .stderr(contains("write-back protocol"));
    let mut second = Command::cargo_bin("damon").unwrap();
    second
        .args(["hook", "reflect"])
        .write_stdin(r#"{"session_id":"s","stop_hook_active":true}"#)
        .assert()
        .success();
}

#[test]
fn hook_reflect_tolerates_garbage_stdin() {
    Command::cargo_bin("damon")
        .unwrap()
        .args(["hook", "reflect"])
        .write_stdin("not json")
        .assert()
        .code(2); // fail toward reflecting once, never toward crashing
}
