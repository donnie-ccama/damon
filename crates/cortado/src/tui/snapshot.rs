//! World state, re-derived from scratch every refresh. Holds no UI state.
use cortado_core::models::ModelsFile;
use cortado_core::session_name::SessionName;
use cortado_core::slug::Slug;
use cortado_core::store::{Store, StrayDir};
use cortado_core::CoreError;
use cortado_tmux::Tmux;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LiveSession {
    pub name: String,
    pub created_unix: i64,
    pub model: Option<String>,
}

#[derive(Debug)]
pub struct SessionRow {
    pub name: String,
    pub n: u32,
    pub created_unix: i64,
    pub model: String,
}

#[derive(Debug)]
pub struct MemFile {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct AgentRow {
    pub slug: Slug,
    /// Display name, or the agent.toml error (rendered INVALID).
    pub display: Result<String, String>,
    pub sessions: Vec<SessionRow>,
    pub memory: Vec<MemFile>,
}

#[derive(Debug)]
pub struct TeamRow {
    pub slug: Slug,
    pub display: Result<String, String>,
    pub agents: Vec<AgentRow>,
}

#[derive(Debug)]
pub struct Snapshot {
    pub teams: Vec<TeamRow>,
    pub strays: Vec<StrayDir>,
    /// (registry key, label), models.toml order.
    pub models: Vec<(String, String)>,
}

impl Snapshot {
    pub fn build(
        store: &Store,
        live: &[LiveSession],
        models: &ModelsFile,
    ) -> Result<Snapshot, CoreError> {
        let mut teams = Vec::new();
        for t in store.teams()? {
            let mut agents = Vec::new();
            for a in store.agents(&t.slug)? {
                let mut sessions: Vec<SessionRow> = live
                    .iter()
                    .filter_map(|s| {
                        let parsed = SessionName::parse(&s.name)?;
                        (parsed.team == a.team && parsed.agent == a.slug).then(|| SessionRow {
                            name: s.name.clone(),
                            n: parsed.n,
                            created_unix: s.created_unix,
                            model: s.model.clone().unwrap_or_else(|| "?".into()),
                        })
                    })
                    .collect();
                sessions.sort_by_key(|s| s.n);
                agents.push(AgentRow {
                    // memory computed first: a.slug moves below.
                    memory: memory_files(&store.memory_dir(&a.team, &a.slug)),
                    display: match &a.agent {
                        Ok(f) => Ok(f.agent.name.clone()),
                        Err(e) => Err(e.clone()),
                    },
                    slug: a.slug,
                    sessions,
                });
            }
            teams.push(TeamRow {
                display: match &t.team {
                    Ok(f) => Ok(f.name.clone()),
                    Err(e) => Err(e.clone()),
                },
                slug: t.slug,
                agents,
            });
        }
        Ok(Snapshot {
            teams,
            strays: store.strays()?,
            models: models
                .models
                .iter()
                .map(|(k, m)| (k.clone(), m.label.clone()))
                .collect(),
        })
    }

    pub fn agent(&self, team: &Slug, agent: &Slug) -> Option<&AgentRow> {
        self.teams
            .iter()
            .find(|t| &t.slug == team)?
            .agents
            .iter()
            .find(|a| &a.slug == agent)
    }
}

/// One LiveSession per tmux session. The model comes from the single
/// list_info call (`@cortado_model` user option) — no per-session tmux call.
pub fn live_sessions(tmux: &Tmux) -> Result<Vec<LiveSession>, cortado_tmux::TmuxError> {
    Ok(tmux
        .list_info()?
        .into_iter()
        .map(|info| LiveSession {
            name: info.name,
            created_unix: info.created_unix,
            model: info.model,
        })
        .collect())
}

/// AGENT/USER/MEMORY plus skills/*/SKILL.md, stable order.
fn memory_files(dir: &Path) -> Vec<MemFile> {
    let mut out = Vec::new();
    for f in ["AGENT.md", "USER.md", "MEMORY.md"] {
        let path = dir.join(f);
        if path.is_file() {
            out.push(MemFile {
                label: f.to_string(),
                path,
            });
        }
    }
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("skills")) {
        for entry in entries.flatten() {
            let skill = entry.path().join("SKILL.md");
            if skill.is_file() {
                skills.push(MemFile {
                    label: format!("skills/{}/SKILL.md", entry.file_name().to_string_lossy()),
                    path: skill,
                });
            }
        }
    }
    skills.sort_by(|a, b| a.label.cmp(&b.label));
    out.extend(skills);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortado_core::models::ModelsFile;
    use cortado_core::slug::Slug;
    use cortado_core::store::Store;

