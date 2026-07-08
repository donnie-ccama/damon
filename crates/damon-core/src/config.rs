use crate::CoreError;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub tmux: TmuxCfg,
    pub terminal: TerminalCfg,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct General {
    pub root: String,
    pub default_runtime: String,
}

impl Default for General {
    fn default() -> Self {
        General { root: "~/damon".into(), default_runtime: "claude".into() }
    }
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TmuxCfg {
    pub socket: String,
}

impl Default for TmuxCfg {
    fn default() -> Self {
        TmuxCfg { socket: "damon".into() }
    }
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TerminalCfg {
    pub launcher: Launcher,
}

impl Default for TerminalCfg {
    fn default() -> Self {
        TerminalCfg { launcher: Launcher::Ghostty }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Launcher {
    Ghostty,
    EnvTerminal,
    Print,
}

pub fn expand_tilde(p: &str) -> PathBuf {
    if p == "~" {
        return dirs::home_dir().expect("no home dir");
    }
    match p.strip_prefix("~/") {
        Some(rest) => dirs::home_dir().expect("no home dir").join(rest),
        None => PathBuf::from(p),
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("DAMON_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        dirs::config_dir().expect("no config dir").join("damon")
    }

    /// Load `config.toml` from the config dir; missing file = defaults.
    pub fn load() -> Result<Config, CoreError> {
        let path = Self::config_dir().join("config.toml");
        load_toml_or_default(&path)
    }

    pub fn root(&self) -> PathBuf {
        if let Ok(root) = std::env::var("DAMON_ROOT") {
            return PathBuf::from(root);
        }
        expand_tilde(&self.general.root)
    }

    pub fn default_toml() -> String {
        toml::to_string_pretty(&Config::default()).expect("serialize default config")
    }
}

pub(crate) fn load_toml_or_default<T: serde::de::DeserializeOwned + Default>(
    path: &Path,
) -> Result<T, CoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| CoreError::Toml {
            path: path.to_path_buf(),
            msg: e.to_string(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(CoreError::Io { path: path.to_path_buf(), source: e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_spec_values() {
        let c = Config::default();
        assert_eq!(c.general.root, "~/damon");
        assert_eq!(c.general.default_runtime, "claude");
        assert_eq!(c.tmux.socket, "damon");
        assert_eq!(c.terminal.launcher, Launcher::Ghostty);
    }

    #[test]
    fn default_toml_round_trips() {
        let parsed: Config = toml::from_str(&Config::default_toml()).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn partial_file_fills_defaults() {
        let c: Config = toml::from_str("[terminal]\nlauncher = \"print\"\n").unwrap();
        assert_eq!(c.terminal.launcher, Launcher::Print);
        assert_eq!(c.tmux.socket, "damon");
    }

    #[test]
    fn env_root_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("DAMON_ROOT", tmp.path());
        assert_eq!(Config::default().root(), tmp.path());
        std::env::remove_var("DAMON_ROOT");
    }

    #[test]
    fn expands_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/damon"), home.join("damon"));
        assert_eq!(expand_tilde("/abs/x"), std::path::PathBuf::from("/abs/x"));
    }

    #[test]
    fn load_missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let c: Config = load_toml_or_default(&tmp.path().join("nope.toml")).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn load_malformed_toml_errors_with_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "not [valid toml").unwrap();
        let err = load_toml_or_default::<Config>(&path).unwrap_err();
        match err {
            crate::CoreError::Toml { path: p, .. } => assert_eq!(p, path),
            other => panic!("expected Toml error, got {other:?}"),
        }
        // never silently rewritten
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not [valid toml");
    }
}
