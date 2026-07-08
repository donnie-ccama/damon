use damon_core::config::Config;
use damon_core::slug::Slug;
use damon_core::store::Store;

fn store() -> anyhow::Result<Store> {
    Ok(Store::new(Config::load()?.root()))
}

pub fn new(name: &str) -> anyhow::Result<()> {
    let slug = store()?.create_team(name)?;
    println!("created team {slug}");
    Ok(())
}

pub fn ls() -> anyhow::Result<()> {
    for t in store()?.teams()? {
        match &t.team {
            Ok(file) => println!("{:<24} {}", t.slug.as_str(), file.name),
            Err(e) => println!("{:<24} INVALID: {e}", t.slug.as_str()),
        }
    }
    Ok(())
}

pub fn rm(slug: &str, force: bool) -> anyhow::Result<()> {
    let store = store()?;
    let slug = Slug::parse(slug).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dir = store.team_dir(&slug);
    if !dir.exists() {
        anyhow::bail!("no such team: {slug}");
    }
    let agents = store.agents(&slug)?;
    if !agents.is_empty() && !force {
        anyhow::bail!("team {slug} has {} agent(s); pass --force to delete anyway", agents.len());
    }
    std::fs::remove_dir_all(&dir)?;
    println!("removed team {slug}");
    Ok(())
}
