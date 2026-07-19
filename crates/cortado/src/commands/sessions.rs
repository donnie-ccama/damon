use cortado_core::config::Config;
use cortado_core::session_log::models_for;
use cortado_core::session_name::SessionName;
use cortado_core::store::Store;
use cortado_herdr::Herdr;

fn herdr(config: &Config) -> Herdr {
    Herdr::new(
        config.herdr.binary.clone(),
        config.herdr.workspace.clone(),
        Config::herdr_session(),
    )
}

pub fn ls() -> anyhow::Result<()> {
    let config = Config::load()?;
    let store = Store::new(config.root()?);
    let live = herdr(&config).list()?;
    let names: Vec<String> = live.iter().map(|a| a.name.clone()).collect();
    let models = models_for(&store, &names);
    for a in &live {
        if let Some(parsed) = SessionName::parse(&a.name) {
            println!(
                "{:<40} {}/{:<20} {:<8} {}",
                a.name,
                parsed.team,
                parsed.agent,
                a.status,
                models.get(&a.name).map(String::as_str).unwrap_or("?"),
            );
        }
    }
    Ok(())
}

pub struct KillOutcome {
    pub killed: Vec<String>,
    pub failed: Vec<String>,
}

/// Close every live pane of team/agent (or unique bare slug).
pub fn kill_agent(reference: &str) -> anyhow::Result<KillOutcome> {
    let config = Config::load()?;
    let h = herdr(&config);
    let store = Store::new(config.root()?);
    let entry = store.resolve(reference)?;
    let mut out = KillOutcome {
        killed: Vec::new(),
        failed: Vec::new(),
    };
    for a in h.list()? {
        if SessionName::parse(&a.name)
            .is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        {
            match h.close(&a.pane_id) {
                Ok(()) => out.killed.push(a.name),
                Err(e) => out.failed.push(format!("{}: {e}", a.name)),
            }
        }
    }
    Ok(out)
}

/// Close one session by exact name, or every session of team/agent | bare slug.
pub fn kill(target: &str) -> anyhow::Result<()> {
    let config = Config::load()?;
    let h = herdr(&config);
    if SessionName::parse(target).is_some() {
        let live = h.list()?;
        let Some(a) = live.iter().find(|a| a.name == target) else {
            println!("no live session named {target}");
            return Ok(());
        };
        h.close(&a.pane_id)?;
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
