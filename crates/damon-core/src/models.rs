use crate::{config, CoreError};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelsFile {
    pub models: BTreeMap<String, Model>,
}

impl Default for ModelsFile {
    fn default() -> Self {
        toml::from_str(DEFAULT_MODELS_TOML).expect("default models parse")
    }
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Model {
    pub label: String,
    pub runtime: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl ModelsFile {
    pub fn load() -> Result<ModelsFile, CoreError> {
        let path = config::Config::config_dir().join("models.toml");
        config::load_toml_or_default(&path)
    }

    pub fn get(&self, key: &str) -> Option<&Model> {
        self.models.get(key)
    }
}

pub const DEFAULT_MODELS_TOML: &str = r#"[models.claude]
label = "Claude"
runtime = "claude"

[models.gpt]
label = "GPT-5.5"
runtime = "codex"

[models.kimi]
label = "Kimi K2.7"
runtime = "claude"
env = { ANTHROPIC_BASE_URL = "https://openrouter.ai/api/v1", ANTHROPIC_AUTH_TOKEN = "${keyring:openrouter}", ANTHROPIC_MODEL = "moonshotai/kimi-k2.7" }

[models.minimax]
label = "MiniMax M3"
runtime = "claude"
env = { ANTHROPIC_BASE_URL = "https://openrouter.ai/api/v1", ANTHROPIC_AUTH_TOKEN = "${keyring:openrouter}", ANTHROPIC_MODEL = "minimax/minimax-m3" }

[models.glm]
label = "GLM 5.2"
runtime = "claude"
env = { ANTHROPIC_BASE_URL = "https://openrouter.ai/api/v1", ANTHROPIC_AUTH_TOKEN = "${keyring:openrouter}", ANTHROPIC_MODEL = "z-ai/glm-5.2" }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_the_five_models() {
        let m: ModelsFile = toml::from_str(DEFAULT_MODELS_TOML).unwrap();
        for key in ["claude", "gpt", "kimi", "minimax", "glm"] {
            assert!(m.get(key).is_some(), "{key}");
        }
        assert!(m.get("claude").unwrap().env.is_empty());
        let kimi = m.get("kimi").unwrap();
        assert_eq!(kimi.runtime, "claude");
        assert_eq!(kimi.env["ANTHROPIC_AUTH_TOKEN"], "${keyring:openrouter}");
    }

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("DAMON_CONFIG_DIR", tmp.path());
        let m = ModelsFile::load().unwrap();
        assert!(m.get("claude").is_some());
        std::env::remove_var("DAMON_CONFIG_DIR");
    }
}
