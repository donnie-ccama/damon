use cortado_core::config::{expand_tilde, Config};
use cortado_core::entity::{AgentFile, AgentSection, RepoSection, RepoSource, RuntimeId};
use cortado_core::memory::scaffold_memory;
use cortado_core::slug::Slug;
use cortado_core::store::Store;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
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
    let branch_msg = branch.clone();
    let (team, slug) = create(reference, runtime, role, repo, branch)?;
    let branch = branch_msg.unwrap_or_else(|| format!("agent/{slug}"));
    println!("created agent {team}/{slug} (branch {branch})");
    Ok(())
}

pub fn create(
    reference: &str,
    runtime: RuntimeId,
    role: Option<String>,
    repo: RepoArg,
    branch: Option<String>,
) -> anyhow::Result<(Slug, Slug)> {
    let store = store()?;
    let (team_raw, name) = reference
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("agent reference must be team/<Name>"))?;
    let team = Slug::parse(team_raw).map_err(|e| anyhow::anyhow!("{e}"))?;
    if !store.team_dir(&team).exists() {
        anyhow::bail!("no such team: {team} (create it with: cortado team new)");
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
            // These strings are models.toml registry keys, not display names.
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
        RepoArg::New => cortado_git::init_new(&worktree, &branch),
        RepoArg::Clone(u) => cortado_git::clone_repo(u, &worktree, &branch),
        RepoArg::Worktree(p) => match expand_tilde(p) {
            Ok(project) => cortado_git::worktree_add(&project, &worktree, &branch),
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

    Ok((team, slug))
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
    let mut worktree_source: Option<String> = None;
    if let Ok(file) = &entry.agent {
        if file.repo.source == RepoSource::Worktree {
            if let Some(project) = &file.repo.path {
                worktree_source = Some(project.clone());
                let wt = store.worktree_dir(&entry.team, &entry.slug);
                match expand_tilde(project) {
                    Ok(project_dir) => {
                        if let Err(e) = cortado_git::worktree_remove(&project_dir, &wt) {
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
    // The agent is out of the store now, so the scan below sees only survivors.
    if let Some(project) = worktree_source {
        cleanup_exclude(&store, &project);
    }
    println!("removed agent {}/{}", entry.team, entry.slug);
    Ok(())
}

/// expand-tilde -> git common dir -> canonicalized, or None if any step fails.
fn canonical_common_dir(path: &str) -> Option<PathBuf> {
    expand_tilde(path)
        .ok()
        .and_then(|p| cortado_git::common_dir(&p).ok())
        .and_then(|c| c.canonicalize().ok())
}

/// Drop cortado's info/exclude block once the last worktree agent for a repo is
/// gone. Warn-and-continue throughout: cleanup must never block removal.
fn cleanup_exclude(store: &Store, project: &str) {
    let Some(target) = canonical_common_dir(project) else {
        eprintln!(
            "warning: cannot resolve {project:?} for info/exclude cleanup; \
             remove the cortado block manually if it lingers"
        );
        return;
    };
    let still_used = store.all_agents().unwrap_or_default().iter().any(|a| {
        match a.agent.as_ref() {
            // Unreadable agent.toml: we cannot rule this agent out, so treat
            // it as still using the repo (stale block beats broken exclusions).
            Err(_) => true,
            Ok(f) => {
                f.repo.source == RepoSource::Worktree
                    && f.repo
                        .path
                        .as_deref()
                        .and_then(canonical_common_dir)
                        .is_some_and(|c| c == target)
            }
        }
    });
    if still_used {
        return;
    }
    if let Err(e) = cortado_git::exclude_remove(&target) {
        eprintln!("warning: could not clean info/exclude ({e}); remove the cortado block manually");
    }
}
