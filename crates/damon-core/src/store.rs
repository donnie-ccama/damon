use crate::entity::{AgentFile, TeamFile};
use crate::slug::Slug;
use crate::CoreError;
use std::path::PathBuf;

pub struct Store {
    root: PathBuf,
}

#[derive(Debug)]
pub struct TeamEntry {
    pub slug: Slug,
    pub dir: PathBuf,
    pub team: Result<TeamFile, String>,
}

#[derive(Debug)]
pub struct AgentEntry {
    pub team: Slug,
    pub slug: Slug,
    pub dir: PathBuf,
    pub agent: Result<AgentFile, String>,
}

impl Store {
    pub fn new(root: PathBuf) -> Store {
        Store { root }
    }

    pub fn team_dir(&self, team: &Slug) -> PathBuf {
        self.root.join("teams").join(team.as_str())
    }
    pub fn agent_dir(&self, team: &Slug, agent: &Slug) -> PathBuf {
        self.team_dir(team).join("agents").join(agent.as_str())
    }
    pub fn worktree_dir(&self, team: &Slug, agent: &Slug) -> PathBuf {
        self.agent_dir(team, agent).join("worktree")
    }
    pub fn memory_dir(&self, team: &Slug, agent: &Slug) -> PathBuf {
        self.agent_dir(team, agent).join("memory")
    }
    pub fn logs_dir(&self, team: &Slug, agent: &Slug) -> PathBuf {
        self.agent_dir(team, agent).join("logs")
    }

    pub fn create_team(&self, name: &str) -> Result<Slug, CoreError> {
        let slug = Slug::derive(name).map_err(|e| CoreError::Invalid(e.to_string()))?;
        let dir = self.team_dir(&slug);
        if dir.exists() {
            return Err(CoreError::Exists(format!("team {slug}")));
        }
        std::fs::create_dir_all(dir.join("agents"))
            .map_err(|e| CoreError::Io { path: dir.clone(), source: e })?;
        let team = TeamFile { name: name.to_string(), created: chrono::Utc::now() };
        let text = toml::to_string_pretty(&team)
            .map_err(|e| CoreError::Invalid(e.to_string()))?;
        std::fs::write(dir.join("team.toml"), text)
            .map_err(|e| CoreError::Io { path: dir.join("team.toml"), source: e })?;
        Ok(slug)
    }

    pub fn teams(&self) -> Result<Vec<TeamEntry>, CoreError> {
        let mut out = Vec::new();
        for (slug, dir) in slug_dirs(&self.root.join("teams"))? {
            let team = read_toml::<TeamFile>(&dir.join("team.toml"));
            out.push(TeamEntry { slug, dir, team });
        }
        Ok(out)
    }

    pub fn agents(&self, team: &Slug) -> Result<Vec<AgentEntry>, CoreError> {
        let mut out = Vec::new();
        for (slug, dir) in slug_dirs(&self.team_dir(team).join("agents"))? {
            let agent = read_toml::<AgentFile>(&dir.join("agent.toml"));
            out.push(AgentEntry { team: team.clone(), slug, dir, agent });
        }
        Ok(out)
    }

    pub fn all_agents(&self) -> Result<Vec<AgentEntry>, CoreError> {
        let mut out = Vec::new();
        for t in self.teams()? {
            out.extend(self.agents(&t.slug)?);
        }
        Ok(out)
    }

