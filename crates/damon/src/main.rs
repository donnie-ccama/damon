mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "damon", version, about = "Orchestrator for persistent coding agents")]
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

fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Init => commands::init::run(),
        Cmd::Doctor => commands::doctor::run(),
        Cmd::Team { cmd } => match cmd {
            TeamCmd::New { name } => commands::team::new(&name),
            TeamCmd::Ls => commands::team::ls(),
            TeamCmd::Rm { slug, force } => commands::team::rm(&slug, force),
        },
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli.cmd) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
