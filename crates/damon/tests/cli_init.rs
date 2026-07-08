use assert_cmd::Command;

fn damon(root: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("damon").unwrap();
    cmd.env("DAMON_ROOT", root).env("DAMON_CONFIG_DIR", cfg);
    cmd
}

#[test]
fn init_scaffolds_root_and_config() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    damon(root.path(), cfg.path()).arg("init").assert().success();
    assert!(root.path().join("teams").is_dir());
    assert!(cfg.path().join("config.toml").exists());
    let models = std::fs::read_to_string(cfg.path().join("models.toml")).unwrap();
    assert!(models.contains("[models.kimi]"));
}

#[test]
fn init_never_overwrites_user_config() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(cfg.path().join("models.toml"), "# mine\n[models.custom]\nlabel = \"X\"\nruntime = \"claude\"\n").unwrap();
    damon(root.path(), cfg.path()).arg("init").assert().success();
    let models = std::fs::read_to_string(cfg.path().join("models.toml")).unwrap();
    assert!(models.contains("# mine"));
    assert!(!models.contains("[models.kimi]"));
}
