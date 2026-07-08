use damon_core::config::Config;
use damon_core::models::DEFAULT_MODELS_TOML;

pub fn run() -> anyhow::Result<()> {
    let config = Config::load()?;
    let root = config.root()?;
    std::fs::create_dir_all(root.join("teams"))?;

    let cfg_dir = Config::config_dir()?;
    std::fs::create_dir_all(&cfg_dir)?;
    for (file, content) in [
        ("config.toml", Config::default_toml()),
        ("models.toml", DEFAULT_MODELS_TOML.to_string()),
    ] {
        let path = cfg_dir.join(file);
        if !path.exists() {
            std::fs::write(&path, content)?;
            println!("wrote {}", path.display());
        } else {
            println!("kept  {} (exists)", path.display());
        }
    }
    println!("root  {}", root.display());
    Ok(())
}
