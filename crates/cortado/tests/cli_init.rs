use assert_cmd::Command;

fn cortado(root: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("cortado").unwrap();
    cmd.env("CORTADO_ROOT", root).env("CORTADO_CONFIG_DIR", cfg);
    cmd
}

#[test]
fn init_scaffolds_root_and_config() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    cortado(root.path(), cfg.path())
        .arg("init")
        .assert()
        .success();
    assert!(root.path().join("teams").is_dir());
    assert!(cfg.path().join("config.toml").exists());
    let models = std::fs::read_to_string(cfg.path().join("models.toml")).unwrap();
    assert!(models.contains("[models.kimi]"));
}

#[test]
fn init_never_overwrites_user_config() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(
        cfg.path().join("models.toml"),
        "# mine\n[models.custom]\nlabel = \"X\"\nruntime = \"claude\"\n",
    )
    .unwrap();
    cortado(root.path(), cfg.path())
        .arg("init")
        .assert()
        .success();
    let models = std::fs::read_to_string(cfg.path().join("models.toml")).unwrap();
    assert!(models.contains("# mine"));
    assert!(!models.contains("[models.kimi]"));
}
