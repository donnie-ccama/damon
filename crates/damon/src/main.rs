mod commands;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "damon",
    version,
    about = "Orchestrator for persistent coding agents"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scaffold ~/damon plus config.toml and models.toml
    Init,
    /// Check required external tools
    Doctor,
    /// Manage teams
    Team {
        #[command(subcommand)]
        cmd: TeamCmd,
    },
    /// Manage agents
    Agent {
        #[command(subcommand)]
        cmd: AgentCmd,
    },
    /// Manage provider API keys (OS keyring)
    Key {
        #[command(subcommand)]
        cmd: KeyCmd,
    },
    /// Open an agent session (spawn or reattach) in the terminal
    Open {
        reference: String,
        #[arg(long)]
        model: Option<String>,
        /// Always spawn a fresh session
        #[arg(long)]
        new: bool,
    },
    /// List live sessions
    Sessions,
    /// Kill a session by name, or all of an agent's sessions
    Kill { target: String },
    /// Internal: hooks invoked by agent runtimes (e.g. Claude Code's Stop hook)
    #[command(hide = true)]
    Hook {
        #[command(subcommand)]
        cmd: HookCmd,
    },
    /// Launch the interactive TUI
    Ui,
}

#[derive(Subcommand)]
enum HookCmd {
    /// Session-end reflection: block the first Stop, prompt a memory write-back, allow the second
    Reflect,
}

#[derive(Subcommand)]
enum TeamCmd {
    /// Create a team
    New { name: String },
    /// List teams
    Ls,
    /// Remove a team (refuses if it has agents unless --force)
    Rm {
        slug: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum AgentCmd {
    /// Create an agent: damon agent new <team>/<Name> --repo-new|--repo-clone URL|--repo-worktree PATH
    New {
        reference: String,
        #[arg(long, value_enum, default_value = "claude")]
        runtime: RuntimeArg,
        #[arg(long)]
        role: Option<String>,
        #[arg(long, group = "repo")]
        repo_new: bool,
        #[arg(long, group = "repo")]
        repo_clone: Option<String>,
        #[arg(long, group = "repo")]
        repo_worktree: Option<String>,
        #[arg(long)]
        branch: Option<String>,
    },
    /// List agents (optionally one team's)
    Ls { team: Option<String> },
    /// Remove an agent and its worktree (needs --yes)
    Rm {
        reference: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum KeyCmd {
    /// Store a provider API key in the OS keyring
    Set { provider: String },
    /// Remove a provider API key from the OS keyring
    Rm { provider: String },
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum RuntimeArg {
    Claude,
    Codex,
    Opencode,
}

impl From<RuntimeArg> for damon_core::entity::RuntimeId {
    fn from(r: RuntimeArg) -> Self {
        match r {
            RuntimeArg::Claude => Self::Claude,
            RuntimeArg::Codex => Self::Codex,
            RuntimeArg::Opencode => Self::Opencode,
        }
    }
}

fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Init => commands::init::run(),
        Cmd::Doctor => commands::doctor::run(),
        Cmd::Team { cmd } => match cmd {
            TeamCmd::New { name } => commands::team::new(&name),
            TeamCmd::Ls => commands::team::ls(),
            TeamCmd::Rm { slug, force } => commands::team::rm(&slug, force),
        },
        Cmd::Agent { cmd } => match cmd {
            AgentCmd::New {
                reference,
                runtime,
                role,
                repo_new,
                repo_clone,
                repo_worktree,
                branch,
            } => {
                let repo = if repo_new {
                    commands::agent::RepoArg::New
                } else if let Some(url) = repo_clone {
                    commands::agent::RepoArg::Clone(url)
                } else if let Some(path) = repo_worktree {
                    commands::agent::RepoArg::Worktree(path)
                } else {
                    anyhow::bail!("pick one of --repo-new, --repo-clone URL, --repo-worktree PATH")
                };
                commands::agent::new(&reference, runtime.into(), role, repo, branch)
            }
            AgentCmd::Ls { team } => commands::agent::ls(team.as_deref()),
            AgentCmd::Rm { reference, yes } => commands::agent::rm(&reference, yes),
        },
        Cmd::Key { cmd } => match cmd {
            KeyCmd::Set { provider } => commands::key::set(&provider),
            KeyCmd::Rm { provider } => commands::key::rm(&provider),
        },
        Cmd::Open {
            reference,
            model,
            new,
        } => commands::open::run(&reference, model.as_deref(), new),
        Cmd::Sessions => commands::sessions::ls(),
        Cmd::Kill { target } => commands::sessions::kill(&target),
        Cmd::Hook { cmd } => match cmd {
            HookCmd::Reflect => commands::hook::reflect(),
        },
        Cmd::Ui => tui::run(),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli.cmd) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