    /// Accepts "team/agent" or a bare agent slug (must be unique across teams).
    pub fn resolve(&self, reference: &str) -> Result<AgentEntry, CoreError> {
        if let Some((team, agent)) = reference.split_once('/') {
            let team = Slug::parse(team).map_err(|e| CoreError::Invalid(e.to_string()))?;
            let agent = Slug::parse(agent).map_err(|e| CoreError::Invalid(e.to_string()))?;
            return self
                .agents(&team)?
                .into_iter()
                .find(|a| a.slug == agent)
                .ok_or_else(|| CoreError::NotFound(format!("agent {reference}")));
        }
        let slug = Slug::parse(reference).map_err(|e| CoreError::Invalid(e.to_string()))?;
        let matches: Vec<AgentEntry> = self
            .all_agents()?
            .into_iter()
            .filter(|a| a.slug == slug)
            .collect();
        match matches.len() {
            0 => Err(CoreError::NotFound(format!("agent {reference}"))),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let teams: Vec<String> =
                    matches.iter().map(|a| format!("{}/{}", a.team, a.slug)).collect();
                Err(CoreError::Ambiguous(reference.to_string(), teams.join(", ")))
            }
        }
    }
}

fn slug_dirs(parent: &std::path::Path) -> Result<Vec<(Slug, PathBuf)>, CoreError> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(CoreError::Io { path: parent.to_path_buf(), source: e }),
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        if let Ok(slug) = Slug::parse(&entry.file_name().to_string_lossy()) {
            out.push((slug, entry.path()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slug::Slug;

    fn fixture() -> (tempfile::TempDir, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::new(tmp.path().to_path_buf());
        store.create_team("Newsletter").unwrap();
        store.create_team("Church").unwrap();
        // hand-write a valid agent (files are the database)
        let dir = store.agent_dir(&Slug::parse("newsletter").unwrap(), &Slug::parse("scout").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("agent.toml"),
            "[agent]\nname = \"Scout\"\nruntime = \"claude\"\ndefault_model = \"claude\"\n[repo]\nsource = \"new\"\nbranch = \"agent/scout\"\n",
        )
        .unwrap();
        (tmp, store)
    }

    #[test]
    fn create_team_writes_team_toml_and_rejects_dupes() {
        let (_tmp, store) = fixture();
        let teams = store.teams().unwrap();
        assert_eq!(teams.len(), 2);
        assert!(teams.iter().any(|t| t.slug.as_str() == "newsletter" && t.team.is_ok()));
        assert!(matches!(store.create_team("Newsletter"), Err(CoreError::Exists(_))));
    }

    #[test]
    fn discovers_agents_and_reports_invalid_toml() {
        let (_tmp, store) = fixture();
        let bad = store.agent_dir(&Slug::parse("church").unwrap(), &Slug::parse("broken").unwrap());
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("agent.toml"), "not [valid").unwrap();
        let all = store.all_agents().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|a| a.slug.as_str() == "broken" && a.agent.is_err()));
    }

    #[test]
    fn resolve_by_path_and_bare_slug() {
        let (_tmp, store) = fixture();
        assert_eq!(store.resolve("newsletter/scout").unwrap().slug.as_str(), "scout");
        assert_eq!(store.resolve("scout").unwrap().team.as_str(), "newsletter");
        assert!(matches!(store.resolve("nope"), Err(CoreError::NotFound(_))));
    }

    #[test]
    fn resolve_rejects_ambiguous_bare_slug() {
        let (_tmp, store) = fixture();
        let dup = store.agent_dir(&Slug::parse("church").unwrap(), &Slug::parse("scout").unwrap());
        std::fs::create_dir_all(&dup).unwrap();
        std::fs::write(
            dup.join("agent.toml"),
            "[agent]\nname = \"Scout\"\nruntime = \"claude\"\ndefault_model = \"claude\"\n[repo]\nsource = \"new\"\nbranch = \"agent/scout\"\n",
        )
        .unwrap();
        assert!(matches!(store.resolve("scout"), Err(CoreError::Ambiguous(_, _))));
    }

    #[test]
    fn agent_dir_without_toml_reports_invalid() {
        let (_tmp, store) = fixture();
        let empty = store.agent_dir(&Slug::parse("church").unwrap(), &Slug::parse("hollow").unwrap());
        std::fs::create_dir_all(&empty).unwrap();
        let all = store.all_agents().unwrap();
        let hollow = all.iter().find(|a| a.slug.as_str() == "hollow").expect("hollow listed");
        assert!(hollow.agent.is_err());
    }
}