    fn s(x: &str) -> Slug {
        Slug::parse(x).unwrap()
    }

    fn fixture() -> (tempfile::TempDir, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::new(tmp.path().to_path_buf());
        store.create_team("Newsletter").unwrap();
        let dir = store.agent_dir(&s("newsletter"), &s("scout"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("agent.toml"),
            "[agent]\nname = \"Scout\"\nruntime = \"claude\"\ndefault_model = \"claude\"\n[repo]\nsource = \"new\"\nbranch = \"agent/scout\"\n",
        )
        .unwrap();
        cortado_core::memory::scaffold_memory(
            &store.memory_dir(&s("newsletter"), &s("scout")),
            "Scout",
            None,
        )
        .unwrap();
        (tmp, store)
    }

    #[test]
    fn joins_agents_with_their_sessions() {
        let (_tmp, store) = fixture();
        let live = vec![
            LiveSession {
                name: "cortado_newsletter_scout_2".into(),
                created_unix: 100,
                model: Some("kimi".into()),
            },
            LiveSession {
                name: "cortado_newsletter_scout_1".into(),
                created_unix: 50,
                model: None,
            },
            LiveSession {
                name: "cortado_other_agent_1".into(),
                created_unix: 10,
                model: None,
            },
            LiveSession {
                name: "not_a_cortado_session".into(),
                created_unix: 10,
                model: None,
            },
        ];
        let snap = Snapshot::build(&store, &live, &ModelsFile::default()).unwrap();
        assert_eq!(snap.teams.len(), 1);
        let agent = snap.agent(&s("newsletter"), &s("scout")).unwrap();
        assert_eq!(agent.display.as_deref().unwrap(), "Scout");
        // sorted by n; unknown model renders "?"
        assert_eq!(agent.sessions.len(), 2);
        assert_eq!(agent.sessions[0].n, 1);
        assert_eq!(agent.sessions[0].model, "?");
        assert_eq!(agent.sessions[1].model, "kimi");
    }

    #[test]
    fn lists_memory_files_including_skills() {
        let (_tmp, store) = fixture();
        let skills = store
            .memory_dir(&s("newsletter"), &s("scout"))
            .join("skills/research");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("SKILL.md"), "skill").unwrap();
        let snap = Snapshot::build(&store, &[], &ModelsFile::default()).unwrap();
        let agent = snap.agent(&s("newsletter"), &s("scout")).unwrap();
        let labels: Vec<&str> = agent.memory.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "AGENT.md",
                "USER.md",
                "MEMORY.md",
                "skills/research/SKILL.md"
            ]
        );
    }

    #[test]
    fn carries_models_and_invalid_agents() {
        let (_tmp, store) = fixture();
        let bad = store.agent_dir(&s("newsletter"), &s("broken"));
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("agent.toml"), "not [valid").unwrap();
        let snap = Snapshot::build(&store, &[], &ModelsFile::default()).unwrap();
        assert!(snap.models.iter().any(|(k, _)| k == "claude"));
        let broken = snap.agent(&s("newsletter"), &s("broken")).unwrap();
        assert!(broken.display.is_err());
    }

    struct SocketGuard(cortado_tmux::Tmux);
    impl Drop for SocketGuard {
        fn drop(&mut self) {
            self.0.kill_server().ok();
        }
    }

    #[test]
    fn builds_from_a_real_tmux_server() {
        let (_tmp, store) = fixture();
        let tmux = cortado_tmux::Tmux::new(format!("cortado-test-tui-{}", std::process::id()));
        let guard = SocketGuard(tmux);
        let tmux = &guard.0;
        let mut env = std::collections::BTreeMap::new();
        env.insert("CORTADO_MODEL".to_string(), "claude".to_string());
        tmux.spawn(
            "cortado_newsletter_scout_1",
            std::path::Path::new("/tmp"),
            &env,
            &["sleep".to_string(), "30".to_string()],
        )
        .unwrap();
        tmux.set_option("cortado_newsletter_scout_1", "@cortado_model", "claude")
            .unwrap();

        let live = live_sessions(tmux).unwrap();
        let snap = Snapshot::build(&store, &live, &ModelsFile::default()).unwrap();
        let agent = snap.agent(&s("newsletter"), &s("scout")).unwrap();
        assert_eq!(agent.sessions.len(), 1);
        assert_eq!(agent.sessions[0].model, "claude");
        assert!(agent.sessions[0].created_unix > 1_500_000_000);
    }
}
