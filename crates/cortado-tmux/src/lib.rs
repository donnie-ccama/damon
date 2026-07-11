//! tmux wrapper on a dedicated socket (`tmux -L <socket>`).
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[derive(thiserror::Error, Debug)]
pub enum TmuxError {
    #[error("failed to run tmux: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("tmux {args} failed: {stderr}")]
    Failed { args: String, stderr: String },
    #[error("cannot parse tmux version from {0:?}")]
    Version(String),
}

/// Join args for error display, redacting `-e KEY=VALUE` values (secrets).
fn display_args(args: &[String]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(args.len());
    let mut prev_was_e = false;
    for a in args {
        if prev_was_e {
            out.push(match a.split_once('=') {
                Some((k, _)) => format!("{k}=***"),
                None => "***".to_string(),
            });
        } else {
            out.push(a.clone());
        }
        prev_was_e = a == "-e";
    }
    out.join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub name: String,
    pub created_unix: i64,
    /// The `@cortado_model` user option, if set at spawn.
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: String,
    /// The `@cortado_session` pane option, if tagged.
    pub session_tag: Option<String>,
}

/// Parse one `#{pane_id}|#{@cortado_session}` line.
fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let (id, tag) = line.split_once('|')?;
    Some(PaneInfo {
        id: id.to_string(),
        session_tag: (!tag.is_empty()).then(|| tag.to_string()),
    })
}

pub struct Tmux {
    socket: String,
}

impl Tmux {
    pub fn new(socket: String) -> Tmux {
        Tmux { socket }
    }

    pub fn socket(&self) -> &str {
        &self.socket
    }

