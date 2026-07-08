use assert_cmd::Command;
use predicates::str::contains;

fn damon() -> Command {
    Command::cargo_bin("damon").unwrap()
}

#[test]
fn key_set_rejects_empty_input() {
    damon()
        .args(["key", "set", "openrouter"])
        .write_stdin("\n")
        .assert()
        .failure()
        .stderr(contains("empty"));
}

#[test]
#[ignore = "touches the real OS keyring; run manually: cargo test -- --ignored"]
fn key_set_get_rm_round_trip_real_keyring() {
    damon().args(["key", "set", "damon-selftest"]).write_stdin("v1\n").assert().success();
    damon().args(["key", "rm", "damon-selftest"]).assert().success();
}
