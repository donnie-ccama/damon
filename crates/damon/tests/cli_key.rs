use assert_cmd::Command;
use predicates::str::contains;

fn damon() -> Command {
    Command::cargo_bin("damon").unwrap()
}

#[test]
fn key_set_rejects_empty_input() {
    damon()
        .env_remove("DAMON_NO_KEYRING")
        .args(["key", "set", "openrouter"])
        .write_stdin("\n")
        .assert()
        .failure()
        .stderr(contains("empty"));
}

#[test]
fn key_commands_respect_no_keyring_seam() {
    damon()
        .env("DAMON_NO_KEYRING", "1")
        .args(["key", "rm", "openrouter"])
        .assert()
        .failure()
        .stderr(contains("DAMON_NO_KEYRING"));
    damon()
        .env("DAMON_NO_KEYRING", "1")
        .args(["key", "set", "openrouter"])
        .write_stdin("x\n")
        .assert()
        .failure()
        .stderr(contains("DAMON_NO_KEYRING"));
}

#[test]
#[ignore = "touches the real OS keyring; run manually: cargo test -- --ignored"]
fn key_set_get_rm_round_trip_real_keyring() {
    damon()
        .args(["key", "set", "damon-selftest"])
        .write_stdin("v1\n")
        .assert()
        .success();
    damon()
        .args(["key", "rm", "damon-selftest"])
        .assert()
        .success();
    // verify second rm fails with friendly error
    damon()
        .args(["key", "rm", "damon-selftest"])
        .assert()
        .failure()
        .stderr(contains("no key stored for damon-selftest"));
}