    fn run(&self, args: &[String]) -> Result<String, TmuxError> {
        let out = Command::new("tmux")
            .arg("-L")
            .arg(&self.socket)
            .args(args)
            .output()?;
        if !out.status.success() {
            return Err(TmuxError::Failed {
                args: display_args(args),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// `new-session -d -s <name> -c <cwd> [-e K=V]... -- command...`
    /// Env goes via `-e` (tmux >= 3.2) so secrets never hit a shell rc or disk.
    pub fn spawn(
        &self,
        name: &str,
        cwd: &Path,
        env: &BTreeMap<String, String>,
        command: &[String],
    ) -> Result<(), TmuxError> {
        let mut args: Vec<String> = vec![
            "new-session".into(),
            "-d".into(),
            "-s".into(),
            name.into(),
            "-c".into(),
            cwd.to_string_lossy().into_owned(),
        ];
        for (k, v) in env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push("--".into());
        args.extend(command.iter().cloned());
        self.run(&args)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>, TmuxError> {
        let args: Vec<String> = vec![
            "list-sessions".into(),
            "-F".into(),
            "#{session_name}".into(),
        ];
        match self.run(&args) {
            Ok(out) => Ok(out.lines().map(str::to_string).collect()),
            // No server on this socket yet = no sessions.
            Err(TmuxError::Failed { stderr, .. })
                if stderr.contains("no server running") || stderr.contains("No such file") =>
            {
                Ok(Vec::new())
            }
            Err(e) => Err(e),
        }
    }

    pub fn has(&self, name: &str) -> Result<bool, TmuxError> {
        Ok(self.list()?.iter().any(|s| s == name))
    }

    pub fn kill(&self, name: &str) -> Result<(), TmuxError> {
        self.run(&["kill-session".into(), "-t".into(), name.into()])?;
        Ok(())
    }

    pub fn kill_server(&self) -> Result<(), TmuxError> {
        self.run(&["kill-server".into()])?;
        Ok(())
    }

    /// Sessions with creation time and the `@cortado_model` user option.
    /// Missing server = empty. `|` separator (tmux 3.7b mangles `\t` in `-F`).
    pub fn list_info(&self) -> Result<Vec<SessionInfo>, TmuxError> {
        let args: Vec<String> = vec![
            "list-sessions".into(),
            "-F".into(),
            "#{session_name}|#{session_created}|#{@cortado_model}".into(),
        ];
        let out = match self.run(&args) {
            Ok(out) => out,
            Err(TmuxError::Failed { stderr, .. })
                if stderr.contains("no server running") || stderr.contains("No such file") =>
            {
                return Ok(Vec::new())
            }
            Err(e) => return Err(e),
        };
        Ok(out.lines().filter_map(parse_info_line).collect())
    }

    /// Set a tmux option on a session (used for `@cortado_model` at spawn).
    pub fn set_option(&self, session: &str, name: &str, value: &str) -> Result<(), TmuxError> {
        self.run(&[
            "set-option".into(),
            "-t".into(),
            session.into(),
            name.into(),
            value.into(),
        ])?;
        Ok(())
    }

    /// Set several session options in one call (used at spawn/workspace-create).
    pub fn set_session_options(
        &self,
        session: &str,
        opts: &[(&str, &str)],
    ) -> Result<(), TmuxError> {
        for (k, v) in opts {
            self.set_option(session, k, v)?;
        }
        Ok(())
    }

    /// Window-scoped option (e.g. `main-pane-width` on `session:0`).
    pub fn set_window_option(
        &self,
        target: &str,
        name: &str,
        value: &str,
    ) -> Result<(), TmuxError> {
        self.run(&[
            "set-option".into(),
            "-w".into(),
            "-t".into(),
            target.into(),
            name.into(),
            value.into(),
        ])?;
        Ok(())
    }

    /// `split-window -t <target> [-e K=V]... -P -F #{pane_id} -- command...`
    /// Returns the new pane's id (`%N`). Env via `-e` (tmux >= 3.2), same
    /// secrecy rationale as `spawn`.
    pub fn split_window(
        &self,
        target: &str,
        env: &BTreeMap<String, String>,
        command: &[String],
    ) -> Result<String, TmuxError> {
        let mut args: Vec<String> = vec![
            "split-window".into(),
            "-t".into(),
            target.into(),
            "-P".into(),
            "-F".into(),
            "#{pane_id}".into(),
        ];
        for (k, v) in env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push("--".into());
        args.extend(command.iter().cloned());
        Ok(self.run(&args)?.trim().to_string())
    }

    pub fn select_pane(&self, pane: &str) -> Result<(), TmuxError> {
        self.run(&["select-pane".into(), "-t".into(), pane.into()])?;
        Ok(())
    }

    /// Pane-scoped user option (`set-option -p`), e.g. `@cortado_session`.
    pub fn set_pane_option(&self, pane: &str, name: &str, value: &str) -> Result<(), TmuxError> {
        self.run(&[
            "set-option".into(),
            "-p".into(),
            "-t".into(),
            pane.into(),
            name.into(),
            value.into(),
        ])?;
        Ok(())
    }

    /// All panes of a session (all windows), with the `@cortado_session` tag.
    pub fn list_panes(&self, session: &str) -> Result<Vec<PaneInfo>, TmuxError> {
        let out = self.run(&[
            "list-panes".into(),
            "-s".into(),
            "-t".into(),
            session.into(),
            "-F".into(),
            "#{pane_id}|#{@cortado_session}".into(),
        ])?;
        Ok(out.lines().filter_map(parse_pane_line).collect())
    }

    pub fn select_layout(&self, target: &str, layout: &str) -> Result<(), TmuxError> {
        self.run(&[
            "select-layout".into(),
            "-t".into(),
            target.into(),
            layout.into(),
        ])?;
        Ok(())
    }

    /// True if any client is attached to the session.
    pub fn has_client(&self, session: &str) -> Result<bool, TmuxError> {
        let out = self.run(&[
            "list-clients".into(),
            "-t".into(),
            session.into(),
            "-F".into(),
            "#{client_name}".into(),
        ])?;
        Ok(out.lines().any(|l| !l.is_empty()))
    }

    /// One session option's value (test/assertion helper).
    pub fn show_session_option(&self, session: &str, name: &str) -> Result<String, TmuxError> {
        let out = self.run(&[
            "show-options".into(),
            "-v".into(),
            "-t".into(),
            session.into(),
            name.into(),
        ])?;
        Ok(out.trim().to_string())
    }
}

/// Parse one `#{session_name}|#{session_created}|#{@cortado_model}` line.
/// An empty model field (unset user option) becomes `None`.
fn parse_info_line(line: &str) -> Option<SessionInfo> {
    let mut parts = line.split('|');
    let name = parts.next()?.to_string();
    let created_unix = parts.next()?.parse().ok()?;
    let model = parts.next().filter(|m| !m.is_empty()).map(str::to_string);
    Some(SessionInfo {
        name,
        created_unix,
        model,
    })
}

/// Parse `tmux -V` (e.g. "tmux 3.4", "tmux 3.3a") into (major, minor).
pub fn version() -> Result<(u32, u32), TmuxError> {
    let out = Command::new("tmux").arg("-V").output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let num = text.split_whitespace().last().unwrap_or_default();
    let cleaned: String = num
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let mut parts = cleaned.split('.');
    let major = parts.next().and_then(|p| p.parse().ok());
    let minor = parts.next().and_then(|p| p.parse().ok());
    match (major, minor) {
        (Some(ma), Some(mi)) => Ok((ma, mi)),
        _ => Err(TmuxError::Version(text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pane_lines_with_optional_session_tag() {
        assert_eq!(
            parse_pane_line("%3|cortado_newsletter_scout_1"),
            Some(PaneInfo {
                id: "%3".into(),
                session_tag: Some("cortado_newsletter_scout_1".into()),
            })
        );
        // Untagged pane (e.g. the rail) renders an empty trailing field.
        assert_eq!(
            parse_pane_line("%0|"),
            Some(PaneInfo {
                id: "%0".into(),
                session_tag: None
            })
        );
        assert_eq!(parse_pane_line("no-separator"), None);
    }

    #[test]
    fn parses_three_field_lines_with_optional_model() {
        assert_eq!(
            parse_info_line("cortado_a_b_1|1700000000|claude"),
            Some(SessionInfo {
                name: "cortado_a_b_1".into(),
                created_unix: 1_700_000_000,
                model: Some("claude".into()),
            })
        );
        // Unset @cortado_model renders as an empty trailing field.
        assert_eq!(
            parse_info_line("cortado_a_b_1|1700000000|"),
            Some(SessionInfo {
                name: "cortado_a_b_1".into(),
                created_unix: 1_700_000_000,
                model: None,
            })
        );
        // Missing created field -> unparseable -> dropped.
        assert_eq!(parse_info_line("weird-line"), None);
    }
}
