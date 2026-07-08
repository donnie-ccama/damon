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

    /// Sessions with creation time (unix seconds). Missing server = empty.
    ///
    /// Uses `|` (not `\t`) as the field separator: tmux 3.7b silently
    /// rewrites embedded tab bytes in `-F` output to `_`, which is
    /// indistinguishable from underscores already used in session names
    /// (e.g. `damon_team_agent_1`). Verified with `tmux -F $'...\t...'`
    /// bypassing shell quoting; `|` passes through unmodified.
    pub fn list_info(&self) -> Result<Vec<SessionInfo>, TmuxError> {
        let args: Vec<String> = vec![
            "list-sessions".into(),
            "-F".into(),
            "#{session_name}|#{session_created}".into(),
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
        Ok(out
            .lines()
            .filter_map(|l| {
                let (name, created) = l.split_once('|')?;
                Some(SessionInfo {
                    name: name.to_string(),
                    created_unix: created.parse().ok()?,
                })
            })
            .collect())
    }

    /// One variable from the session's environment (set at spawn via `-e`).
    pub fn env_var(&self, session: &str, var: &str) -> Result<Option<String>, TmuxError> {
        let args: Vec<String> = vec![
            "show-environment".into(),
            "-t".into(),
            session.into(),
            var.into(),
        ];
        match self.run(&args) {
            Ok(out) => Ok(out
                .lines()
                .next()
                .and_then(|l| l.split_once('='))
                .map(|(_, v)| v.to_string())),
            // tmux exits nonzero for an unknown variable.
            Err(TmuxError::Failed { stderr, .. }) if stderr.contains("unknown variable") => {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
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
