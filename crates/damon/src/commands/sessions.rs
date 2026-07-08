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

pub struct KillOutcome {
    pub killed: Vec<String>,
    pub failed: Vec<String>,
}

/// Kill every live session of team/agent (or unique bare slug).
pub fn kill_agent(reference: &str) -> anyhow::Result<KillOutcome> {
    let config = Config::load()?;
    let tmux = Tmux::new(config.tmux.socket.clone());
    let store = Store::new(config.root()?);
    let entry = store.resolve(reference)?;
    let mut out = KillOutcome {
        killed: Vec::new(),
        failed: Vec::new(),
    };
    for name in tmux.list()? {
        if SessionName::parse(&name).is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        {
            match tmux.kill(&name) {
                Ok(()) => out.killed.push(name),
                Err(e) => out.failed.push(format!("{name}: {e}")),
            }
        }
    }
    Ok(out)
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
    let out = kill_agent(target)?;
    for name in &out.killed {
        println!("killed {name}");
    }
    if !out.failed.is_empty() {
        anyhow::bail!(
            "killed {}, failed {}: {}",
            out.killed.len(),
            out.failed.len(),
            out.failed.join("; ")
        );
    }
    if out.killed.is_empty() {
        println!("no live sessions for {target}");
    }
    Ok(())
}
