use damon_core::bridge::write_bridges;
use damon_core::config::Config;
use damon_core::entity::RuntimeId;
use damon_core::models::ModelsFile;
use damon_core::session_name::SessionName;
use damon_core::store::Store;
use damon_tmux::Tmux;
use keyring::Entry;
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
    let store = Store::new(config.root()?);
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
        "codex" => RuntimeId::Codex,
        "opencode" => RuntimeId::Opencode,
        other => anyhow::bail!("unknown runtime {other:?} in models.toml"),
    };

    let tmux = Tmux::new(config.tmux.socket.clone());
    let live = tmux.list()?;
    let mine: Vec<&String> = live
        .iter()
        .filter(|s| {
            SessionName::parse(s).is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        })
        .collect();

    let session = if !new && !mine.is_empty() {
        mine.iter()
            .max_by_key(|s| SessionName::parse(s).map(|n| n.n).unwrap_or(0))
            .unwrap()
            .to_string() // most recent = highest n (numeric, not lexical)
    } else {
        // Regenerate bridges from canonical memory before every spawn.
        let worktree = store.worktree_dir(&entry.team, &entry.slug);
        let memory = store.memory_dir(&entry.team, &entry.slug);
        let damon_exe = std::env::current_exe()?.display().to_string();
        let written = write_bridges(runtime, &agent.agent.name, &memory, &worktree, &damon_exe)?;
        let names: Vec<String> = written
            .iter()
            .filter_map(|p| p.strip_prefix(&worktree).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        damon_git::exclude(
            &worktree,
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;

        let name = SessionName::next_free(&entry.team, &entry.slug, &live).encode();
        let mut env: BTreeMap<String, String> = model
            .env
            .iter()
            .map(|(k, v)| resolve_model_env(key, k, v))
            .collect::<anyhow::Result<_>>()?;
        env.insert("DAMON_TEAM".into(), entry.team.to_string());
        env.insert("DAMON_AGENT".into(), entry.slug.to_string());
        env.insert("DAMON_MODEL".into(), key.to_string());
        env.insert("DAMON_SESSION".into(), name.clone());

        let mut command = vec![runtime.binary()];
        // Test seam: extra args for substitute binaries (e.g. sleep 30).
        let args_var = format!("DAMON_{}_ARGS", runtime.as_str().to_uppercase());
        if let Ok(extra) = std::env::var(&args_var) {
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
        if let Err(log_err) = append_log(&store, &entry.team, &entry.slug, &event) {
            eprintln!("warning: session created but log append failed: {log_err:#}");
        }
        name
    };

    println!("session {session}");
    damon_term::launcher_for(config.terminal.launcher, config.tmux.socket.clone())
        .open(&session, &format!("{}/{}", entry.team, entry.slug))?;
    Ok(())
}

fn resolve_model_env(model_key: &str, name: &str, value: &str) -> anyhow::Result<(String, String)> {
    if value.starts_with("${") && value.ends_with('}') && value.len() > 3 {
        let inner = &value[2..value.len() - 1];
        if inner.starts_with("keyring:") {
            let account = inner.strip_prefix("keyring:").unwrap_or("");
            if account.is_empty() {
                anyhow::bail!(
                    "model {model_key:?} has an empty ${{keyring:}} account for {name:?} — fix models.toml"
                );
            }
            return resolve_from_keyring(model_key, name, account);
        }
        let env_value = std::env::var(inner).map_err(|_| {
            anyhow::anyhow!("model {model_key:?} uses unresolved env var ${inner:?} for {name:?}")
        })?;
        return Ok((name.to_string(), env_value));
    }

    Ok((name.to_string(), value.to_string()))
}

fn resolve_from_keyring(
    model_key: &str,
    name: &str,
    account: &str,
) -> anyhow::Result<(String, String)> {
    // Test/CI seam and container escape hatch: DAMON_KEY_<ACCOUNT>.
    let seam = format!(
        "DAMON_KEY_{}",
        account.to_uppercase().replace(['-', '.'], "_")
    );
    if let Ok(v) = std::env::var(&seam) {
        if !v.is_empty() {
            return Ok((name.to_string(), v));
        }
    }

    let missing = || {
        anyhow::anyhow!(
            "model {model_key:?} needs the {account:?} key for {name:?} — run: damon key set {account}"
        )
    };

    // Test/CI seam: skip the OS keychain entirely (deterministic missing-key path).
    if std::env::var("DAMON_NO_KEYRING").is_ok_and(|v| !v.is_empty()) {
        return Err(missing());
    }

    let entry = Entry::new("damon", account)
        .map_err(|e| anyhow::anyhow!("keyring unavailable for account {account:?}: {e}"))?;
    let password = entry.get_password().map_err(|_| missing())?;
    Ok((name.to_string(), password))
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
