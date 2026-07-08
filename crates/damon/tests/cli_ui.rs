use assert_cmd::Command;

#[test]
fn ui_without_a_tty_fails_with_a_clear_message() {
    // assert_cmd pipes stdout, so is_terminal() is false in the child.
    Command::cargo_bin("damon")
        .unwrap()
        .arg("ui")
        .assert()
        .failure()
        .stderr(predicates::str::contains("interactive terminal"));
}
