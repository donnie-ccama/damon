use crate::CoreError;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    /// Obsolete since the Herdr substrate swap — removed when cortado-tmux/cortado-term are deleted.
    pub tmux: TmuxCfg,
    /// Obsolete since the Herdr substrate swap — removed when cortado-tmux/cortado-term are deleted.
    pub terminal: TerminalCfg,
    pub herdr: HerdrCfg,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct General {
    pub root: String,
    pub default_runtime: String,
}

impl Default for General {
    fn default() -> Self {
        General {
            root: "~/cortado".into(),
            default_runtime: "claude".into(),
        }
    }
}

/// Obsolete since the Herdr substrate swap — removed when cortado-tmux/cortado-term are deleted.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TmuxCfg {
    pub socket: String,
}

impl Default for TmuxCfg {
    fn default() -> Self {
        TmuxCfg {
            socket: "cortado".into(),
        }
    }
}

/// Obsolete since the Herdr substrate swap — removed when cortado-tmux/cortado-term are deleted.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TerminalCfg {
    pub launcher: Launcher,
    /// How workspace mode opens its one OS window. Ignored by other launchers.
    pub window: Window,
}

impl Default for TerminalCfg {
    fn default() -> Self {
        TerminalCfg {
            launcher: Launcher::Workspace,
            window: Window::Ghostty,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Launcher {
    Workspace,
    Ghostty,
    EnvTerminal,
    Print,
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Window {
    Ghostty,
    EnvTerminal,
    Print,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HerdrCfg {
    /// Path or name of the herdr binary.
    pub binary: String,
    /// Label of the Herdr workspace cortado owns.
    pub workspace: String,
}

impl Default for HerdrCfg {
    fn default() -> Self {
        HerdrCfg { binary: "herdr".into(), workspace: "Cortado".into() }
    }
}

fn home_dir() -> Result<PathBuf, CoreError> {
    dirs::home_dir().ok_or(CoreError::NoHome)
}

pub fn expand_tilde(p: &str) -> Result<PathBuf, CoreError> {
    if p == "~" {
        return home_dir();
    }
    match p.strip_prefix("~/") {
        Some(rest) => Ok(home_dir()?.join(rest)),
        None => Ok(PathBuf::from(p)),
    }
}

/// One layout on every OS: `<home>/.config/cortado` (macOS and Linux alike),
/// so configs stay rsync-identical across machines.
pub fn default_config_dir(home: &Path) -> PathBuf {
    home.join(".config").join("cortado")
}

impl Config {
    pub fn config_dir() -> Result<PathBuf, CoreError> {
        if let Ok(dir) = std::env::var("CORTADO_CONFIG_DIR") {
            return Ok(PathBuf::from(dir));
        }
        Ok(default_config_dir(&home_dir()?))
    }

    /// Load `config.toml` from the config dir; missing file = defaults.
    pub fn load() -> Result<Config, CoreError> {
        let path = Self::config_dir()?.join("config.toml");
        load_toml_or_default(&path)
    }

    pub fn root(&self) -> Result<PathBuf, CoreError> {
        if let Ok(root) = std::env::var("CORTADO_ROOT") {
            return Ok(PathBuf::from(root));
        }
        expand_tilde(&self.general.root)
    }

    pub fn default_toml() -> String {
        toml::to_string_pretty(&Config::default()).expect("serialize default config")
    }

    /// Test seam: target a named herdr session (isolated socket).
    pub fn herdr_session() -> Option<String> {
        std::env::var("CORTADO_HERDR_SESSION").ok().filter(|s| !s.is_empty())
    }
}

/// Retired config sections present in the file at `path` (best-effort:
/// unreadable/invalid files report nothing — load() owns real errors).
pub fn obsolete_sections(path: &Path) -> Vec<&'static str> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let Ok(v) = text.parse::<toml::Table>() else { return Vec::new() };
    ["tmux", "terminal"].into_iter().filter(|s| v.contains_key(*s)).collect()
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
        Err(e) => Err(CoreError::Io {
            path: path.to_path_buf(),
            source: e,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_spec_values() {
        let c = Config::default();
        assert_eq!(c.general.root, "~/cortado");
        assert_eq!(c.general.default_runtime, "claude");
        assert_eq!(c.tmux.socket, "cortado");
        assert_eq!(c.terminal.launcher, Launcher::Workspace);
        assert_eq!(c.terminal.window, Window::Ghostty);
        assert_eq!(c.herdr.binary, "herdr");
        assert_eq!(c.herdr.workspace, "Cortado");
    }

    #[test]
    fn workspace_launcher_and_window_parse() {
        let c: Config =
            toml::from_str("[terminal]\nlauncher = \"workspace\"\nwindow = \"print\"\n").unwrap();
        assert_eq!(c.terminal.launcher, Launcher::Workspace);
        assert_eq!(c.terminal.window, Window::Print);
        // Old configs without `window` still parse.
        let c: Config = toml::from_str("[terminal]\nlauncher = \"ghostty\"\n").unwrap();
        assert_eq!(c.terminal.launcher, Launcher::Ghostty);
        assert_eq!(c.terminal.window, Window::Ghostty);
    }

    #[test]
    fn herdr_section_parses_and_fills_defaults() {
        let c: Config = toml::from_str("[herdr]\nbinary = \"/opt/herdr\"\n").unwrap();
        assert_eq!(c.herdr.binary, "/opt/herdr");
        assert_eq!(c.herdr.workspace, "Cortado");
    }

    #[test]
    fn old_tmux_terminal_sections_still_parse_but_are_reported() {
        // Old config files must keep loading (serde ignores unknown sections)…
        let text = "[tmux]\nsocket = \"cortado\"\n[terminal]\nlauncher = \"workspace\"\n";
        let c: Config = toml::from_str(text).unwrap();
        assert_eq!(c, Config::default());
        // …and obsolete_sections names what should be deleted.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        assert_eq!(obsolete_sections(&path), vec!["tmux", "terminal"]);
        assert!(obsolete_sections(&tmp.path().join("missing.toml")).is_empty());
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
        assert_eq!(c.tmux.socket, "cortado");
    }

    #[test]
    fn env_root_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CORTADO_ROOT", tmp.path());
        assert_eq!(Config::default().root().unwrap(), tmp.path());
        std::env::remove_var("CORTADO_ROOT");
    }

    #[test]
    fn expands_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~").unwrap(), home);
        assert_eq!(expand_tilde("~/cortado").unwrap(), home.join("cortado"));
        assert_eq!(
            expand_tilde("/abs/x").unwrap(),
            std::path::PathBuf::from("/abs/x")
        );
    }

    #[test]
    fn config_dir_is_dot_config_on_every_os() {
        // Pure function: same layout on macOS and Linux, no dirs::config_dir()
        // (which would put macOS config in ~/Library/Application Support).
        assert_eq!(
            default_config_dir(std::path::Path::new("/home/donnie")),
            std::path::PathBuf::from("/home/donnie/.config/cortado")
        );
    }

    #[test]
    fn resolution_failures_are_typed_errors_not_panics() {
        // API-shape pin: home/config resolution returns CoreError, never panics.
        let _: Result<std::path::PathBuf, crate::CoreError> = expand_tilde("~/x");
        let _: Result<std::path::PathBuf, crate::CoreError> = Config::config_dir();
        let _: Result<std::path::PathBuf, crate::CoreError> = Config::default().root();
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
