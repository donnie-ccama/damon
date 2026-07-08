use crate::CoreError;
use chrono::{DateTime, Utc};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TeamFile {
    pub name: String,
    pub created: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentFile {
    pub agent: AgentSection,
    pub repo: RepoSection,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentSection {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub runtime: RuntimeId,
    pub default_model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeId {
    Claude,
    Codex,
    Opencode,
}

impl RuntimeId {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeId::Claude => "claude",
            RuntimeId::Codex => "codex",
            RuntimeId::Opencode => "opencode",
        }
    }

    /// CLI binary; overridable for tests, e.g. DAMON_BIN_CLAUDE=sleep.
    pub fn binary(&self) -> String {
        let var = format!("DAMON_BIN_{}", self.as_str().to_uppercase());
        std::env::var(var).unwrap_or_else(|_| self.as_str().to_string())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RepoSection {
    pub source: RepoSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub branch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoSource {
    New,
    Clone,
    Worktree,
}

impl AgentFile {
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.repo.source {
            RepoSource::Clone if self.repo.url.is_none() => Err(CoreError::Invalid(
                "repo.source = \"clone\" requires repo.url".into(),
            )),
            RepoSource::Worktree if self.repo.path.is_none() => Err(CoreError::Invalid(
                "repo.source = \"worktree\" requires repo.path".into(),
            )),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_toml(repo: &str) -> String {
        format!(
            "[agent]\nname = \"Scout\"\nruntime = \"claude\"\ndefault_model = \"claude\"\n\n[repo]\n{repo}\nbranch = \"agent/scout\"\n"
        )
    }

    #[test]
    fn parses_full_agent_file() {
        let a: AgentFile = toml::from_str(&agent_toml(
            "source = \"worktree\"\npath = \"~/Projects/site\"",
        ))
        .unwrap();
        assert_eq!(a.agent.name, "Scout");
        assert_eq!(a.agent.runtime, RuntimeId::Claude);
        assert_eq!(a.repo.source, RepoSource::Worktree);
        a.validate().unwrap();
    }

    #[test]
    fn validate_checks_source_specific_keys() {
        let clone_no_url: AgentFile = toml::from_str(&agent_toml("source = \"clone\"")).unwrap();
        assert!(clone_no_url.validate().is_err());
        let wt_no_path: AgentFile = toml::from_str(&agent_toml("source = \"worktree\"")).unwrap();
        assert!(wt_no_path.validate().is_err());
        let new_ok: AgentFile = toml::from_str(&agent_toml("source = \"new\"")).unwrap();
        new_ok.validate().unwrap();
    }

    #[test]
    fn rejects_unknown_runtime() {
        assert!(toml::from_str::<AgentFile>(
            &agent_toml("source = \"new\"").replace("claude\"\ndefault", "cursor\"\ndefault")
        )
        .is_err());
    }

    #[test]
    fn runtime_binary_env_seam() {
        assert_eq!(RuntimeId::Claude.binary(), "claude");
        std::env::set_var("DAMON_BIN_CLAUDE", "sleep");
        assert_eq!(RuntimeId::Claude.binary(), "sleep");
        std::env::remove_var("DAMON_BIN_CLAUDE");
    }

    #[test]
    fn team_file_round_trips() {
        let t = TeamFile {
            name: "Newsletter".into(),
            created: chrono::Utc::now(),
        };
        let back: TeamFile = toml::from_str(&toml::to_string(&t).unwrap()).unwrap();
        assert_eq!(back.name, "Newsletter");
    }
}
