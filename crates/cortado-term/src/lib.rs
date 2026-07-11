//! Terminal launchers: open a window attached to a cortado tmux session.
pub mod workspace;

use std::process::Command;

pub trait TerminalLauncher {
    fn open(&self, session: &str, title: &str) -> anyhow::Result<()>;
}

pub fn attach_command(socket: &str, session: &str) -> Vec<String> {
    ["tmux", "-L", socket, "attach", "-t", session]
        .map(String::from)
        .to_vec()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Os {
    MacOs,
    Linux,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// (binary, args) to open Ghostty attached to the session. Pure for testing.
pub fn ghostty_invocation(os: Os, socket: &str, session: &str) -> (String, Vec<String>) {
    let attach = attach_command(socket, session);
    match os {
        Os::MacOs => {
            let mut args: Vec<String> = ["-na", "Ghostty", "--args", "-e"]
                .map(String::from)
                .to_vec();
            args.extend(attach);
            ("open".to_string(), args)
        }
        Os::Linux => {
            let mut args = vec!["-e".to_string()];
            args.extend(attach);
            ("ghostty".to_string(), args)
        }
    }
}

pub struct GhosttyLauncher {
    pub socket: String,
}

impl TerminalLauncher for GhosttyLauncher {
    fn open(&self, session: &str, _title: &str) -> anyhow::Result<()> {
        let (bin, args) = ghostty_invocation(Os::current(), &self.socket, session);
        // Fire-and-forget: Ghostty owns the window from here.
        Command::new(&bin)
            .args(&args)
            .spawn()
            .map_err(|e| anyhow::anyhow!("launching {bin}: {e} (is Ghostty installed?)"))?;
        Ok(())
    }
}

pub struct EnvTerminalLauncher {
    pub socket: String,
}

impl TerminalLauncher for EnvTerminalLauncher {
    fn open(&self, session: &str, _title: &str) -> anyhow::Result<()> {
        let term = std::env::var("TERMINAL")
            .map_err(|_| anyhow::anyhow!("$TERMINAL is not set (launcher = \"env-terminal\")"))?;
        let mut args = vec!["-e".to_string()];
        args.extend(attach_command(&self.socket, session));
        Command::new(&term).args(&args).spawn()?;
        Ok(())
    }
}

pub struct PrintLauncher {
    pub socket: String,
}

impl TerminalLauncher for PrintLauncher {
    fn open(&self, session: &str, _title: &str) -> anyhow::Result<()> {
        println!(
            "attach with: {}",
            attach_command(&self.socket, session).join(" ")
        );
        Ok(())
    }
}

pub fn launcher_for(
    cfg: cortado_core::config::Launcher,
    socket: String,
) -> Box<dyn TerminalLauncher> {
    use cortado_core::config::Launcher as L;
    match cfg {
        L::Workspace => Box::new(PrintLauncher { socket }), // placeholder until WorkspaceLauncher (Task 4)
        L::Ghostty => Box::new(GhosttyLauncher { socket }),
        L::EnvTerminal => Box::new(EnvTerminalLauncher { socket }),
        L::Print => Box::new(PrintLauncher { socket }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_command_targets_cortado_socket() {
        assert_eq!(
            attach_command("cortado", "cortado_t_a_1"),
            vec!["tmux", "-L", "cortado", "attach", "-t", "cortado_t_a_1"]
        );
    }

    #[test]
    fn ghostty_invocation_per_os() {
        let (bin, args) = ghostty_invocation(Os::MacOs, "cortado", "s1");
        assert_eq!(bin, "open");
        assert_eq!(
            args[..4],
            [
                "-na".to_string(),
                "Ghostty".to_string(),
                "--args".to_string(),
                "-e".to_string()
            ]
        );
        assert!(args.ends_with(&[
            "tmux".into(),
            "-L".into(),
            "cortado".into(),
            "attach".into(),
            "-t".into(),
            "s1".into()
        ]));

        let (bin, args) = ghostty_invocation(Os::Linux, "cortado", "s1");
        assert_eq!(bin, "ghostty");
        assert_eq!(args[0], "-e");
    }

    #[test]
    fn print_launcher_never_fails() {
        PrintLauncher {
            socket: "cortado".into(),
        }
        .open("s1", "title")
        .unwrap();
    }
}
