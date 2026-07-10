use assert_cmd::Command;
use predicates::str::contains;

fn cortado() -> Command {
    Command::cargo_bin("cortado").unwrap()
}

#[test]
fn key_set_rejects_empty_input() {
    cortado()
        .env_remove("CORTADO_NO_KEYRING")
        .args(["key", "set", "openrouter"])
        .write_stdin("\n")
        .assert()
        .failure()
        .stderr(contains("empty"));
}

#[test]
fn key_commands_respect_no_keyring_seam() {
    cortado()
        .env("CORTADO_NO_KEYRING", "1")
        .args(["key", "rm", "openrouter"])
        .assert()
        .failure()
        .stderr(contains("CORTADO_NO_KEYRING"));
    cortado()
        .env("CORTADO_NO_KEYRING", "1")
        .args(["key", "set", "openrouter"])
        .write_stdin("x\n")
        .assert()
        .failure()
        .stderr(contains("CORTADO_NO_KEYRING"));
}

#[test]
#[ignore = "touches the real OS keyring; run manually: cargo test -- --ignored"]
fn key_set_get_rm_round_trip_real_keyring() {
    cortado()
        .args(["key", "set", "cortado-selftest"])
        .write_stdin("v1\n")
        .assert()
        .success();
    cortado()
        .args(["key", "rm", "cortado-selftest"])
        .assert()
        .success();
    // verify second rm fails with friendly error
    cortado()
        .args(["key", "rm", "cortado-selftest"])
        .assert()
        .failure()
        .stderr(contains("no key stored for cortado-selftest"));
}
