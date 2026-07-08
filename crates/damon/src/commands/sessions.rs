use damon_core::config::Config;
use damon_core::session_name::SessionName;
use damon_core::store::Store;
use damon_tmux::Tmux;

pub fn ls() -> anyhow::Result<()> {
    let config = Config::load()?;
    let tmux = Tmux::new(config.tmux.socket.clone());
    for name in tmux.list()? {
        if let Some(parsed) = SessionName::parse(&name) {
            println!("{:<40} {}/{}", name, parsed.team, parsed.agent);
        }
    }
    Ok(())
}

/// Kill one session by exact name, or every session of team/agent | bare slug.
pub fn kill(target: &str) -> anyhow::Result<()> {
    let config = Config::load()?;
    let tmux = Tmux::new(config.tmux.socket.clone());
    if SessionName::parse(target).is_some() {
        tmux.kill(target)?;
        println!("killed {target}");
        return Ok(());
    }
    let store = Store::new(config.root()?);
    let entry = store.resolve(target)?;
    let mut killed = 0;
    let mut failures: Vec<String> = Vec::new();
    for name in tmux.list()? {
        if SessionName::parse(&name).is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        {
            match tmux.kill(&name) {
                Ok(()) => {
                    println!("killed {name}");
                    killed += 1;
                }
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
    }
    if !failures.is_empty() {
        anyhow::bail!(
            "killed {killed}, failed {}: {}",
            failures.len(),
            failures.join("; ")
        );
    }
    if killed == 0 {
        println!("no live sessions for {}/{}", entry.team, entry.slug);
    }
    Ok(())
}
