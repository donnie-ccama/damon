use damon_core::bridge::write_bridges;
use damon_core::config::Config;
use damon_core::entity::RuntimeId;
use damon_core::models::ModelsFile;
use damon_core::session_name::SessionName;
use damon_core::store::Store;
use damon_tmux::Tmux;
use std::collections::BTreeMap;

#[derive(serde::Serialize)]
struct SessionEvent<'a> {
    ts: chrono::DateTime<chrono::Utc>,
    event: &'a str,
    session: &'a str,
    model: &'a str,
    runtime: &'a str,
}

pub fn run(reference: &str, model_key: Option<&str>, new: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let store = Store::new(config.root());
    let entry = store.resolve(reference)?;
    let agent = entry
        .agent
        .as_ref()
        .map_err(|e| anyhow::anyhow!("agent.toml invalid: {e}"))?;

    let models = ModelsFile::load()?;
    let key = model_key.unwrap_or(&agent.agent.default_model);
    let model = models
        .get(key)
        .ok_or_else(|| anyhow::anyhow!("unknown model {key:?} (see models.toml)"))?;
    let runtime = match model.runtime.as_str() {
        "claude" => RuntimeId::Claude,
        other => anyhow::bail!("runtime {other:?} not yet supported (M2)"),
    };
    for value in model.env.values() {
        if value.contains("${keyring:") {
            anyhow::bail!("model {key:?} needs a provider key; key management lands in M2");
        }
    }

    let tmux = Tmux::new(config.tmux.socket.clone());
    let live = tmux.list()?;
    let mine: Vec<&String> = live
        .iter()
        .filter(|s| {
            SessionName::parse(s)
                .is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        })
        .collect();

    let session = if !new && !mine.is_empty() {
        mine.iter().max().unwrap().to_string() // most recent = highest n
    } else {
        // Regenerate bridges from canonical memory before every spawn.
        let worktree = store.worktree_dir(&entry.team, &entry.slug);
        let memory = store.memory_dir(&entry.team, &entry.slug);
        let written = write_bridges(runtime, &agent.agent.name, &memory, &worktree)?;
        let names: Vec<String> = written
            .iter()
            .filter_map(|p| p.strip_prefix(&worktree).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        damon_git::exclude(&worktree, &names.iter().map(String::as_str).collect::<Vec<_>>())?;

        let name = SessionName::next_free(&entry.team, &entry.slug, &live).encode();
        let mut env: BTreeMap<String, String> = model.env.clone();
        env.insert("DAMON_TEAM".into(), entry.team.to_string());
        env.insert("DAMON_AGENT".into(), entry.slug.to_string());
        env.insert("DAMON_MODEL".into(), key.to_string());
        env.insert("DAMON_SESSION".into(), name.clone());

        let mut command = vec![runtime.binary()];
        // Test seam: extra args for substitute binaries (e.g. sleep 30).
        if let Ok(extra) = std::env::var("DAMON_CLAUDE_ARGS") {
            command.extend(extra.split_whitespace().map(String::from));
        }
        if let Err(e) = tmux.spawn(&name, &worktree, &env, &command) {
            tmux.kill(&name).ok(); // clean up any half-created session
            return Err(e.into());
        }

        let event = SessionEvent {
            ts: chrono::Utc::now(),
            event: "spawn",
            session: &name,
            model: key,
            runtime: runtime.as_str(),
        };
        append_log(&store, &entry.team, &entry.slug, &event)?;
        name
    };

    println!("session {session}");
    damon_term::launcher_for(config.terminal.launcher, config.tmux.socket.clone())
        .open(&session, &format!("{}/{}", entry.team, entry.slug))?;
    Ok(())
}

fn append_log(
    store: &Store,
    team: &damon_core::slug::Slug,
    agent: &damon_core::slug::Slug,
    event: &SessionEvent,
) -> anyhow::Result<()> {
    use std::io::Write;
    let dir = store.logs_dir(team, agent);
    std::fs::create_dir_all(&dir)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("sessions.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(event)?)?;
    Ok(())
}
