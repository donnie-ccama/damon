use cortado_core::bridge::write_bridges;
use cortado_core::config::Config;
use cortado_core::entity::RuntimeId;
use cortado_core::models::ModelsFile;
use cortado_core::session_name::SessionName;
use cortado_core::store::Store;
use cortado_herdr::Herdr;
use keyring::Entry;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
struct SessionEvent<'a> {
    ts: chrono::DateTime<chrono::Utc>,
    event: &'a str,
    session: &'a str,
    model: &'a str,
    runtime: &'a str,
}

pub struct OpenOutcome {
    pub session: String,
    pub warnings: Vec<String>,
}

pub fn run(reference: &str, model_key: Option<&str>, new: bool) -> anyhow::Result<()> {
    warn_obsolete_config();
    let out = open_session(reference, model_key, new)?;
    for w in &out.warnings {
        eprintln!("warning: {w}");
    }
    println!("session {}", out.session);
    Ok(())
}

/// One-line nudge when the config file still has retired sections.
pub fn warn_obsolete_config() {
    if let Ok(dir) = Config::config_dir() {
        let stale = cortado_core::config::obsolete_sections(&dir.join("config.toml"));
        if !stale.is_empty() {
            eprintln!(
                "note: config sections [{}] are obsolete since the Herdr substrate swap — remove them from config.toml",
                stale.join("], [")
            );
        }
    }
}

pub fn open_session(
    reference: &str,
    model_key: Option<&str>,
    fresh: bool,
) -> anyhow::Result<OpenOutcome> {
    let mut warnings: Vec<String> = Vec::new();
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

    let herdr = Herdr::new(
        config.herdr.binary.clone(),
        config.herdr.workspace.clone(),
        Config::herdr_session(),
    );
    herdr.ensure_server()?;
    let live = herdr.list()?;
    let mine: Vec<&cortado_herdr::AgentInfo> = live
        .iter()
        .filter(|a| {
            SessionName::parse(&a.name).is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        })
        .collect();

    let session = if !fresh && !mine.is_empty() {
        // most recent = highest n (numeric, not lexical)
        let best = mine
            .iter()
            .max_by_key(|a| SessionName::parse(&a.name).map(|n| n.n).unwrap_or(0))
            .unwrap();
        herdr.focus(&best.name)?;
        best.name.clone()
    } else {
        // Regenerate bridges from canonical memory before every spawn.
        let worktree = store.worktree_dir(&entry.team, &entry.slug);
        let memory = store.memory_dir(&entry.team, &entry.slug);
        let cortado_exe = std::env::current_exe()?.display().to_string();
        let bridges = write_bridges(runtime, &agent.agent.name, &memory, &worktree, &cortado_exe)?;
        warnings.extend(bridges.warnings.iter().cloned());
        let names: Vec<String> = bridges
            .written
            .iter()
            .filter_map(|p| p.strip_prefix(&worktree).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        cortado_git::exclude(
            &worktree,
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;

        let live_names: Vec<String> = live.iter().map(|a| a.name.clone()).collect();
        let name = SessionName::next_free(&entry.team, &entry.slug, &live_names).encode();
        let mut env: BTreeMap<String, String> = model
            .env
            .iter()
            .map(|(k, v)| resolve_model_env(key, k, v))
            .collect::<anyhow::Result<_>>()?;
        env.insert("CORTADO_TEAM".into(), entry.team.to_string());
        env.insert("CORTADO_AGENT".into(), entry.slug.to_string());
        env.insert("CORTADO_MODEL".into(), key.to_string());
        env.insert("CORTADO_SESSION".into(), name.clone());

        let binary = runtime.binary();
        if find_executable(&binary).is_none() {
            let install = match runtime {
                RuntimeId::Opencode if cfg!(target_os = "macos") => {
                    "install it with `brew install anomalyco/tap/opencode`"
                }
                RuntimeId::Opencode => "install OpenCode from https://opencode.ai/docs",
                RuntimeId::Codex => "install Codex, or ensure `codex` is on PATH",
                RuntimeId::Claude => "install Claude Code, or ensure `claude` is on PATH",
            };
            anyhow::bail!(
                "{} runtime executable {binary:?} was not found; {install}, or set CORTADO_BIN_{}",
                runtime_display(runtime),
                runtime.as_str().to_uppercase()
            );
        }
        let mut command = vec![binary];
        // Test seam: extra args for substitute binaries (e.g. sleep 30).
        let args_var = format!("CORTADO_{}_ARGS", runtime.as_str().to_uppercase());
        if let Ok(extra) = std::env::var(&args_var) {
            command.extend(extra.split_whitespace().map(String::from));
        }

        let workspace_id = herdr.ensure_workspace()?;
        let started = match herdr.start(&name, &worktree, &env, &command, &workspace_id, true) {
            Ok(info) => info,
            Err(e) => {
                // Best-effort cleanup of any half-created pane.
                if let Ok(after) = herdr.list() {
                    if let Some(a) = after.iter().find(|a| a.name == name) {
                        herdr.close(&a.pane_id).ok();
                    }
                }
                return Err(e.into());
            }
        };
        debug_assert_eq!(started.name, name);

        let event = SessionEvent {
            ts: chrono::Utc::now(),
            event: "spawn",
            session: &name,
            model: key,
            runtime: runtime.as_str(),
        };
        if let Err(log_err) = append_log(&store, &entry.team, &entry.slug, &event) {
            warnings.push(format!(
                "session created but log append failed: {log_err:#}"
            ));
        }
        name
    };

    Ok(OpenOutcome { session, warnings })
}

fn runtime_display(runtime: RuntimeId) -> &'static str {
    match runtime {
        RuntimeId::Claude => "Claude Code",
        RuntimeId::Codex => "Codex",
        RuntimeId::Opencode => "OpenCode",
    }
}

/// Resolve without executing the program. Herdr can report a successful
/// start before a missing child argv[0] exits, so launch must validate the
/// runtime itself first.
fn find_executable(binary: &str) -> Option<PathBuf> {
    let candidate = Path::new(binary);
    if candidate.components().count() > 1 {
        return is_executable(candidate).then(|| candidate.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(binary))
        .find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
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
    // Test/CI seam and container escape hatch: CORTADO_KEY_<ACCOUNT>.
    let seam = format!(
        "CORTADO_KEY_{}",
        account.to_uppercase().replace(['-', '.'], "_")
    );
    if let Ok(v) = std::env::var(&seam) {
        if !v.is_empty() {
            return Ok((name.to_string(), v));
        }
    }

    let missing = || {
        anyhow::anyhow!(
            "model {model_key:?} needs the {account:?} key for {name:?} — run: cortado key set {account}"
        )
    };

    // Test/CI seam: skip the OS keychain entirely (deterministic missing-key path).
    if std::env::var("CORTADO_NO_KEYRING").is_ok_and(|v| !v.is_empty()) {
        return Err(missing());
    }

    let entry = Entry::new("cortado", account)
        .map_err(|e| anyhow::anyhow!("keyring unavailable for account {account:?}: {e}"))?;
    let password = entry.get_password().map_err(|_| missing())?;
    Ok((name.to_string(), password))
}

fn append_log(
    store: &Store,
    team: &cortado_core::slug::Slug,
    agent: &cortado_core::slug::Slug,
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
