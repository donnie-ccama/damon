use damon_core::config::{expand_tilde, Config};
use damon_core::entity::{AgentFile, AgentSection, RepoSection, RepoSource, RuntimeId};
use damon_core::memory::scaffold_memory;
use damon_core::slug::Slug;
use damon_core::store::Store;

pub enum RepoArg {
    New,
    Clone(String),
    Worktree(String),
}

fn store() -> anyhow::Result<Store> {
    Ok(Store::new(Config::load()?.root()?))
}

pub fn new(
    reference: &str,
    runtime: RuntimeId,
    role: Option<String>,
    repo: RepoArg,
    branch: Option<String>,
) -> anyhow::Result<()> {
    let store = store()?;
    let (team_raw, name) = reference
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("agent reference must be team/<Name>"))?;
    let team = Slug::parse(team_raw).map_err(|e| anyhow::anyhow!("{e}"))?;
    if !store.team_dir(&team).exists() {
        anyhow::bail!("no such team: {team} (create it with: damon team new)");
    }
    let slug = Slug::derive(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dir = store.agent_dir(&team, &slug);
    if dir.exists() {
        anyhow::bail!("agent {team}/{slug} already exists");
    }

    let branch = branch.unwrap_or_else(|| format!("agent/{slug}"));
    let (source, url, path) = match &repo {
        RepoArg::New => (RepoSource::New, None, None),
        RepoArg::Clone(u) => (RepoSource::Clone, Some(u.clone()), None),
        RepoArg::Worktree(p) => (RepoSource::Worktree, None, Some(p.clone())),
    };
    let file = AgentFile {
        agent: AgentSection {
            name: name.to_string(),
            role: role.clone(),
            runtime,
            default_model: match runtime {
                RuntimeId::Claude => "claude",
                RuntimeId::Codex => "gpt",
                RuntimeId::Opencode => "opencode",
            }
            .to_string(),
        },
        repo: RepoSection {
            source,
            url,
            path,
            branch: branch.clone(),
        },
    };
    file.validate()?;

    // Directories + memory first; repo last so a git failure rolls back cleanly.
    std::fs::create_dir_all(store.logs_dir(&team, &slug))?;
    scaffold_memory(&store.memory_dir(&team, &slug), name, role.as_deref())?;
    std::fs::write(dir.join("agent.toml"), toml::to_string_pretty(&file)?)?;

    let worktree = store.worktree_dir(&team, &slug);
    let repo_result = match &repo {
        RepoArg::New => damon_git::init_new(&worktree, &branch),
        RepoArg::Clone(u) => damon_git::clone_repo(u, &worktree, &branch),
        RepoArg::Worktree(p) => match expand_tilde(p) {
            Ok(project) => damon_git::worktree_add(&project, &worktree, &branch),
            Err(e) => {
                std::fs::remove_dir_all(&dir).ok();
                anyhow::bail!("cannot resolve repo path {p:?}: {e}");
            }
        },
    };
    if let Err(e) = repo_result {
        let cleanup = std::fs::remove_dir_all(&dir);
        return Err(match cleanup {
            Ok(()) => anyhow::anyhow!("repo setup failed, rolled back agent dir: {e}"),
            Err(rm) => anyhow::anyhow!(
                "repo setup failed: {e}; cleanup of {} also failed ({rm}) — remove it manually",
                dir.display()
            ),
        });
    }

    println!("created agent {team}/{slug} (branch {branch})");
    Ok(())
}

pub fn ls(team: Option<&str>) -> anyhow::Result<()> {
    let store = store()?;
    let agents = match team {
        Some(t) => store.agents(&Slug::parse(t).map_err(|e| anyhow::anyhow!("{e}"))?)?,
        None => store.all_agents()?,
    };
    for a in agents {
        match &a.agent {
            Ok(f) => println!(
                "{}/{:<20} {:<9} {}",
                a.team,
                a.slug.as_str(),
                f.agent.runtime.as_str(),
                f.agent.role.as_deref().unwrap_or("-")
            ),
            Err(e) => println!("{}/{:<20} INVALID: {e}", a.team, a.slug.as_str()),
        }
    }
    Ok(())
}

pub fn rm(reference: &str, yes: bool) -> anyhow::Result<()> {
    if !yes {
        anyhow::bail!("agent rm deletes the worktree and memory; re-run with --yes");
    }
    let store = store()?;
    let entry = store.resolve(reference)?;
    if let Ok(file) = &entry.agent {
        if file.repo.source == RepoSource::Worktree {
            if let Some(project) = &file.repo.path {
                let wt = store.worktree_dir(&entry.team, &entry.slug);
                match expand_tilde(project) {
                    Ok(project_dir) => {
                        if let Err(e) = damon_git::worktree_remove(&project_dir, &wt) {
                            eprintln!(
                                "warning: git worktree remove failed ({e}); deleting directory anyway"
                            );
                        }
                    }
                    Err(e) => eprintln!(
                        "warning: cannot resolve source repo path ({e}); deleting directory anyway"
                    ),
                }
            }
        }
    }
    std::fs::remove_dir_all(&entry.dir)?;
    println!("removed agent {}/{}", entry.team, entry.slug);
    Ok(())
}
